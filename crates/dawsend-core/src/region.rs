//! Ouverture et validation de la zone partagée, accès typés aux en-têtes et aux rings.

use std::fmt;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, Instant};

use crate::layout::*;
use crate::shm::{self, Mapping};
use crate::{clock, process};

/// L'appli est considérée présente si son heartbeat date de moins de ça.
pub const APP_ALIVE_TIMEOUT_NS: u64 = 1_000 * clock::MS;

#[derive(Debug)]
pub enum OpenError {
    Io(std::io::Error),
    /// La zone existe mais n'a pas le format attendu.
    Incompatible {
        magic: u64,
        version: u32,
    },
}

impl fmt::Display for OpenError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OpenError::Io(e) => write!(f, "mémoire partagée inaccessible : {e}"),
            OpenError::Incompatible { magic, version } => write!(
                f,
                "mémoire partagée incompatible (magic {magic:#x}, version {version}, attendu version {LAYOUT_VERSION})"
            ),
        }
    }
}

impl std::error::Error for OpenError {}

impl From<std::io::Error> for OpenError {
    fn from(e: std::io::Error) -> Self {
        OpenError::Io(e)
    }
}

/// La zone partagée Dawsend, mappée dans ce processus.
pub struct Region {
    map: Mapping,
    name: String,
}

impl fmt::Debug for Region {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Region").field("name", &self.name).finish()
    }
}

impl Region {
    /// Ouvre (ou crée) la zone partagée par défaut. Hors thread audio uniquement.
    pub fn open() -> Result<Arc<Self>, OpenError> {
        Self::open_named(&shm::default_name())
    }

    /// Ouvre (ou crée) une zone partagée nommée. Hors thread audio uniquement.
    pub fn open_named(name: &str) -> Result<Arc<Self>, OpenError> {
        clock::init();
        let map = Mapping::open_or_create(name, TOTAL_SIZE)?;
        let region = Region {
            map,
            name: name.to_owned(),
        };
        region.initialize()?;
        Ok(Arc::new(region))
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    fn initialize(&self) -> Result<(), OpenError> {
        let h = self.header();
        let won = h
            .init_state
            .compare_exchange(
                INIT_BLANK,
                INIT_IN_PROGRESS,
                Ordering::AcqRel,
                Ordering::Acquire,
            )
            .is_ok();
        if !won {
            // Un autre processus initialise : attendre au plus 200 ms.
            let deadline = Instant::now() + Duration::from_millis(200);
            while h.init_state.load(Ordering::Acquire) != INIT_READY {
                if Instant::now() >= deadline {
                    // Il a probablement planté pendant l'initialisation : on reprend.
                    // Les valeurs écrites sont des constantes, donc l'opération est idempotente.
                    break;
                }
                std::thread::sleep(Duration::from_millis(1));
            }
        }
        if won || h.init_state.load(Ordering::Acquire) != INIT_READY {
            let r = Ordering::Relaxed;
            h.layout_version.store(LAYOUT_VERSION, r);
            h.header_size.store(HEADER_SIZE as u32, r);
            h.slot_count.store(SLOT_COUNT as u32, r);
            h.slot_header_size.store(SLOT_HEADER_SIZE as u32, r);
            h.max_channels.store(MAX_CHANNELS as u32, r);
            h.ring_capacity_frames.store(RING_CAPACITY_FRAMES as u32, r);
            h.data_offset.store(DATA_OFFSET as u64, r);
            h.total_size.store(TOTAL_SIZE as u64, r);
            h.magic.store(MAGIC, r);
            h.init_state.store(INIT_READY, Ordering::Release);
        }
        self.validate()
    }

    fn validate(&self) -> Result<(), OpenError> {
        let h = self.header();
        let r = Ordering::Relaxed;
        let magic = h.magic.load(r);
        let version = h.layout_version.load(r);
        let ok = magic == MAGIC
            && version == LAYOUT_VERSION
            && h.header_size.load(r) == HEADER_SIZE as u32
            && h.slot_count.load(r) == SLOT_COUNT as u32
            && h.slot_header_size.load(r) == SLOT_HEADER_SIZE as u32
            && h.max_channels.load(r) == MAX_CHANNELS as u32
            && h.ring_capacity_frames.load(r) == RING_CAPACITY_FRAMES as u32
            && h.data_offset.load(r) == DATA_OFFSET as u64
            && h.total_size.load(r) == TOTAL_SIZE as u64;
        if ok {
            Ok(())
        } else {
            Err(OpenError::Incompatible { magic, version })
        }
    }

    pub fn header(&self) -> &GlobalHeader {
        // SAFETY : la zone fait TOTAL_SIZE octets, est alignée sur une page, et
        // GlobalHeader n'est fait que d'atomiques (tout motif de bits est valide).
        unsafe { &*(self.map.as_ptr() as *const GlobalHeader) }
    }

    /// En-tête du slot `index` (< SLOT_COUNT, sinon panique).
    pub fn slot(&self, index: usize) -> &SlotHeader {
        assert!(index < SLOT_COUNT);
        // SAFETY : offset dans la zone, aligné sur 64 octets, atomiques uniquement.
        unsafe { &*(self.map.as_ptr().add(slot_offset(index)) as *const SlotHeader) }
    }

    /// Échantillons (bits de f32) du ring du slot `index` : RING_SAMPLES valeurs entrelacées.
    pub fn ring(&self, index: usize) -> &[AtomicU32] {
        assert!(index < SLOT_COUNT);
        // SAFETY : plage [ring_offset, ring_offset + RING_BYTES) incluse dans la zone,
        // alignée sur 4 octets ; AtomicU32 a la taille et l'alignement de u32.
        unsafe {
            std::slice::from_raw_parts(
                self.map.as_ptr().add(ring_offset(index)) as *const AtomicU32,
                RING_SAMPLES,
            )
        }
    }

    // ---------------------------------------------------------------- côté appli

    /// L'appli annonce sa présence (à appeler au démarrage, puis `app_heartbeat`).
    pub fn app_announce(&self) {
        let h = self.header();
        h.app_start_time
            .store(process::current_start_time(), Ordering::Relaxed);
        h.app_pid.store(process::current_pid(), Ordering::Relaxed);
        self.app_heartbeat(false);
    }

    /// Heartbeat de l'appli (toutes les ~100 ms).
    pub fn app_heartbeat(&self, playing: bool) {
        let h = self.header();
        h.app_flags.store(
            if playing { APP_FLAG_PLAYING } else { 0 },
            Ordering::Relaxed,
        );
        h.app_heartbeat_ns.store(clock::now_ns(), Ordering::Release);
    }

    /// L'appli se retire proprement.
    pub fn app_leave(&self) {
        let h = self.header();
        if h.app_pid.load(Ordering::Relaxed) == process::current_pid() {
            h.app_pid.store(0, Ordering::Relaxed);
            h.app_flags.store(0, Ordering::Relaxed);
            h.app_heartbeat_ns.store(0, Ordering::Release);
        }
    }

    /// `true` si une appli Dawsend a donné signe de vie récemment. Non bloquant.
    pub fn app_alive(&self, now_ns: u64) -> bool {
        let h = self.header();
        h.app_pid.load(Ordering::Relaxed) != 0
            && now_ns.saturating_sub(h.app_heartbeat_ns.load(Ordering::Acquire))
                < APP_ALIVE_TIMEOUT_NS
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;

    /// Zone de test au nom unique, supprimée à la fin.
    pub struct TestRegion {
        pub name: String,
    }

    impl TestRegion {
        pub fn new() -> Self {
            static N: AtomicUsize = AtomicUsize::new(0);
            let n = N.fetch_add(1, Ordering::Relaxed);
            #[cfg(windows)]
            let name = format!("Local\\DawsendTest.{}.{}", std::process::id(), n);
            #[cfg(unix)]
            let name = format!("/dwt.{}.{}", std::process::id(), n);
            Self { name }
        }

        pub fn open(&self) -> Arc<Region> {
            Region::open_named(&self.name).expect("ouverture de la zone de test")
        }
    }

    impl Drop for TestRegion {
        fn drop(&mut self) {
            shm::unlink(&self.name);
        }
    }

    #[test]
    fn two_mappings_share_memory() {
        let t = TestRegion::new();
        let a = t.open();
        let b = t.open();
        a.slot(3).write_pos.store(1234, Ordering::Release);
        assert_eq!(b.slot(3).write_pos.load(Ordering::Acquire), 1234);
        assert_eq!(b.header().magic.load(Ordering::Relaxed), MAGIC);
    }

    #[test]
    fn app_presence() {
        let t = TestRegion::new();
        let r = t.open();
        assert!(!r.app_alive(clock::now_ns()));
        r.app_announce();
        assert!(r.app_alive(clock::now_ns()));
        r.app_leave();
        assert!(!r.app_alive(clock::now_ns()));
    }

    #[test]
    fn incompatible_version_is_refused() {
        let t = TestRegion::new();
        let r = t.open();
        r.header().layout_version.store(99, Ordering::Relaxed);
        let err = Region::open_named(&t.name).unwrap_err();
        assert!(matches!(err, OpenError::Incompatible { version: 99, .. }));
    }
}
