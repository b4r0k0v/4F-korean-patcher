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
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            commands::check_inputs,
            commands::check_hash
        ])
        .run(tauri::generate_context!())
        .expect("앱 시작 실패");
}
