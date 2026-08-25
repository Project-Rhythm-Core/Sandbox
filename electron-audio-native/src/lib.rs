// electron-audio-native/src/lib.rs
mod audio;
mod decoder;

use napi_derive::napi;
use std::sync::Mutex;
use once_cell::sync::Lazy;
use audio::AudioPlayer;

static PLAYER: Lazy<Mutex<Option<AudioPlayer>>> = Lazy::new(|| Mutex::new(None));

#[napi]
pub fn play_test_audio(path: String) {
    let (samples, sample_rate, channels) = decoder::decode_file(&path);
    let player = AudioPlayer::play(samples, sample_rate, channels);
    *PLAYER.lock().unwrap() = Some(player);
}

#[napi]
pub fn get_position_ms() -> f64 {
    PLAYER.lock().unwrap()
        .as_ref()
        .map(|p| p.position_ms())
        .unwrap_or(0.0)
}