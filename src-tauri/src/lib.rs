mod audio;
mod decoder;

use std::sync::Mutex;
use crate::audio::AudioPlayer;


struct AppState {
  player: Mutex<Option<AudioPlayer>>,
}

#[tauri::command]
fn play_test_audio(state: tauri::State<AppState>) {
  let (samples, sample_rate, channels) = decoder::decode_file("test-audio.mp3");
  let player = AudioPlayer::play(samples, sample_rate, channels);
  *state.player.lock().unwrap() = Some(player);
}

#[tauri::command]
fn get_position_ms(state: tauri::State<AppState>) -> f64 {
  state.player.lock().unwrap()
    .as_ref()
    .map(|p| p.position_ms())
    .unwrap_or(0.0)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
  tauri::Builder::default()
    .setup(|app| {
      if cfg!(debug_assertions) {
        app.handle().plugin(
          tauri_plugin_log::Builder::default()
            .level(log::LevelFilter::Info)
            .build(),
        )?;
      }
      Ok(())
    })
    .manage(AppState { player: Mutex::new(None) })
    .invoke_handler(tauri::generate_handler![
      play_test_audio,
      get_position_ms
    ])
    .run(tauri::generate_context!())
    .expect("error while running tauri application");
}
