use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

pub struct AudioPlayer {
    _stream: cpal::Stream,
    samples_played: Arc<AtomicUsize>,
    sample_rate: u32,
}

impl AudioPlayer {
    pub fn play(samples: Vec<f32>, sample_rate: u32, channels: u16) -> Self {
        let host = cpal::default_host();
        let device = host.default_output_device().expect("no hay dispositivo de salida");

        let config = cpal::StreamConfig {
            channels,
            sample_rate,
            buffer_size: cpal::BufferSize::Default,
        };

        let samples_played = Arc::new(AtomicUsize::new(0));
        let samples_played_clone = samples_played.clone();

        let mut position = 0usize;

        let stream = device.build_output_stream(&config,
            move | data: &mut [f32], _| {
                for sample in data.iter_mut() {
                    *sample = if position < samples.len() {
                        samples[position]
                    } else {
                        0.0
                    };
                    position += 1;
                }

                samples_played_clone.store(position / channels as usize, Ordering::Relaxed);
            },
            move |err| eprint!("error de stream de audio: {}", err),
            None,
        ).expect("no se pudo contruir el stream");

        stream.play().expect("no se pudo iniciar el stream");

        Self { _stream: stream, samples_played, sample_rate }
    }

    pub fn position_ms(&self) -> f64 {
        let samples = self.samples_played.load(Ordering::Relaxed);
        (samples as f64 / self.sample_rate as f64) * 1000.0
    }
}

