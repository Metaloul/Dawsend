//! Identité et vivacité des processus.
//!
//! Un PID peut être réutilisé par le système après la mort d'un processus. On garde
//! donc aussi l'« heure de création » du processus, dans une unité propre à la
//! plateforme : elle ne sert qu'à être comparée à elle-même.

/// PID du processus courant.
pub fn current_pid() -> u32 {
    std::process::id()
}

/// Heure de création du processus courant (0 si inconnue).
pub fn current_start_time() -> u64 {
    start_time(current_pid()).unwrap_or(0)
}

/// `true` si le processus `pid` existe et a bien été créé à `start_time`
/// (`start_time == 0` : on ne vérifie que l'existence).
pub fn is_alive(pid: u32, start_time: u64) -> bool {
    if pid == 0 {
        return false;
    }
    match start_time_or_alive(pid) {
        Liveness::Dead => false,
        Liveness::AliveUnknownStart => true,
        Liveness::Alive(t) => start_time == 0 || t == 0 || t == start_time,
    }
}

/// Heure de création du processus `pid`, s'il existe.
pub fn start_time(pid: u32) -> Option<u64> {
    match start_time_or_alive(pid) {
        Liveness::Alive(t) => Some(t),
        Liveness::AliveUnknownStart => Some(0),
        Liveness::Dead => None,
    }
}

enum Liveness {
    Dead,
    AliveUnknownStart,
    Alive(u64),
}

#[cfg(windows)]
fn start_time_or_alive(pid: u32) -> Liveness {
    use windows_sys::Win32::Foundation::{CloseHandle, FILETIME, STILL_ACTIVE};
    use windows_sys::Win32::System::Threading::{
        GetExitCodeProcess, GetProcessTimes, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
    };

    // SAFETY : FFI ; le handle est fermé avant de sortir.
    unsafe {
        let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if h.is_null() {
            // Accès refusé = le processus existe (autre utilisateur) ; sinon il est mort.
            let err = windows_sys::Win32::Foundation::GetLastError();
            return if err == windows_sys::Win32::Foundation::ERROR_ACCESS_DENIED {
                Liveness::AliveUnknownStart
            } else {
                Liveness::Dead
            };
        }
        let mut code = 0u32;
        let alive = GetExitCodeProcess(h, &mut code) != 0 && code == STILL_ACTIVE as u32;
        let zero = FILETIME {
            dwLowDateTime: 0,
            dwHighDateTime: 0,
        };
        let (mut c, mut e, mut k, mut u) = (zero, zero, zero, zero);
        let ok = GetProcessTimes(h, &mut c, &mut e, &mut k, &mut u) != 0;
        CloseHandle(h);
        if !alive {
            Liveness::Dead
        } else if ok {
            Liveness::Alive(((c.dwHighDateTime as u64) << 32) | c.dwLowDateTime as u64)
        } else {
            Liveness::AliveUnknownStart
        }
    }
}

#[cfg(target_os = "linux")]
fn start_time_or_alive(pid: u32) -> Liveness {
    // Champ 22 de /proc/<pid>/stat : heure de démarrage en ticks depuis le boot.
    match std::fs::read_to_string(format!("/proc/{pid}/stat")) {
        Ok(s) => {
            // Le nom du processus (champ 2) est entre parenthèses et peut contenir des espaces.
            let after = s.rfind(')').map(|i| &s[i + 1..]).unwrap_or("");
            // Après « ) » : champ 3 (état) est le 1er, donc le champ 22 est le 20e.
            let t = after
                .split_whitespace()
                .nth(19)
                .and_then(|v| v.parse::<u64>().ok());
            // Un processus zombie (état Z) est mort pour nous.
            if after.split_whitespace().next() == Some("Z") {
                return Liveness::Dead;
            }
            match t {
                Some(t) => Liveness::Alive(t),
                None => Liveness::AliveUnknownStart,
            }
        }
        Err(_) => Liveness::Dead,
    }
}

#[cfg(all(unix, not(target_os = "linux")))]
fn start_time_or_alive(pid: u32) -> Liveness {
    // TODO(étape 6, macOS) : proc_pidinfo pour l'heure de création.
    // SAFETY : signal 0 = test d'existence, n'envoie rien.
    let r = unsafe { libc::kill(pid as libc::pid_t, 0) };
    if r == 0 || std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM) {
        Liveness::AliveUnknownStart
    } else {
        Liveness::Dead
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn current_process_is_alive() {
        let t = current_start_time();
        assert!(is_alive(current_pid(), t));
        if t != 0 {
            assert!(!is_alive(current_pid(), t + 1));
        }
    }

    #[test]
    fn pid_zero_is_dead() {
        assert!(!is_alive(0, 0));
    }
}
