//! Banc d'essai du lecteur, pour le fuzzing (`fuzz/`) et les tests.
//!
//! L'appli lit en continu des données écrites par des plugins qu'elle ne contrôle pas.
//! [`exercise_reader`] écrit des octets arbitraires dans un slot, comme le ferait un
//! plugin bogué ou malveillant, puis passe par tout ce que l'appli en lit. Aucune entrée
//! ne doit provoquer de panique ni de lecture hors de la zone.
//!
//! Compilé seulement pour les tests ou avec la feature `fuzzing` : ce code écrit dans la
//! zone partagée et n'a rien à faire dans le plugin ni dans l'appli.

use std::sync::Arc;
use std::sync::atomic::Ordering;

use crate::layout::*;
use crate::region::Region;
use crate::registry::read_meta;
use crate::ring::Consumer;

/// Plus grand bloc lu d'un coup, comme un callback audio.
const MAX_FRAMES: usize = 4096;
/// Échantillons de l'anneau remplis à partir de l'entrée, au plus.
const MAX_RING_WRITES: usize = 4096;

/// Format de l'entrée : 1 octet de numéro de slot, puis l'en-tête du slot
/// (`SLOT_HEADER_SIZE` octets au plus), puis 2 octets de taille de bloc, puis des
/// échantillons (4 octets chacun) écrits au début de l'anneau.
pub fn exercise_reader(region: &Arc<Region>, data: &[u8]) {
    let Some((&sel, rest)) = data.split_first() else {
        return;
    };
    let index = sel as usize % SLOT_COUNT;
    let (head, rest) = rest.split_at(rest.len().min(SLOT_HEADER_SIZE));

    let slot = region.slot(index);
    let ptr = std::ptr::from_ref(slot).cast::<u8>().cast_mut();
    // SAFETY : SlotHeader n'est fait que d'atomiques (UnsafeCell), écrire ses octets par
    // un pointeur brut est donc permis ; aucun autre fil ne lit cette zone de test, et
    // head.len() <= SLOT_HEADER_SIZE == size_of::<SlotHeader>() (vérifié dans layout.rs).
    unsafe {
        std::ptr::write_bytes(ptr, 0, SLOT_HEADER_SIZE);
        std::ptr::copy_nonoverlapping(head.as_ptr(), ptr, head.len());
    }

    let (frames, samples) = match rest {
        [a, b, tail @ ..] => (
            usize::from(u16::from_le_bytes([*a, *b])) % (MAX_FRAMES + 1),
            tail,
        ),
        _ => (256, &[][..]),
    };
    let ring = region.ring(index);
    for (dst, src) in ring
        .iter()
        .zip(samples.as_chunks::<4>().0.iter().take(MAX_RING_WRITES))
    {
        dst.store(u32::from_le_bytes(*src), Ordering::Relaxed);
    }

    // Ce que fait l'appli : métadonnées validées, puis lecture de l'anneau.
    let Some(meta) = read_meta(region, index) else {
        return;
    };
    assert!((MIN_SAMPLE_RATE..=MAX_SAMPLE_RATE).contains(&meta.sample_rate));
    assert!((1..=MAX_CHANNELS as u32).contains(&meta.channels));
    assert!(meta.name.len() <= NAME_LEN * 4);
    let _ = crate::process::is_alive(meta.owner_pid, meta.owner_start_time);

    let mut consumer = Consumer::new(region.clone(), &meta);
    let mut buf = vec![0.0f32; frames * 2];
    let _ = consumer.read_stereo(&mut buf, frames);
    consumer.seek_behind_writer(frames as u64);
    let _ = consumer.available();
    let _ = consumer.is_stale();
    let _ = consumer.producer_age_ns(0);
    let _ = consumer.producer_age_ns(u64::MAX);
    let _ = consumer.read_stereo(&mut buf, frames);
    consumer.seek_behind_writer(u64::MAX);
    let _ = consumer.read_stereo(&mut buf, frames);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::region::tests::TestRegion;

    /// Quelques milliers d'entrées pseudo-aléatoires (reproductibles), dont une partie
    /// avec un slot plausible pour passer la validation et atteindre la lecture.
    #[test]
    fn reader_survives_arbitrary_bytes() {
        let t = TestRegion::new();
        let region = t.open();
        let mut x: u64 = 0x9E37_79B9_7F4A_7C15;
        let mut next = move || {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            x
        };
        for round in 0..3000 {
            let len = (next() % 2048) as usize;
            let mut data: Vec<u8> = (0..len).map(|_| next() as u8).collect();
            if round % 2 == 0 && data.len() > 1 + SLOT_HEADER_SIZE + 2 {
                // Slot actif, seq pair, fréquence et canaux valides : la lecture s'exécute.
                let h = &mut data[1..1 + SLOT_HEADER_SIZE];
                h[0..4].copy_from_slice(&0u32.to_le_bytes()); // seq
                h[4..8].copy_from_slice(&SLOT_ACTIVE.to_le_bytes()); // state
                h[12..16].copy_from_slice(&48_000u32.to_le_bytes()); // sample_rate
                h[32..36].copy_from_slice(&(1 + round as u32 % 2).to_le_bytes()); // channels
            }
            exercise_reader(&region, &data);
        }
    }
}
