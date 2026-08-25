// electron-audio-native/src/lib.rs
mod audio;
mod decoder;

use napi::bindgen_prelude::AsyncTask;
use napi::{Env, Error, Result, Status, Task};
use napi_derive::napi;
use once_cell::sync::Lazy;
use std::sync::Mutex;

use audio::AudioEngine;
use decoder::DecodedAudio;

static ENGINE: Lazy<Mutex<Option<AudioEngine>>> = Lazy::new(|| Mutex::new(None));

fn err(msg: String) -> Error {
    Error::new(Status::GenericFailure, msg)
}

#[napi(object)]
pub struct AudioInfo {
    pub duration_ms: f64,
    /// Sample rate y canales con los que se abrio el dispositivo (puede diferir del
    /// archivo si hubo que remezclar o resamplear).
    pub sample_rate: u32,
    pub channels: u32,
    pub device_name: String,
    /// Frames por callback. 0 = el backend eligio el suyo.
    pub buffer_frames: u32,
    /// Latencia teorica de un buffer a ese sample rate (ms).
    pub buffer_ms: f64,
}

#[napi(object)]
pub struct PlaybackStats {
    pub position_ms: f64,
    pub duration_ms: f64,
    /// Latencia de salida que reporta el backend en el primer callback (ms).
    pub output_latency_ms: f64,
    /// Correccion manual aplicada a `position_ms` (ms).
    pub offset_ms: f64,
    pub playing: bool,
    /// El dispositivo ya esta en regimen: `play()` sonara de inmediato.
    pub ready: bool,
}

pub struct LoadTask {
    path: String,
}

impl Task for LoadTask {
    type Output = DecodedAudio;
    type JsValue = AudioInfo;

    /// Corre en un worker de libuv: decodificar un mp3 entero bloquea decenas de ms
    /// y no puede hacerlo el hilo principal de Electron.
    fn compute(&mut self) -> Result<Self::Output> {
        decoder::decode_file(&self.path).map_err(err)
    }

    /// Vuelve al hilo principal, que es donde debe vivir el `cpal::Stream`.
    fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
        let engine = AudioEngine::prepare(output).map_err(err)?;

        let info = AudioInfo {
            duration_ms: engine.duration_ms(),
            sample_rate: engine.sample_rate,
            channels: engine.channels as u32,
            device_name: engine.device_name.clone(),
            buffer_frames: engine.buffer_frames,
            buffer_ms: if engine.buffer_frames > 0 {
                engine.buffer_frames as f64 / engine.sample_rate as f64 * 1000.0
            } else {
                0.0
            },
        };

        *ENGINE.lock().unwrap() = Some(engine);

        Ok(info)
    }
}

/// Decodifica el archivo y deja el dispositivo abierto sacando silencio.
/// Todo el coste de arranque se paga aqui, no en `play()`.
#[napi(ts_return_type = "Promise<AudioInfo>")]
pub fn load_audio(path: String) -> AsyncTask<LoadTask> {
    AsyncTask::new(LoadTask { path })
}

/// Arranca la reproduccion del audio ya cargado. Devuelve en microsegundos.
#[napi]
pub fn play() -> Result<()> {
    let guard = ENGINE.lock().unwrap();
    let engine = guard
        .as_ref()
        .ok_or_else(|| err("no hay audio cargado: llama antes a loadAudio()".into()))?;
    engine.start();
    Ok(())
}

/// Posicion audible actual en ms. Barata: es una lectura atomica y una resta.
#[napi]
pub fn get_position_ms() -> f64 {
    ENGINE
        .lock()
        .unwrap()
        .as_ref()
        .map(|e| e.position_ms())
        .unwrap_or(0.0)
}

#[napi]
pub fn get_stats() -> PlaybackStats {
    let guard = ENGINE.lock().unwrap();
    match guard.as_ref() {
        Some(e) => PlaybackStats {
            position_ms: e.position_ms(),
            duration_ms: e.duration_ms(),
            output_latency_ms: e.output_latency_ms(),
            offset_ms: e.offset_ms(),
            playing: e.is_playing(),
            ready: e.is_warm(),
        },
        None => PlaybackStats {
            position_ms: 0.0,
            duration_ms: 0.0,
            output_latency_ms: 0.0,
            offset_ms: 0.0,
            playing: false,
            ready: false,
        },
    }
}

/// True cuando el dispositivo esta en regimen. Hasta entonces `play()` puede tardar
/// ~200 ms en sonar porque ALSA todavia esta levantando el stream.
#[napi]
pub fn is_ready() -> bool {
    ENGINE.lock().unwrap().as_ref().map(|e| e.is_warm()).unwrap_or(false)
}

/// Vuelve al principio sin reabrir el dispositivo, asi que suena al instante.
#[napi]
pub fn restart() -> Result<()> {
    let guard = ENGINE.lock().unwrap();
    let engine = guard
        .as_ref()
        .ok_or_else(|| err("no hay audio cargado: llama antes a loadAudio()".into()))?;
    engine.restart();
    Ok(())
}

/// Calibracion manual: positivo si el audio se oye mas tarde de lo que dice el reloj.
#[napi]
pub fn set_offset_ms(offset_ms: f64) {
    if let Some(e) = ENGINE.lock().unwrap().as_ref() {
        e.set_offset_ms(offset_ms);
    }
}

/// Cierra el stream y libera el dispositivo.
#[napi]
pub fn stop() {
    *ENGINE.lock().unwrap() = None;
}
