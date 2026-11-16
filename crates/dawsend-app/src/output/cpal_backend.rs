//! Backend de sortie cpal.

use std::fmt;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, SampleFormat, SizedSample};

use super::Renderer;

/// Taille max d'un morceau rendu d'un coup (le callback est découpé au-delà).
const CHUNK_FRAMES: usize = 2048;

#[derive(Debug, Clone)]
pub struct OutputInfo {
    pub device_name: String,
    pub sample_rate: u32,
    pub channels: u16,
    pub sample_format: String,
}

impl fmt::Display for OutputInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} — {} Hz, {} canaux, {}",
            self.device_name, self.sample_rate, self.channels, self.sample_format
        )
    }
}

/// Flux de sortie en cours. S'arrête au `Drop`.
pub struct OutputStream {
    _stream: cpal::Stream,
}

/// Liste les périphériques de sortie : (nom, est-ce le défaut ?).
pub fn list_devices() -> Result<Vec<(String, bool)>, String> {
    let host = cpal::default_host();
    let default = host.default_output_device().map(|d| d.to_string());
    let devices = host.output_devices().map_err(|e| e.to_string())?;
    Ok(devices
        .map(|d| {
            let name = d.to_string();
            let is_default = Some(&name) == default.as_ref();
            (name, is_default)
        })
        .collect())
}

/// Ouvre le périphérique `device` (sous-chaîne du nom, insensible à la casse) ou celui
/// par défaut, et démarre la lecture avec `renderer`.
pub fn start<R: Renderer>(
    device: Option<&str>,
    renderer: R,
) -> Result<(OutputStream, OutputInfo), String> {
    let host = cpal::default_host();
    let dev = match device {
        None => host
            .default_output_device()
            .ok_or("aucun périphérique de sortie par défaut")?,
        Some(wanted) => {
            let wanted = wanted.to_lowercase();
            host.output_devices()
                .map_err(|e| e.to_string())?
                .find(|d| d.to_string().to_lowercase().contains(&wanted))
                .ok_or_else(|| format!("aucun périphérique de sortie ne contient « {wanted} »"))?
        }
    };
    let supported = dev.default_output_config().map_err(|e| e.to_string())?;
    let config = supported.config();
    let info = OutputInfo {
        device_name: dev.to_string(),
        sample_rate: config.sample_rate,
        channels: config.channels,
        sample_format: format!("{:?}", supported.sample_format()),
    };

    let stream = match supported.sample_format() {
        SampleFormat::F32 => build::<f32, R>(&dev, &config, renderer),
        SampleFormat::F64 => build::<f64, R>(&dev, &config, renderer),
        SampleFormat::I16 => build::<i16, R>(&dev, &config, renderer),
        SampleFormat::I32 => build::<i32, R>(&dev, &config, renderer),
        SampleFormat::U16 => build::<u16, R>(&dev, &config, renderer),
        other => {
            return Err(format!(
                "format d'échantillon non pris en charge : {other:?}"
            ));
        }
    }?;
    stream.play().map_err(|e| e.to_string())?;
    Ok((OutputStream { _stream: stream }, info))
}

fn build<T, R>(
    dev: &cpal::Device,
    config: &cpal::StreamConfig,
    mut renderer: R,
) -> Result<cpal::Stream, String>
where
    T: SizedSample + FromSample<f32> + Send + 'static,
    R: Renderer,
{
    let channels = usize::from(config.channels).max(1);
    // Alloué ici, une fois : le callback n'alloue jamais.
    let mut scratch = vec![0.0f32; CHUNK_FRAMES * 2];
    let silence = T::from_sample(0.0f32);
    dev.build_output_stream(
        *config,
        move |data: &mut [T], _info| {
            let total = data.len() / channels;
            let mut done = 0;
            while done < total {
                let n = (total - done).min(CHUNK_FRAMES);
                renderer.render(&mut scratch[..n * 2], n);
                let dst = &mut data[done * channels..(done + n) * channels];
                for (frame, src) in dst.chunks_exact_mut(channels).zip(scratch.chunks_exact(2)) {
                    if channels == 1 {
                        frame[0] = T::from_sample((src[0] + src[1]) * 0.5);
                    } else {
                        frame[0] = T::from_sample(src[0]);
                        frame[1] = T::from_sample(src[1]);
                        for s in &mut frame[2..] {
                            *s = silence;
                        }
                    }
                }
                done += n;
            }
        },
        |err| eprintln!("[sortie audio] erreur : {err}"),
        None,
    )
    .map_err(|e| e.to_string())
}
