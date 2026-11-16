//! Backend Windows : file mapping adossé au fichier d'échange (`CreateFileMappingW`).

use std::io;
use std::ptr::NonNull;

use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, INVALID_HANDLE_VALUE};
use windows_sys::Win32::System::Memory::{
    CreateFileMappingW, FILE_MAP_ALL_ACCESS, MEMORY_MAPPED_VIEW_ADDRESS, MapViewOfFile,
    PAGE_READWRITE, UnmapViewOfFile,
};

pub struct Handle(HANDLE);

pub fn default_name() -> String {
    // `Local\` : espace de noms de la session de l'utilisateur, aucun privilège requis.
    format!("Local\\Dawsend.v{}", crate::layout::LAYOUT_VERSION)
}

pub fn open_or_create(name: &str, len: usize) -> io::Result<(NonNull<u8>, Handle)> {
    let wide: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
    let size = len as u64;
    // SAFETY : appel FFI avec un nom terminé par NUL ; INVALID_HANDLE_VALUE demande un
    // mapping adossé au fichier d'échange. Si le nom existe déjà, la zone existante est
    // ouverte (GetLastError == ERROR_ALREADY_EXISTS), ce qui est le comportement voulu.
    let handle = unsafe {
        CreateFileMappingW(
            INVALID_HANDLE_VALUE,
            std::ptr::null(),
            PAGE_READWRITE,
            (size >> 32) as u32,
            size as u32,
            wide.as_ptr(),
        )
    };
    if handle.is_null() {
        return Err(io::Error::last_os_error());
    }
    // SAFETY : `handle` est un handle de mapping valide.
    let view: MEMORY_MAPPED_VIEW_ADDRESS =
        unsafe { MapViewOfFile(handle, FILE_MAP_ALL_ACCESS, 0, 0, len) };
    let Some(ptr) = NonNull::new(view.Value as *mut u8) else {
        let err = io::Error::last_os_error();
        // SAFETY : handle valide, fermé une seule fois.
        unsafe { CloseHandle(handle) };
        return Err(err);
    };
    Ok((ptr, Handle(handle)))
}

pub fn close(ptr: NonNull<u8>, _len: usize, handle: &mut Handle) {
    // SAFETY : `ptr` vient de MapViewOfFile et `handle` de CreateFileMappingW ; appelés
    // une seule fois, depuis `Drop`.
    unsafe {
        UnmapViewOfFile(MEMORY_MAPPED_VIEW_ADDRESS {
            Value: ptr.as_ptr().cast(),
        });
        CloseHandle(handle.0);
    }
}

pub fn unlink(_name: &str) {}
