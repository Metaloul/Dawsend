//! Backend Unix (Linux, macOS) : `shm_open` + `mmap`.

use std::ffi::CString;
use std::io;
use std::ptr::NonNull;

pub struct Handle;

pub fn default_name() -> String {
    // Sous macOS, un nom shm_open est limité à 31 caractères : rester court.
    // SAFETY : getuid ne peut pas échouer.
    let uid = unsafe { libc::getuid() };
    format!("/dawsend.v{}.{}", crate::layout::LAYOUT_VERSION, uid)
}

fn c_name(name: &str) -> io::Result<CString> {
    CString::new(name).map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "nom invalide"))
}

pub fn open_or_create(name: &str, len: usize) -> io::Result<(NonNull<u8>, Handle)> {
    let cname = c_name(name)?;
    // SAFETY : FFI avec une chaîne terminée par NUL.
    let fd = unsafe { libc::shm_open(cname.as_ptr(), libc::O_RDWR | libc::O_CREAT, 0o600) };
    if fd < 0 {
        return Err(io::Error::last_os_error());
    }
    let result = (|| {
        // SAFETY : fd valide, `st` initialisé par fstat.
        let mut st: libc::stat = unsafe { std::mem::zeroed() };
        if unsafe { libc::fstat(fd, &mut st) } != 0 {
            return Err(io::Error::last_os_error());
        }
        if (st.st_size as u64) < len as u64 {
            // Agrandir une zone fraîchement créée : les nouveaux octets valent zéro.
            // Deux processus qui le font en même temps choisissent la même taille.
            // SAFETY : fd valide.
            if unsafe { libc::ftruncate(fd, len as libc::off_t) } != 0 {
                return Err(io::Error::last_os_error());
            }
        }
        // SAFETY : fd valide, taille ≥ len.
        let p = unsafe {
            libc::mmap(
                std::ptr::null_mut(),
                len,
                libc::PROT_READ | libc::PROT_WRITE,
                libc::MAP_SHARED,
                fd,
                0,
            )
        };
        if p == libc::MAP_FAILED {
            return Err(io::Error::last_os_error());
        }
        Ok(NonNull::new(p.cast::<u8>()).expect("mmap ne renvoie pas NULL en cas de succès"))
    })();
    // Le mapping reste valide après la fermeture du descripteur.
    // SAFETY : fd valide, fermé une seule fois.
    unsafe { libc::close(fd) };
    result.map(|p| (p, Handle))
}

pub fn close(ptr: NonNull<u8>, len: usize, _handle: &mut Handle) {
    // SAFETY : plage obtenue par mmap avec cette longueur ; appelé une seule fois.
    unsafe { libc::munmap(ptr.as_ptr().cast(), len) };
}

pub fn unlink(name: &str) {
    if let Ok(cname) = c_name(name) {
        // SAFETY : FFI avec une chaîne terminée par NUL.
        unsafe { libc::shm_unlink(cname.as_ptr()) };
    }
}
