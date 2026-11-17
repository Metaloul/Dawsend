//! Lecture par l'appli de ce qu'un plugin a écrit dans la mémoire partagée : en-tête de
//! slot et anneau audio arbitraires. Voir `dawsend_core::fuzzing`.
#![no_main]

use std::sync::{Arc, OnceLock};

use dawsend_core::Region;
use libfuzzer_sys::fuzz_target;

static REGION: OnceLock<Arc<Region>> = OnceLock::new();

fuzz_target!(|data: &[u8]| {
    let region = REGION.get_or_init(|| {
        // Zone propre à ce processus, jamais celle d'une vraie session.
        let name = format!("/dawsend-fuzz.{}", std::process::id());
        Region::open_named(&name).expect("ouverture de la zone de fuzzing")
    });
    dawsend_core::fuzzing::exercise_reader(region, data);
});
