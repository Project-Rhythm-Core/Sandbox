// electron-audio-native/src/audio.rs
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use once_cell::sync::Lazy;

use crate::decoder::DecodedAudio;

/// Tamano de buffer que pedimos al dispositivo. 256 frames a 48 kHz son ~5.3 ms de
/// latencia de salida; el default de ALSA/Pulse puede irse a 40-200 ms.
const DESIRED_BUFFER_FRAMES: u32 = 256;

/// Origen comun para todos los `Instant`, para poder guardarlos en un atomico.
static EPOCH: Lazy<Instant> = Lazy::new(Instant::now);

fn now_nanos() -> u64 {
    EPOCH.elapsed().as_nanos() as u64
}

/// Reloj de reproduccion anclado al instante en que la muestra 0 se **oye**,
/// no a cuantas muestras se han copiado al buffer del dispositivo.
struct PlaybackClock {
    /// Se pone a true cuando el usuario da al play; hasta entonces el stream saca silencio.
    armed: AtomicBool,
    /// `origin_nanos` ya tiene un valor fiable.
    started: AtomicBool,
    /// Instante (nanos desde EPOCH) en que la muestra 0 llega al altavoz.
    origin_nanos: AtomicU64,
    /// Callbacks servidos desde el ultimo arranque, para el suavizado del ancla.
    callbacks: AtomicU64,
    /// El dispositivo ya esta girando y encolando audio por delante. Hasta que no lo
    /// esta, `play()` tarda ~200 ms en sonar porque ALSA sigue levantando el stream.
    warm: AtomicBool,
    /// Peticion de salto en frames; -1 = ninguna. La atiende el hilo de audio.
    seek_frames: AtomicI64,
}

/// Techo del suavizado: a partir de aqui el reloj es un EMA de 1/32 que sigue al
/// reloj del hardware sin dar saltos.
const MAX_SMOOTHING: u64 = 32;

pub struct AudioEngine {
    _stream: cpal::Stream,
    clock: Arc<PlaybackClock>,
    duration_ms: f64,
    /// Config real con la que se abrio el dispositivo, no la que pedimos.
    pub device_name: String,
    pub sample_rate: u32,
    pub channels: u16,
    pub buffer_frames: u32,
    /// Latencia de salida que reporta el backend en el primer callback, en nanos.
    output_latency_ns: Arc<AtomicU64>,
    /// Correccion manual del usuario, en nanos. Positivo = el audio se oye tarde,
    /// asi que la posicion reportada debe ir por detras.
    offset_ns: AtomicI64,
}

// cpal::Stream no es Send en algunos backends. El stream se crea y se destruye
// siempre desde el hilo principal de Node, que es el unico que toca el Mutex global.
unsafe impl Send for AudioEngine {}

impl AudioEngine {
    /// Abre el dispositivo y arranca el stream ya, sacando silencio. Asi el coste de
    /// abrir el device (decenas de ms en Linux) se paga al cargar, no al pulsar play.
    pub fn prepare(audio: DecodedAudio) -> Result<Self, String> {
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .ok_or("no hay dispositivo de salida")?;

        let default_cfg = device
            .default_output_config()
            .map_err(|e| format!("no se pudo leer la config por defecto: {}", e))?;

        let (samples, sample_rate, channels) = adapt_to_device(&device, audio, &default_cfg)?;

        let frames = samples.len() / channels as usize;
        let duration_ms = (frames as f64 / sample_rate as f64) * 1000.0;

        let buffer_size = pick_buffer_size(&device, channels, sample_rate);
        let buffer_frames = match buffer_size {
            cpal::BufferSize::Fixed(n) => n,
            cpal::BufferSize::Default => 0,
        };

        let config = cpal::StreamConfig {
            channels,
            sample_rate: cpal::SampleRate(sample_rate),
            buffer_size,
        };

        let clock = Arc::new(PlaybackClock {
            armed: AtomicBool::new(false),
            started: AtomicBool::new(false),
            origin_nanos: AtomicU64::new(0),
            callbacks: AtomicU64::new(0),
            warm: AtomicBool::new(false),
            seek_frames: AtomicI64::new(-1),
        });
        let output_latency_ns = Arc::new(AtomicU64::new(0));

        let cb_clock = clock.clone();
        let cb_latency = output_latency_ns.clone();
        let ch = channels as usize;
        let rate = sample_rate as u64;
        let total_frames = samples.len() / ch;

        // Estado privado del hilo de audio.
        // `t0`: instante del primer callback, o sea cuando el hardware empieza a consumir.
        // `written`: frames entregados al dispositivo (incluido el silencio previo al play).
        // `cursor`: frames de la cancion ya entregados.
        let mut t0: u64 = 0;
        let mut written: u64 = 0;
        let mut cursor: usize = 0;

        let stream = device
            .build_output_stream(
                &config,
                move |data: &mut [f32], info: &cpal::OutputCallbackInfo| {
                    let now = now_nanos();
                    let frames = (data.len() / ch) as u64;

                    if written == 0 {
                        t0 = now;
                    }

                    // Cuanto falta para que el primer frame de este buffer se oiga.
                    // ALSA a traves de PipeWire/Pulse suele reportar 0 aqui, asi que
                    // tambien lo calculamos por nuestra cuenta: lo ya entregado al
                    // dispositivo tarda `written / rate` en sonar desde `t0`.
                    let ts = info.timestamp();
                    let reported = ts
                        .playback
                        .duration_since(&ts.callback)
                        .unwrap_or_default()
                        .as_nanos() as u64;
                    let measured = (t0 + written * 1_000_000_000 / rate).saturating_sub(now);
                    let ahead = reported.max(measured);
                    let audible_at = now + ahead;

                    // En cuanto el dispositivo encola por delante, esta en regimen.
                    if measured > 0 {
                        cb_clock.warm.store(true, Ordering::Release);
                    }

                    // Saltos pedidos desde JS: reposicionan el cursor y reanclan el reloj.
                    let seek = cb_clock.seek_frames.swap(-1, Ordering::AcqRel);
                    if seek >= 0 {
                        cursor = (seek as usize).min(total_frames);
                        cb_clock.callbacks.store(0, Ordering::Relaxed);
                        cb_clock.started.store(false, Ordering::Release);
                    }

                    if cb_clock.armed.load(Ordering::Acquire) {
                        let cursor_nanos = (cursor as u64) * 1_000_000_000 / rate;
                        let candidate = audible_at.saturating_sub(cursor_nanos);

                        let n = cb_clock.callbacks.fetch_add(1, Ordering::Relaxed);
                        if n == 0 {
                            cb_clock.origin_nanos.store(candidate, Ordering::Relaxed);
                            cb_latency.store(ahead, Ordering::Relaxed);
                            cb_clock.started.store(true, Ordering::Release);
                        } else {
                            // Media movil: rapida al principio para converger en pocos ms,
                            // luego un EMA lento que sigue la deriva del reloj del hardware
                            // sin dar saltos que se notarian en pantalla.
                            let div = (n + 1).min(MAX_SMOOTHING) as i128;
                            let prev = cb_clock.origin_nanos.load(Ordering::Relaxed) as i128;
                            let next = prev + (candidate as i128 - prev) / div;
                            cb_clock.origin_nanos.store(next as u64, Ordering::Relaxed);
                        }

                        let start = cursor * ch;
                        let available = samples.len().saturating_sub(start).min(data.len());
                        data[..available].copy_from_slice(&samples[start..start + available]);
                        data[available..].fill(0.0);

                        cursor = (cursor + frames as usize).min(total_frames);
                    } else {
                        data.fill(0.0);
                    }

                    written += frames;
                },
                move |err| eprintln!("[audio] error de stream: {}", err),
                None,
            )
            .map_err(|e| format!("no se pudo construir el stream: {}", e))?;

        stream
            .play()
            .map_err(|e| format!("no se pudo iniciar el stream: {}", e))?;

        Ok(Self {
            _stream: stream,
            clock,
            duration_ms,
            device_name: device.name().unwrap_or_else(|_| "desconocido".into()),
            sample_rate,
            channels,
            buffer_frames,
            output_latency_ns,
            offset_ns: AtomicI64::new(0),
        })
    }

    /// Arranca la reproduccion. Solo levanta un flag: el audio empieza en el siguiente
    /// callback, es decir un periodo de buffer despues (~5 ms), no cientos.
    pub fn start(&self) {
        self.clock.armed.store(true, Ordering::Release);
    }

    pub fn position_ms(&self) -> f64 {
        if !self.clock.started.load(Ordering::Acquire) {
            return 0.0;
        }
        let origin = self.clock.origin_nanos.load(Ordering::Relaxed) as i128;
        let offset = self.offset_ns.load(Ordering::Relaxed) as i128;
        let elapsed = (now_nanos() as i128 - origin - offset) as f64 / 1_000_000.0;
        elapsed.clamp(0.0, self.duration_ms)
    }

    /// Offset de calibracion en ms. Ninguna API sabe la latencia real de los altavoces
    /// (bluetooth, DSP del amplificador...), asi que esto se ajusta a oido.
    pub fn set_offset_ms(&self, ms: f64) {
        self.offset_ns
            .store((ms * 1_000_000.0) as i64, Ordering::Relaxed);
    }

    pub fn offset_ms(&self) -> f64 {
        self.offset_ns.load(Ordering::Relaxed) as f64 / 1_000_000.0
    }

    pub fn duration_ms(&self) -> f64 {
        self.duration_ms
    }

    pub fn output_latency_ms(&self) -> f64 {
        self.output_latency_ns.load(Ordering::Relaxed) as f64 / 1_000_000.0
    }

    pub fn is_playing(&self) -> bool {
        self.clock.armed.load(Ordering::Acquire)
    }

    /// True cuando el dispositivo ya esta en regimen y `start()` suena al instante.
    pub fn is_warm(&self) -> bool {
        self.clock.warm.load(Ordering::Acquire)
    }

    /// Vuelve al principio sin cerrar ni reabrir el dispositivo, asi que no se paga
    /// otra vez el arranque del stream.
    pub fn restart(&self) {
        self.clock.seek_frames.store(0, Ordering::Release);
        self.clock.armed.store(true, Ordering::Release);
    }
}

/// Elige un buffer pequeno dentro de lo que el dispositivo admite.
fn pick_buffer_size(device: &cpal::Device, channels: u16, sample_rate: u32) -> cpal::BufferSize {
    let Ok(configs) = device.supported_output_configs() else {
        return cpal::BufferSize::Default;
    };

    for cfg in configs {
        if cfg.channels() != channels
            || sample_rate < cfg.min_sample_rate().0
            || sample_rate > cfg.max_sample_rate().0
        {
            continue;
        }
        if let cpal::SupportedBufferSize::Range { min, max } = cfg.buffer_size() {
            return cpal::BufferSize::Fixed(DESIRED_BUFFER_FRAMES.clamp(*min, *max));
        }
    }

    cpal::BufferSize::Default
}

/// Ajusta canales y sample rate del PCM decodificado a algo que el dispositivo acepte.
fn adapt_to_device(
    device: &cpal::Device,
    audio: DecodedAudio,
    default_cfg: &cpal::SupportedStreamConfig,
) -> Result<(Vec<f32>, u32, u16), String> {
    let DecodedAudio {
        mut samples,
        mut sample_rate,
        mut channels,
    } = audio;

    let supported: Vec<_> = device
        .supported_output_configs()
        .map_err(|e| format!("no se pudieron listar las configs: {}", e))?
        .collect();

    let supports = |ch: u16, rate: u32| {
        supported.iter().any(|c| {
            c.channels() == ch && rate >= c.min_sample_rate().0 && rate <= c.max_sample_rate().0
        })
    };

    // 1. Canales: si el device no admite los del archivo, remezclamos a los suyos.
    if !supported.iter().any(|c| c.channels() == channels) {
        let target = default_cfg.channels();
        samples = remap_channels(&samples, channels, target);
        channels = target;
    }

    // 2. Sample rate: si no cae en ningun rango, resampleamos al del device.
    if !supports(channels, sample_rate) {
        let target = default_cfg.sample_rate().0;
        samples = resample_linear(&samples, channels, sample_rate, target);
        sample_rate = target;
    }

    Ok((samples, sample_rate, channels))
}

fn remap_channels(samples: &[f32], from: u16, to: u16) -> Vec<f32> {
    if from == to {
        return samples.to_vec();
    }
    let (from, to) = (from as usize, to as usize);
    let frames = samples.len() / from;
    let mut out = Vec::with_capacity(frames * to);

    for f in 0..frames {
        let src = &samples[f * from..f * from + from];
        for c in 0..to {
            // Mono se duplica a todos los canales; el resto toma el canal equivalente
            // y repite el ultimo disponible si el destino tiene mas.
            out.push(if from == 1 { src[0] } else { src[c.min(from - 1)] });
        }
    }
    out
}

fn resample_linear(samples: &[f32], channels: u16, from: u32, to: u32) -> Vec<f32> {
    if from == to {
        return samples.to_vec();
    }
    let ch = channels as usize;
    let in_frames = samples.len() / ch;
    let ratio = from as f64 / to as f64;
    let out_frames = ((in_frames as f64) / ratio).floor() as usize;
    let mut out = Vec::with_capacity(out_frames * ch);

    for f in 0..out_frames {
        let pos = f as f64 * ratio;
        let i = pos.floor() as usize;
        let frac = (pos - i as f64) as f32;
        let j = (i + 1).min(in_frames - 1);

        for c in 0..ch {
            let a = samples[i * ch + c];
            let b = samples[j * ch + c];
            out.push(a + (b - a) * frac);
        }
    }
    out
}
