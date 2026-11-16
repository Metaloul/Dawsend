//! Mémoire partagée nommée, isolée par plateforme.
//!
//! Chaque backend fournit `Mapping::open_or_create(name, len)`, qui ouvre la zone si
//! elle existe déjà ou la crée remplie de zéros sinon. La zone reste mappée jusqu'au
//! `Drop` du `Mapping`.

use std::ptr::NonNull;

#[cfg(windows)]
mod windows;
#[cfg(windows)]
use self::windows as imp;

#[cfg(unix)]
mod unix;
#[cfg(unix)]
use self::unix as imp;

/// Nom par défaut de la zone partagée v1 pour la plateforme courante.
pub fn default_name() -> String {
    imp::default_name()
}

/// Une vue mappée en lecture/écriture sur une zone partagée nommée.
pub struct Mapping {
    ptr: NonNull<u8>,
    len: usize,
    inner: imp::Handle,
}

// SAFETY : le mapping est une simple plage d'adresses ; toute la synchronisation
// passe par des atomiques dans la zone elle-même.
unsafe impl Send for Mapping {}
unsafe impl Sync for Mapping {}

impl Mapping {
    /// Ouvre ou crée la zone `name` d'au moins `len` octets.
    ///
    /// Peut faire des appels système : à n'appeler que hors du thread audio.
    pub fn open_or_create(name: &str, len: usize) -> std::io::Result<Self> {
        let (ptr, inner) = imp::open_or_create(name, len)?;
        Ok(Self { ptr, len, inner })
    }

    pub fn as_ptr(&self) -> *mut u8 {
        self.ptr.as_ptr()
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
}

impl Drop for Mapping {
    fn drop(&mut self) {
        imp::close(self.ptr, self.len, &mut self.inner);
    }
}

/// Supprime le nom de la zone (Unix uniquement ; sans effet sous Windows, où la zone
/// disparaît d'elle-même quand plus aucun processus ne l'a ouverte). Sert aux tests.
pub fn unlink(name: &str) {
    imp::unlink(name);
}
