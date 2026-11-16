//! Horloge monotone en nanosecondes, commune à tous les processus de la machine.
//!
//! Utilisable depuis le thread audio : aucun appel bloquant, aucune allocation.
//! Appeler [`init`] une fois hors du thread audio (le plugin le fait dans `initialize()`).

#[cfg(windows)]
mod imp {
    use std::sync::atomic::{AtomicU64, Ordering};
    use windows_sys::Win32::System::Performance::{
        QueryPerformanceCounter, QueryPerformanceFrequency,
    };

    static FREQ: AtomicU64 = AtomicU64::new(0);

    fn freq() -> u64 {
        let f = FREQ.load(Ordering::Relaxed);
        if f != 0 {
            return f;
        }
        let mut v = 0i64;
        // SAFETY : écrit dans une variable locale ; ne peut pas échouer depuis Windows XP.
        unsafe { QueryPerformanceFrequency(&mut v) };
        let v = (v as u64).max(1);
        FREQ.store(v, Ordering::Relaxed);
        v
    }

    pub fn init() {
        freq();
    }

    pub fn now_ns() -> u64 {
        let mut ticks = 0i64;
        // SAFETY : écrit dans une variable locale ; non bloquant.
        unsafe { QueryPerformanceCounter(&mut ticks) };
        ((ticks as u128 * 1_000_000_000) / freq() as u128) as u64
    }
}

#[cfg(unix)]
mod imp {
    pub fn init() {}

    pub fn now_ns() -> u64 {
        let mut ts = libc::timespec {
            tv_sec: 0,
            tv_nsec: 0,
        };
        // SAFETY : écrit dans une variable locale ; CLOCK_MONOTONIC est commun aux
        // processus et passe par le vDSO (pas d'appel système réel).
        unsafe { libc::clock_gettime(libc::CLOCK_MONOTONIC, &mut ts) };
        ts.tv_sec as u64 * 1_000_000_000 + ts.tv_nsec as u64
    }
}

/// Prépare l'horloge (lit la fréquence du compteur sous Windows).
pub fn init() {
    imp::init();
}

/// Temps monotone en ns. Origine arbitraire mais identique pour tous les processus.
pub fn now_ns() -> u64 {
    imp::now_ns()
}

pub const MS: u64 = 1_000_000;
