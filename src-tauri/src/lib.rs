//! AnimaBooter library entry — wires plugins, state and IPC commands.

mod commands;
mod core;
mod error;
mod image;
mod platform;
mod safety;
mod state;

use tauri::Manager;

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .setup(|app| {
            app.manage(state::AppState::load(app.handle()));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::list_drives::list_drives,
            commands::flash::flash,
            commands::cancel_flash::cancel_flash,
            commands::eject::eject,
            commands::detect_image,
            commands::get_settings,
            commands::set_settings,
            commands::restart_as_admin
        ])
        .run(tauri::generate_context!())
        .expect("failed to run AnimaBooter");
}
