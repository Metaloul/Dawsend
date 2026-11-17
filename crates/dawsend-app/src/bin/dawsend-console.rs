//! Appli console Dawsend : liste les sources publiées par les plugins, les joue sur une
//! sortie audio normale du système, et publie des sources de test.
//!
//! Outil de diagnostic : l'appli de tous les jours est l'appli à fenêtre (`dawsend`).

use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use dawsend_app::output;
use dawsend_app::service::{Engine, EngineSettings};
use dawsend_core::registry;
use dawsend_core::{LinkStatus, Region, SlotClaim, clock};

const USAGE: &str = "\
Dawsend console — envoie le son des plugins Dawsend vers une sortie audio normale.

UTILISATION :
  dawsend-console play [--source NOM] [--device NOM] [--latency MS]
      Joue les sources (ou seulement celle demandée) sur la sortie choisie
      (par défaut celle de Windows). Ctrl+C pour quitter.
  dawsend-console list
      Liste les sources publiées par les plugins.
  dawsend-console devices
      Liste les périphériques de sortie.
  dawsend-console sine [--name NOM] [--rate HZ] [--freq HZ] [--steady] [--ppm N] [--block N]
                       [--spike MS [--catch-up]]
      Publie un bip de test comme le ferait un plugin (pour tester sans DAW).
      --steady : son continu (sinon 0,5 s sur 1 s) ; --ppm : décale l'horloge du faux
      DAW de N ppm (test de la compensation de dérive) ; --block : taille des blocs.

OPTIONS :
  --source NOM   nom de la source (insensible à la casse)
  --device NOM   partie du nom du périphérique de sortie
  --latency MS   marge visée dans le buffer, en ms (défaut : 10)
  -v             affiche aussi le journal détaillé
";

/// Journal de la console : sur la sortie d'erreur, détaillé avec `-v`.
struct StderrLog;

impl log::Log for StderrLog {
    fn enabled(&self, m: &log::Metadata) -> bool {
        m.level() <= log::max_level()
    }
    fn log(&self, r: &log::Record) {
        if self.enabled(r.metadata()) {
            eprintln!("[{}] {}", r.level(), r.args());
        }
    }
    fn flush(&self) {}
}

fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let verbose = args.iter().any(|a| a == "-v");
    args.retain(|a| a != "-v");
    let _ = log::set_logger(&StderrLog);
    log::set_max_level(if verbose {
        log::LevelFilter::Debug
    } else {
        log::LevelFilter::Warn
    });

    let result = match args.first().map(String::as_str) {
        Some("play") => play(&args[1..]),
        Some("list") => list(),
        Some("devices") => devices(),
        Some("sine") => sine(&args[1..]),
        Some("-h" | "--help" | "help") | None => {
            print!("{USAGE}");
            Ok(())
        }
        Some(other) => Err(format!("commande inconnue : {other}\n\n{USAGE}")),
    };
    if let Err(e) = result {
        eprintln!("Erreur : {e}");
        pause_if_own_window();
        std::process::exit(1);
    }
}

/// Lancée depuis un raccourci, la console a sa propre fenêtre qui se fermerait aussitôt :
/// on attend Entrée pour laisser le temps de lire le message.
fn pause_if_own_window() {
    #[cfg(windows)]
    {
        let mut pids = [0u32; 4];
        // SAFETY : écrit au plus 4 PID dans le tableau local.
        let n = unsafe {
            windows_sys::Win32::System::Console::GetConsoleProcessList(pids.as_mut_ptr(), 4)
        };
        if n == 1 {
            eprintln!("\nAppuie sur Entrée pour fermer.");
            let _ = std::io::stdin().read_line(&mut String::new());
        }
    }
}

/// Lit la valeur de `--name valeur` dans `args`.
fn opt<'a>(args: &'a [String], name: &str) -> Result<Option<&'a str>, String> {
    let mut it = args.iter();
    while let Some(a) = it.next() {
        if a == name {
            return it
                .next()
                .map(|v| Some(v.as_str()))
                .ok_or_else(|| format!("valeur manquante après {name}"));
        }
    }
    Ok(None)
}

/// Vérifie les options ; `flags` sont les options sans valeur.
fn check_args(args: &[String], allowed: &[&str], flags: &[&str]) -> Result<(), String> {
    let mut it = args.iter();
    while let Some(a) = it.next() {
        if flags.contains(&a.as_str()) {
            continue;
        }
        if !allowed.contains(&a.as_str()) {
            return Err(format!("option inconnue : {a}\n\n{USAGE}"));
        }
        it.next();
    }
    Ok(())
}

fn open_region() -> Result<Arc<Region>, String> {
    Region::open().map_err(|e| e.to_string())
}

fn list() -> Result<(), String> {
    let region = open_region()?;
    registry::reap_dead(&region);
    let sources = registry::active_sources(&region);
    if sources.is_empty() {
        println!("Aucune source. Ajoute le plugin Dawsend sur une piste et lance la lecture.");
        return Ok(());
    }
    let now = clock::now_ns();
    println!("{:<4} {:<24} {:>8} {:>7}  État", "#", "Nom", "Hz", "Canaux");
    for m in sources {
        let age = now.saturating_sub(
            region
                .slot(m.index)
                .producer_heartbeat_ns
                .load(Ordering::Relaxed),
        );
        let state = if age < 250 * clock::MS {
            "active"
        } else {
            "en pause (DAW arrêté ou piste inactive)"
        };
        println!(
            "{:<4} {:<24} {:>8} {:>7}  {}",
            m.index + 1,
            m.name,
            m.sample_rate,
            m.channels,
            state
        );
    }
    Ok(())
}

fn devices() -> Result<(), String> {
    for (name, is_default) in output::list_devices()? {
        println!("{} {name}", if is_default { "*" } else { " " });
    }
    println!("\n(* = périphérique par défaut)");
    Ok(())
}

fn play(args: &[String]) -> Result<(), String> {
    check_args(args, &["--source", "--device", "--latency"], &[])?;
    let latency_ms: f64 = match opt(args, "--latency")? {
        Some(v) => v
            .parse()
            .ok()
            .filter(|v: &f64| {
                (EngineSettings::MIN_LATENCY_MS..=EngineSettings::MAX_LATENCY_MS).contains(v)
            })
            .ok_or("--latency attend un nombre de ms entre 3 et 200")?,
        None => 10.0,
    };
    let settings = EngineSettings {
        device: opt(args, "--device")?.map(str::to_owned),
        latency_ms,
        solo: opt(args, "--source")?.map(str::to_owned),
        ..EngineSettings::default()
    };
    let engine = Engine::start(settings)?;
    println!("Marge visée : {latency_ms} ms. Ctrl+C pour quitter.\n");

    let mut last_output = String::new();
    let mut waiting_shown = false;
    loop {
        std::thread::sleep(Duration::from_secs(1));
        let v = engine.view();
        let output = match (&v.output, &v.output_error) {
            (Some(o), Some(e)) => format!("Sortie : {o} ({e})"),
            (Some(o), None) => format!("Sortie : {o}"),
            (None, Some(e)) => format!("Sortie indisponible : {e}"),
            (None, None) => String::new(),
        };
        if !output.is_empty() && output != last_output {
            println!("{output}");
            last_output = output;
        }
        let playing: Vec<_> = v.sources.iter().filter(|s| s.enabled).collect();
        if playing.is_empty() {
            if !waiting_shown {
                println!("… En attente d'une source (plugin Dawsend dans le DAW).");
                waiting_shown = true;
            }
            continue;
        }
        waiting_shown = false;
        for s in playing {
            let state = match (s.active, s.playing) {
                (false, _) => "en pause",
                (true, true) => "lecture",
                (true, false) => "amorçage",
            };
            println!(
                "  [{}] {state:<9} buffer {:5.1}/{:4.1} ms  dérive {:+6.0} ppm  underruns {}  resynchros {}{}",
                s.name,
                s.fill_ms,
                s.target_ms,
                s.correction_ppm,
                s.underruns,
                s.resyncs,
                if s.anti_alias {
                    "  (anti-repliement)"
                } else {
                    ""
                }
            );
        }
    }
}

/// Simule un plugin : publie un bip en temps réel.
fn sine(args: &[String]) -> Result<(), String> {
    check_args(
        args,
        &["--name", "--rate", "--freq", "--ppm", "--block", "--spike"],
        &["--steady", "--catch-up"],
    )?;
    let name = opt(args, "--name")?.unwrap_or("Test 440 Hz");
    let num = |flag: &str, default: f64| -> Result<f64, String> {
        match opt(args, flag)? {
            Some(v) => v.parse().map_err(|_| format!("{flag} attend un nombre")),
            None => Ok(default),
        }
    };
    let rate = num("--rate", 48_000.0)? as u32;
    let freq = num("--freq", 440.0)?;
    let ppm = num("--ppm", 0.0)?;
    let block = (num("--block", 128.0)? as usize).clamp(16, 8192);
    let steady = args.iter().any(|a| a == "--steady");
    // Pic simulé du DAW toutes les 5 s : blocage de `spike` ms, puis blocs perdus (comme
    // un décrochage ASIO) ou rattrapés d'un coup (`--catch-up`).
    let spike = Duration::from_secs_f64(num("--spike", 0.0)?.clamp(0.0, 2000.0) / 1000.0);
    let catch_up = args.iter().any(|a| a == "--catch-up");
    let mut next_spike = Instant::now() + Duration::from_secs(5);
    let mut lost = Duration::ZERO;
    let region = open_region()?;
    let claim = SlotClaim::claim(region.clone(), rate, 2, name).map_err(|e| e.to_string())?;
    let producer = claim.producer();
    println!(
        "Source de test « {name} » publiée ({rate} Hz, {freq} Hz, blocs de {block}, horloge {ppm:+} ppm, emplacement {}). Ctrl+C pour arrêter.",
        claim.index() + 1
    );

    // Horloge simulée du DAW : `rate × (1 + ppm)` trames par seconde réelle.
    let real_rate = f64::from(rate) * (1.0 + ppm * 1e-6);
    let mut phase = 0.0f64;
    let inc = freq * std::f64::consts::TAU / f64::from(rate);
    let mut l = vec![0.0f32; block];
    let mut r = vec![0.0f32; block];
    let start = Instant::now();
    let mut blocks: u64 = 0;
    let mut last_status = None;
    loop {
        for i in 0..block {
            let pos = blocks * block as u64 + i as u64;
            let on = steady || pos % u64::from(rate) < u64::from(rate) / 2;
            let v = if on { (phase.sin() * 0.25) as f32 } else { 0.0 };
            l[i] = v;
            r[i] = v;
            phase = (phase + inc) % std::f64::consts::TAU;
        }
        producer.write_planar(&[&l[..], &r[..]]);
        blocks += 1;
        let status = claim.status();
        if last_status != Some(status) {
            let txt = match status {
                LinkStatus::AppAbsent => "appli Dawsend absente",
                LinkStatus::AppConnected => "appli connectée (n'écoute pas cette source)",
                LinkStatus::Listening => "l'appli écoute cette source",
            };
            println!("  état : {txt}");
            last_status = Some(status);
        }
        if !spike.is_zero() && Instant::now() >= next_spike {
            next_spike += Duration::from_secs(5);
            std::thread::sleep(spike);
            if !catch_up {
                lost += spike;
            }
        }
        // Cadence régulière, sans dérive cumulée.
        let due =
            start + lost + Duration::from_secs_f64((blocks * block as u64) as f64 / real_rate);
        if let Some(wait) = due.checked_duration_since(Instant::now()) {
            std::thread::sleep(wait);
        }
    }
}
