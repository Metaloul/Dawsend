//! Appli console Dawsend (étapes 1 à 3) : liste les sources publiées par les plugins
//! et en joue une sur une sortie audio normale du système.

use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use dawsend_app::engine::mixer::{Command, MixerControl, mixer};
use dawsend_app::engine::source::{SourceKey, SourcePlayer, SourceStats};
use dawsend_app::output;
use dawsend_core::layout::CONSUMER_FLAG_LISTENING;
use dawsend_core::registry::{self, SlotMeta};
use dawsend_core::{Consumer, LinkStatus, Region, SlotClaim, clock};

const USAGE: &str = "\
Dawsend console — envoie le son des plugins Dawsend vers une sortie audio normale.

UTILISATION :
  dawsend-console play [--source NOM] [--device NOM] [--latency MS]
      Joue une source (par défaut la première trouvée) sur la sortie choisie
      (par défaut celle de Windows). Ctrl+C pour quitter.
  dawsend-console list
      Liste les sources publiées par les plugins.
  dawsend-console devices
      Liste les périphériques de sortie.
  dawsend-console sine [--name NOM] [--rate HZ]
      Publie un bip de test de 440 Hz comme le ferait un plugin (pour tester sans DAW).

OPTIONS :
  --source NOM   nom de la source (insensible à la casse)
  --device NOM   partie du nom du périphérique de sortie
  --latency MS   marge visée dans le buffer, en ms (défaut : 10)
";

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
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
        std::process::exit(1);
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

fn check_args(args: &[String], allowed: &[&str]) -> Result<(), String> {
    let mut it = args.iter();
    while let Some(a) = it.next() {
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

/// Choisit la source à jouer : par nom si demandé, sinon la première.
fn pick_source<'a>(sources: &'a [SlotMeta], wanted: Option<&str>) -> Option<&'a SlotMeta> {
    match wanted {
        Some(w) => sources.iter().find(|m| m.name.eq_ignore_ascii_case(w)),
        None => sources.first(),
    }
}

struct Current {
    key: SourceKey,
    name: String,
    stats: Arc<SourceStats>,
    source_rate: u32,
}

fn play(args: &[String]) -> Result<(), String> {
    check_args(args, &["--source", "--device", "--latency"])?;
    let wanted = opt(args, "--source")?;
    let device = opt(args, "--device")?;
    let latency_ms: f64 = match opt(args, "--latency")? {
        Some(v) => v
            .parse()
            .ok()
            .filter(|v: &f64| (1.0..=200.0).contains(v))
            .ok_or("--latency attend un nombre de ms entre 1 et 200")?,
        None => 10.0,
    };

    let region = open_region()?;
    region.app_announce();

    let (mixer, mut control) = mixer();
    let (_stream, info) = output::start(device, mixer)?;
    println!("Sortie : {info}");
    println!("Marge visée : {latency_ms} ms. Ctrl+C pour quitter.\n");

    let mut current: Option<Current> = None;
    let mut waiting_msg_shown = false;
    let mut last_report = Instant::now();
    let mut last = (0u64, 0u64);

    loop {
        region.app_heartbeat(current.is_some());
        registry::reap_dead(&region);
        let sources = registry::active_sources(&region);
        let chosen = pick_source(&sources, wanted);

        // Le callback a rendu des lecteurs (slot libéré ou reconfiguré) ?
        for key in control.collect_garbage() {
            if current.as_ref().is_some_and(|c| c.key == key) {
                current = None;
            }
        }

        let chosen_key = chosen.map(|m| SourceKey {
            index: m.index,
            generation: m.generation,
        });
        if current.as_ref().map(|c| c.key) != chosen_key {
            if let Some(c) = current.take() {
                send(&mut control, Command::Remove(c.key));
                println!("■ Source « {} » arrêtée.", c.name);
            }
            if let Some(m) = chosen {
                let player = SourcePlayer::new(
                    Consumer::new(region.clone(), m),
                    info.sample_rate,
                    latency_ms,
                );
                let c = Current {
                    key: player.key(),
                    name: m.name.clone(),
                    stats: player.stats(),
                    source_rate: player.source_rate(),
                };
                let effective_ms = player.effective_latency_ms();
                let anti_alias = player.has_anti_alias();
                send(&mut control, Command::Add(Box::new(player)));
                println!(
                    "▶ Lecture de « {} » ({} Hz, {} canal(aux)) → {} Hz{}",
                    c.name,
                    c.source_rate,
                    m.channels,
                    info.sample_rate,
                    if anti_alias {
                        " (filtre anti-repliement actif)"
                    } else {
                        ""
                    }
                );
                if effective_ms + 0.05 < latency_ms {
                    println!(
                        "  Marge réduite à {effective_ms:.1} ms pour tenir dans le buffer à cette fréquence."
                    );
                }
                current = Some(c);
                waiting_msg_shown = false;
                last = (0, 0);
            } else if !waiting_msg_shown {
                match wanted {
                    Some(w) => println!("… En attente de la source « {w} »."),
                    None => println!("… En attente d'une source (plugin Dawsend dans le DAW)."),
                }
                waiting_msg_shown = true;
            }
        }

        if let Some(c) = &current {
            registry::mark_consumer(&region, c.key.index, CONSUMER_FLAG_LISTENING);
            if last_report.elapsed() >= Duration::from_secs(1) {
                last_report = Instant::now();
                let s = &c.stats;
                let under = s.underruns.load(Ordering::Relaxed);
                let over = s.overruns.load(Ordering::Relaxed);
                let fill_ms = s.fill_frames.load(Ordering::Relaxed) as f64 * 1000.0
                    / f64::from(c.source_rate);
                let state = if s.playing.load(Ordering::Relaxed) == 1 {
                    "lecture"
                } else {
                    "attente de données"
                };
                let mut line = format!(
                    "  [{}] {state:<18} buffer {fill_ms:5.1} ms   underruns {under}   resynchros {over}",
                    c.name
                );
                if (under, over) != last && last != (0, 0) {
                    line.push_str("   ⚠");
                }
                last = (under, over);
                println!("{line}");
            }
        }

        std::thread::sleep(Duration::from_millis(100));
    }
}

fn send(control: &mut MixerControl, mut cmd: Command) {
    // La file contient 64 places et le callback la vide à chaque période :
    // si elle est pleine, on patiente un instant.
    for _ in 0..50 {
        match control.send(cmd) {
            Ok(()) => return,
            Err(c) => {
                cmd = c;
                std::thread::sleep(Duration::from_millis(10));
            }
        }
    }
    eprintln!("[console] la sortie audio ne répond plus (commande ignorée)");
}

/// Simule un plugin : publie un bip de 440 Hz en temps réel.
fn sine(args: &[String]) -> Result<(), String> {
    check_args(args, &["--name", "--rate"])?;
    let name = opt(args, "--name")?.unwrap_or("Test 440 Hz");
    let rate: u32 = match opt(args, "--rate")? {
        Some(v) => v.parse().map_err(|_| "--rate attend une fréquence en Hz")?,
        None => 48_000,
    };
    let region = open_region()?;
    let claim = SlotClaim::claim(region.clone(), rate, 2, name).map_err(|e| e.to_string())?;
    let producer = claim.producer();
    println!(
        "Source de test « {name} » publiée ({rate} Hz, emplacement {}). Ctrl+C pour arrêter.",
        claim.index() + 1
    );

    const BLOCK: usize = 128;
    let block_dur = Duration::from_secs_f64(BLOCK as f64 / f64::from(rate));
    let mut phase = 0.0f64;
    let inc = 440.0 * std::f64::consts::TAU / f64::from(rate);
    let mut l = vec![0.0f32; BLOCK];
    let mut r = vec![0.0f32; BLOCK];
    let start = Instant::now();
    let mut blocks: u64 = 0;
    let mut last_status = None;
    loop {
        for i in 0..BLOCK {
            // 0,5 s de bip, 0,5 s de silence.
            let on = (blocks * BLOCK as u64 + i as u64) % u64::from(rate) < u64::from(rate) / 2;
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
        // Cadence régulière, sans dérive cumulée.
        let due = start + block_dur * blocks as u32;
        if let Some(wait) = due.checked_duration_since(Instant::now()) {
            std::thread::sleep(wait);
        }
    }
}
