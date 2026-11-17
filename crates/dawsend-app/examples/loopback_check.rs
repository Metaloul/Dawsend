//! Outil de vérification (Windows) : enregistre ce que joue une sortie audio par la
//! capture « loopback » de WASAPI, puis mesure si une tonalité est présente et si le son
//! contient des coupures ou des craquements.
//!
//! C'est ce que « entend » le partage d'écran de Discord : si la tonalité est captée ici,
//! elle l'est aussi par Discord.
//!
//! ```text
//! cargo run --release -p dawsend-app --example loopback_check -- [SECONDES] [FREQ_HZ] [--device NOM]
//! ```
//!
//! Exemple : `dawsend-console sine` (bip 440 Hz, 0,5 s sur 1 s) puis
//! `dawsend-console play`, puis cet outil pendant 10 s.

use std::time::{Duration, Instant};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, SampleFormat, SizedSample};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut positional = args.iter().filter(|a| !a.starts_with("--"));
    let seconds: f64 = positional
        .next()
        .and_then(|v| v.parse().ok())
        .unwrap_or(10.0);
    let freq: f64 = positional
        .next()
        .and_then(|v| v.parse().ok())
        .unwrap_or(440.0);
    let device = args
        .iter()
        .position(|a| a == "--device")
        .and_then(|i| args.get(i + 1))
        .map(|s| s.to_lowercase());

    let host = cpal::default_host();
    let dev = match &device {
        None => host
            .default_output_device()
            .expect("pas de sortie par défaut"),
        Some(w) => host
            .output_devices()
            .expect("liste des sorties")
            .find(|d| d.to_string().to_lowercase().contains(w))
            .expect("sortie introuvable"),
    };
    let supported = dev.default_output_config().expect("config de sortie");
    let config = supported.config();
    let rate = f64::from(config.sample_rate);
    let channels = usize::from(config.channels);
    println!(
        "Capture loopback de « {dev} » ({} Hz, {channels} canaux) pendant {seconds} s…",
        config.sample_rate
    );

    let capacity = (rate * (seconds + 2.0)) as usize;
    let (tx, mut rx) = rtrb::RingBuffer::<f32>::new(capacity);
    let stream = match supported.sample_format() {
        SampleFormat::F32 => build::<f32>(&dev, &config, channels, tx),
        SampleFormat::I16 => build::<i16>(&dev, &config, channels, tx),
        SampleFormat::I32 => build::<i32>(&dev, &config, channels, tx),
        other => panic!("format non pris en charge : {other:?}"),
    };
    stream.play().expect("démarrage de la capture");
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs_f64(seconds) {
        std::thread::sleep(Duration::from_millis(50));
    }
    drop(stream);

    let mut x = Vec::with_capacity(rx.slots());
    while let Ok(v) = rx.pop() {
        x.push(v);
    }
    report(&x, rate, freq);
}

fn build<T>(
    dev: &cpal::Device,
    config: &cpal::StreamConfig,
    channels: usize,
    mut tx: rtrb::Producer<f32>,
) -> cpal::Stream
where
    T: SizedSample + Send + 'static,
    f32: FromSample<T>,
{
    dev.build_input_stream(
        *config,
        move |data: &[T], _| {
            for frame in data.chunks_exact(channels) {
                // Canal gauche seulement : Dawsend écrit le même signal à gauche et à droite.
                let _ = tx.push(frame[0].to_sample::<f32>());
            }
        },
        |e| eprintln!("erreur de capture : {e}"),
        None,
    )
    .expect("capture loopback impossible")
}

/// Amplitude d'une sinusoïde de fréquence `freq` dans `x` (algorithme de Goertzel).
fn goertzel(x: &[f32], rate: f64, freq: f64) -> f64 {
    let w = std::f64::consts::TAU * freq / rate;
    let coeff = 2.0 * w.cos();
    let (mut s1, mut s2) = (0.0f64, 0.0f64);
    for &v in x {
        let s0 = f64::from(v) + coeff * s1 - s2;
        s2 = s1;
        s1 = s0;
    }
    let power = s1 * s1 + s2 * s2 - coeff * s1 * s2;
    2.0 * power.sqrt() / x.len() as f64
}

fn report(x: &[f32], rate: f64, freq: f64) {
    if x.is_empty() {
        println!("RÉSULTAT : rien capté (aucun son joué sur cette sortie ?)");
        return;
    }
    let win = (rate / 10.0) as usize; // fenêtres de 100 ms
    let windows: Vec<(f64, f64)> = x
        .chunks_exact(win)
        .map(|w| {
            let rms =
                (w.iter().map(|v| f64::from(*v).powi(2)).sum::<f64>() / w.len() as f64).sqrt();
            (rms, goertzel(w, rate, freq))
        })
        .collect();
    let with_tone = windows.iter().filter(|(_, a)| *a > 0.01).count();
    let peak_tone = windows.iter().map(|(_, a)| *a).fold(0.0, f64::max);
    let rms_all = (x.iter().map(|v| f64::from(*v).powi(2)).sum::<f64>() / x.len() as f64).sqrt();

    // Craquements : pour une sinusoïde d'amplitude A, la dérivée seconde reste sous
    // A·(2πf/fs)². Un saut au-delà de plusieurs fois cette valeur est une discontinuité.
    let a = peak_tone.max(1e-6);
    let w = std::f64::consts::TAU * freq / rate;
    let limit = (a * w * w * 8.0).max(a * 0.05);
    let mut clicks = 0usize;
    let mut last_click = 0usize;
    for i in 2..x.len() {
        let d2 = (f64::from(x[i]) - 2.0 * f64::from(x[i - 1]) + f64::from(x[i - 2])).abs();
        if d2 > limit && i - last_click > win / 10 {
            clicks += 1;
            last_click = i;
        }
    }

    println!(
        "Capté : {:.1} s, RMS global {:.4}, amplitude max à {freq} Hz : {peak_tone:.3}",
        x.len() as f64 / rate,
        rms_all
    );
    println!(
        "Fenêtres de 100 ms contenant la tonalité : {with_tone}/{} ({:.0} %)",
        windows.len(),
        100.0 * with_tone as f64 / windows.len().max(1) as f64
    );
    println!("Discontinuités détectées (craquements possibles) : {clicks}");

    // Creux : fenêtres de 10 ms où la tonalité tombe sous la moitié de son niveau médian
    // (utile avec un son continu, `dawsend-console sine --steady`).
    let small = (rate / 100.0) as usize;
    let mut amps: Vec<f64> = x
        .chunks_exact(small)
        .map(|w| goertzel(w, rate, freq))
        .collect();
    let mut sorted = amps.clone();
    sorted.sort_by(f64::total_cmp);
    let median = sorted[sorted.len() / 2];
    if median > 0.01 {
        // La première et la dernière fenêtre peuvent être partielles.
        amps.truncate(amps.len().saturating_sub(1));
        let dips = amps.iter().skip(1).filter(|a| **a < median * 0.5).count();
        println!("Son continu : niveau médian {median:.3}, creux de 10 ms : {dips}");
    }
    let ok = with_tone > 0;
    println!(
        "RÉSULTAT : {}",
        if ok {
            "tonalité présente sur la sortie"
        } else {
            "tonalité ABSENTE"
        }
    );
}
