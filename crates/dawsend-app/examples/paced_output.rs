//! Test sans carte son : joue la première source sur une sortie simulée, cadencée par
//! l'horloge système à 48 kHz (périodes de 10 ms), et affiche les statistiques.
//!
//! Lancer d'abord `dawsend-console sine` dans un autre terminal, puis :
//!   cargo run -p dawsend-app --example paced_output -- [secondes]

use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use dawsend_app::engine::mixer::{Command, mixer};
use dawsend_app::engine::source::SourcePlayer;
use dawsend_core::{Consumer, Region, registry};

fn main() {
    let secs: u64 = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(5);
    let region = Region::open().expect("zone partagée");
    region.app_announce();
    let meta = registry::active_sources(&region)
        .into_iter()
        .next()
        .expect("aucune source : lancer `dawsend-console sine`");
    let (mut mixer, mut control) = mixer();
    let player = SourcePlayer::new(Consumer::new(region.clone(), &meta), 48_000, 10.0);
    let stats = player.stats();
    assert!(control.send(Command::Add(Box::new(player))).is_ok());

    const PERIOD: usize = 480;
    let mut out = vec![0.0f32; PERIOD * 2];
    let start = Instant::now();
    let mut peak = 0.0f32;
    let mut n: u32 = 0;
    while start.elapsed() < Duration::from_secs(secs) {
        mixer.render(&mut out, PERIOD);
        peak = out.iter().fold(peak, |m, v| m.max(v.abs()));
        n += 1;
        if n % 10 == 0 {
            region.app_heartbeat(true);
        }
        let due = start + Duration::from_millis(10) * n;
        if let Some(w) = due.checked_duration_since(Instant::now()) {
            std::thread::sleep(w);
        }
    }
    println!(
        "source « {} » {} Hz : {} périodes, crête {:.3}, underruns {}, resynchros {}, buffer {:.1} ms",
        meta.name,
        meta.sample_rate,
        n,
        peak,
        stats.underruns.load(Ordering::Relaxed),
        stats.overruns.load(Ordering::Relaxed),
        stats.fill_frames.load(Ordering::Relaxed) as f64 * 1000.0 / f64::from(meta.sample_rate),
    );
}
