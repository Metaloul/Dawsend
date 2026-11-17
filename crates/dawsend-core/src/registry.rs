//! Réservation et libération des slots, lecture cohérente des métadonnées (seqlock).

use std::fmt;
use std::sync::atomic::{Ordering, fence};
use std::sync::{Arc, Mutex};

use crate::layout::*;
use crate::region::Region;
use crate::ring::Producer;
use crate::{clock, process};

/// Le consommateur (l'appli) est considéré « en écoute » si son heartbeat sur le slot
/// date de moins de ça.
pub const CONSUMER_ALIVE_TIMEOUT_NS: u64 = 1_000 * clock::MS;

/// Instantané cohérent des métadonnées d'un slot actif.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlotMeta {
    pub index: usize,
    pub generation: u32,
    pub owner_pid: u32,
    pub owner_start_time: u64,
    pub instance_id: u64,
    pub sample_rate: u32,
    pub channels: u32,
    pub flags: u32,
    pub name: String,
}

/// Lit les métadonnées du slot `index` s'il est actif et cohérent.
///
/// Toutes les valeurs sont validées : une zone corrompue donne `None`, jamais une panique.
pub fn read_meta(region: &Region, index: usize) -> Option<SlotMeta> {
    if index >= SLOT_COUNT {
        return None;
    }
    let s = region.slot(index);
    for _ in 0..16 {
        if s.state.load(Ordering::Acquire) != SLOT_ACTIVE {
            return None;
        }
        let seq1 = s.seq.load(Ordering::Acquire);
        if seq1 & 1 == 1 {
            std::hint::spin_loop();
            continue;
        }
        let r = Ordering::Relaxed;
        let mut name = [0u8; NAME_LEN];
        for (dst, src) in name.iter_mut().zip(s.name.iter()) {
            *dst = src.load(r);
        }
        let meta = SlotMeta {
            index,
            generation: s.generation.load(r),
            owner_pid: s.owner_pid.load(r),
            owner_start_time: s.owner_start_time.load(r),
            instance_id: s.instance_id.load(r),
            sample_rate: s.sample_rate.load(r),
            channels: s.channels.load(r),
            flags: s.flags.load(r),
            name: decode_name(&name),
        };
        fence(Ordering::Acquire);
        if s.seq.load(Ordering::Relaxed) != seq1 {
            continue;
        }
        let valid = (MIN_SAMPLE_RATE..=MAX_SAMPLE_RATE).contains(&meta.sample_rate)
            && (1..=MAX_CHANNELS as u32).contains(&meta.channels);
        return valid.then_some(meta);
    }
    None
}

/// Liste les sources actives dont le processus propriétaire est vivant.
pub fn active_sources(region: &Region) -> Vec<SlotMeta> {
    (0..SLOT_COUNT)
        .filter_map(|i| read_meta(region, i))
        .filter(|m| process::is_alive(m.owner_pid, m.owner_start_time))
        .collect()
}

/// Libère les slots dont le processus propriétaire est mort (DAW planté).
/// Renvoie le nombre de slots libérés. Hors thread audio.
pub fn reap_dead(region: &Region) -> usize {
    let mut n = 0;
    for i in 0..SLOT_COUNT {
        let s = region.slot(i);
        if s.state.load(Ordering::Acquire) != SLOT_ACTIVE {
            continue;
        }
        let pid = s.owner_pid.load(Ordering::Relaxed);
        let start = s.owner_start_time.load(Ordering::Relaxed);
        if !process::is_alive(pid, start)
            && s.state
                .compare_exchange(SLOT_ACTIVE, SLOT_FREE, Ordering::AcqRel, Ordering::Relaxed)
                .is_ok()
        {
            n += 1;
        }
    }
    n
}

/// Signale au plugin que l'appli lit ce slot (à appeler toutes les ~100 ms).
pub fn mark_consumer(region: &Region, index: usize, flags: u32) {
    if index < SLOT_COUNT {
        let s = region.slot(index);
        s.consumer_flags.store(flags, Ordering::Relaxed);
        s.consumer_heartbeat_ns
            .store(clock::now_ns(), Ordering::Release);
    }
}

fn decode_name(raw: &[u8; NAME_LEN]) -> String {
    let end = raw.iter().position(|&b| b == 0).unwrap_or(NAME_LEN);
    String::from_utf8_lossy(&raw[..end]).into_owned()
}

/// Tronque `name` à NAME_LEN octets sans couper un caractère UTF-8.
fn encode_name(name: &str) -> [u8; NAME_LEN] {
    let mut end = name.len().min(NAME_LEN);
    while !name.is_char_boundary(end) {
        end -= 1;
    }
    let mut out = [0u8; NAME_LEN];
    out[..end].copy_from_slice(&name.as_bytes()[..end]);
    out
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClaimError {
    /// Les 32 slots sont occupés.
    Full,
    InvalidFormat,
}

impl fmt::Display for ClaimError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ClaimError::Full => write!(
                f,
                "plus aucun emplacement de source libre ({SLOT_COUNT} max)"
            ),
            ClaimError::InvalidFormat => write!(f, "format audio non pris en charge"),
        }
    }
}

impl std::error::Error for ClaimError {}

/// État de la liaison vu depuis le plugin.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkStatus {
    /// Aucune appli Dawsend ne tourne.
    AppAbsent,
    /// L'appli tourne mais ne lit pas cette source.
    AppConnected,
    /// L'appli lit cette source.
    Listening,
}

/// Un slot réservé par une instance de plugin. Libéré au `Drop`.
///
/// Toutes les méthodes sont à appeler hors du thread audio, sauf [`SlotClaim::status`].
pub struct SlotClaim {
    region: Arc<Region>,
    index: usize,
    /// Sérialise les écritures de métadonnées dans ce processus (interface / initialize).
    meta_lock: Mutex<()>,
}

impl fmt::Debug for SlotClaim {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SlotClaim")
            .field("index", &self.index)
            .finish()
    }
}

impl SlotClaim {
    /// Réserve un slot libre (ou abandonné par un processus mort) et le publie.
    pub fn claim(
        region: Arc<Region>,
        sample_rate: u32,
        channels: u32,
        name: &str,
    ) -> Result<Self, ClaimError> {
        check_format(sample_rate, channels)?;
        let instance_id = random_id();
        for index in 0..SLOT_COUNT {
            let s = region.slot(index);
            let state = s.state.load(Ordering::Acquire);
            let won = match state {
                SLOT_FREE => s
                    .state
                    .compare_exchange(
                        SLOT_FREE,
                        SLOT_CLAIMING,
                        Ordering::AcqRel,
                        Ordering::Relaxed,
                    )
                    .is_ok(),
                SLOT_ACTIVE => {
                    let pid = s.owner_pid.load(Ordering::Relaxed);
                    let start = s.owner_start_time.load(Ordering::Relaxed);
                    !process::is_alive(pid, start)
                        && s.state
                            .compare_exchange(
                                SLOT_ACTIVE,
                                SLOT_CLAIMING,
                                Ordering::AcqRel,
                                Ordering::Relaxed,
                            )
                            .is_ok()
                }
                _ => false,
            };
            if !won {
                continue;
            }
            let claim = SlotClaim {
                region,
                index,
                meta_lock: Mutex::new(()),
            };
            claim.write_meta(|s| {
                let r = Ordering::Relaxed;
                s.owner_pid.store(process::current_pid(), r);
                s.owner_start_time.store(process::current_start_time(), r);
                s.instance_id.store(instance_id, r);
                s.sample_rate.store(sample_rate, r);
                s.channels.store(channels, r);
                s.flags.store(SLOT_FLAG_SEND_ENABLED, r);
                store_name(s, name);
                s.write_pos.store(0, r);
                s.producer_heartbeat_ns.store(0, r);
                s.generation.fetch_add(1, r);
            });
            claim.slot().state.store(SLOT_ACTIVE, Ordering::Release);
            return Ok(claim);
        }
        Err(ClaimError::Full)
    }

    pub fn index(&self) -> usize {
        self.index
    }

    pub fn region(&self) -> &Arc<Region> {
        &self.region
    }

    fn slot(&self) -> &SlotHeader {
        self.region.slot(self.index)
    }

    fn write_meta(&self, f: impl FnOnce(&SlotHeader)) {
        let _guard = self.meta_lock.lock().unwrap_or_else(|e| e.into_inner());
        let s = self.slot();
        s.seq.fetch_add(1, Ordering::Relaxed); // impair : modification en cours
        fence(Ordering::Release);
        f(s);
        s.seq.fetch_add(1, Ordering::Release); // pair : stable
    }

    /// Change le format (fréquence, canaux). Remet le ring à zéro et incrémente la
    /// génération : l'appli recrée son lecteur.
    ///
    /// L'appelant garantit qu'aucun `Producer` de ce slot n'écrit pendant l'appel
    /// (nih-plug n'appelle jamais `initialize()` en même temps que `process()`).
    pub fn reconfigure(&self, sample_rate: u32, channels: u32) -> Result<(), ClaimError> {
        check_format(sample_rate, channels)?;
        self.write_meta(|s| {
            let r = Ordering::Relaxed;
            s.sample_rate.store(sample_rate, r);
            s.channels.store(channels, r);
            s.write_pos.store(0, r);
            s.generation.fetch_add(1, r);
        });
        Ok(())
    }

    /// Renomme la source (tronqué à 64 octets UTF-8).
    pub fn set_name(&self, name: &str) {
        self.write_meta(|s| store_name(s, name));
    }

    /// Crée le producteur à utiliser sur le thread audio. Un seul à la fois par slot ;
    /// à recréer après `reconfigure`.
    pub fn producer(&self) -> Producer {
        let channels = self.slot().channels.load(Ordering::Relaxed) as usize;
        Producer::new(self.region.clone(), self.index, channels)
    }

    /// `true` si l'appli lit cette source mais l'a coupée (mute). Non bloquant.
    pub fn muted_by_app(&self) -> bool {
        self.slot().consumer_flags.load(Ordering::Relaxed) & CONSUMER_FLAG_MUTED != 0
    }

    /// État de la liaison avec l'appli. Non bloquant, utilisable partout.
    pub fn status(&self) -> LinkStatus {
        let now = clock::now_ns();
        if !self.region.app_alive(now) {
            return LinkStatus::AppAbsent;
        }
        let s = self.slot();
        let hb = s.consumer_heartbeat_ns.load(Ordering::Acquire);
        let listening = s.consumer_flags.load(Ordering::Relaxed) & CONSUMER_FLAG_LISTENING != 0;
        if listening && now.saturating_sub(hb) < CONSUMER_ALIVE_TIMEOUT_NS {
            LinkStatus::Listening
        } else {
            LinkStatus::AppConnected
        }
    }
}

impl Drop for SlotClaim {
    fn drop(&mut self) {
        self.slot().state.store(SLOT_FREE, Ordering::Release);
    }
}

fn store_name(s: &SlotHeader, name: &str) {
    for (dst, b) in s.name.iter().zip(encode_name(name)) {
        dst.store(b, Ordering::Relaxed);
    }
}

fn check_format(sample_rate: u32, channels: u32) -> Result<(), ClaimError> {
    if (MIN_SAMPLE_RATE..=MAX_SAMPLE_RATE).contains(&sample_rate)
        && (1..=MAX_CHANNELS as u32).contains(&channels)
    {
        Ok(())
    } else {
        Err(ClaimError::InvalidFormat)
    }
}

/// Identifiant pseudo-aléatoire, sans dépendance : horloge + PID + adresse de pile, mélangés.
fn random_id() -> u64 {
    let local = 0u8;
    let mut x =
        clock::now_ns() ^ (u64::from(process::current_pid()) << 32) ^ (&local as *const u8 as u64);
    // splitmix64
    x = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^ (x >> 31)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::region::tests::TestRegion;

    #[test]
    fn claim_publish_and_release() {
        let t = TestRegion::new();
        let region = t.open();
        assert!(active_sources(&region).is_empty());

        let a = SlotClaim::claim(region.clone(), 48_000, 2, "Guitare").unwrap();
        let b = SlotClaim::claim(region.clone(), 44_100, 1, "Voix").unwrap();
        assert_ne!(a.index(), b.index());

        let sources = active_sources(&region);
        assert_eq!(sources.len(), 2);
        let g = sources.iter().find(|m| m.name == "Guitare").unwrap();
        assert_eq!((g.sample_rate, g.channels), (48_000, 2));

        let gen_before = g.generation;
        a.reconfigure(96_000, 2).unwrap();
        let g2 = read_meta(&region, a.index()).unwrap();
        assert_eq!(g2.sample_rate, 96_000);
        assert_ne!(g2.generation, gen_before);

        a.set_name("Guitare lead");
        assert_eq!(read_meta(&region, a.index()).unwrap().name, "Guitare lead");

        drop(a);
        let sources = active_sources(&region);
        assert_eq!(sources.len(), 1);
        assert_eq!(sources[0].name, "Voix");
    }

    #[test]
    fn invalid_format_is_refused() {
        let t = TestRegion::new();
        let region = t.open();
        assert_eq!(
            SlotClaim::claim(region.clone(), 48_000, 6, "x").unwrap_err(),
            ClaimError::InvalidFormat
        );
        assert_eq!(
            SlotClaim::claim(region, 0, 2, "x").unwrap_err(),
            ClaimError::InvalidFormat
        );
    }

    #[test]
    fn full_registry() {
        let t = TestRegion::new();
        let region = t.open();
        let claims: Vec<_> = (0..SLOT_COUNT)
            .map(|i| SlotClaim::claim(region.clone(), 48_000, 2, &format!("s{i}")).unwrap())
            .collect();
        assert_eq!(
            SlotClaim::claim(region.clone(), 48_000, 2, "trop").unwrap_err(),
            ClaimError::Full
        );
        drop(claims);
        assert!(SlotClaim::claim(region, 48_000, 2, "ok").is_ok());
    }

    #[test]
    fn dead_owner_is_reaped() {
        let t = TestRegion::new();
        let region = t.open();
        let c = SlotClaim::claim(region.clone(), 48_000, 2, "fantôme").unwrap();
        let idx = c.index();
        std::mem::forget(c); // simule un plantage : le slot reste ACTIVE
        // Un PID quasi certainement inexistant.
        region
            .slot(idx)
            .owner_pid
            .store(u32::MAX - 7, Ordering::Relaxed);
        assert!(active_sources(&region).is_empty());
        assert_eq!(reap_dead(&region), 1);
        assert_eq!(region.slot(idx).state.load(Ordering::Relaxed), SLOT_FREE);
    }

    #[test]
    fn name_truncation_respects_utf8() {
        let long = "é".repeat(40); // 80 octets
        let enc = encode_name(&long);
        let dec = decode_name(&enc);
        assert_eq!(dec, "é".repeat(32));
    }

    #[test]
    fn link_status() {
        let t = TestRegion::new();
        let region = t.open();
        let c = SlotClaim::claim(region.clone(), 48_000, 2, "g").unwrap();
        assert_eq!(c.status(), LinkStatus::AppAbsent);
        region.app_announce();
        assert_eq!(c.status(), LinkStatus::AppConnected);
        mark_consumer(&region, c.index(), CONSUMER_FLAG_LISTENING);
        assert_eq!(c.status(), LinkStatus::Listening);
    }
}
