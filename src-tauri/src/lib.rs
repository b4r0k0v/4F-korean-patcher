pub mod archive;
mod commands;
pub mod error;
pub mod hash;
pub mod model;
pub mod pipeline;
pub mod tools;
pub mod validation;
pub mod workspace;

pub fn run() {
    use tauri::{Emitter, Manager};
    tauri::Builder::default()
        .manage(commands::Jobs::default())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            commands::check_inputs,
            commands::check_hash,
            commands::start_patch,
            commands::cancel_patch
        ])
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                if window
                    .state::<commands::Jobs>()
                    .active
                    .lock()
                    .map(|s| s.is_some())
                    .unwrap_or(true)
                {
                    api.prevent_close();
                    let _ = window.emit("patch-close-requested", ());
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("앱 시작 실패");
}
