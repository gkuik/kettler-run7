//! Point d'entrée de la bibliothèque applicative (partagé desktop/mobile).

mod commands;
mod dto;
mod kettler;
mod treadmill;

use tauri::Manager;

use treadmill::Treadmill;

/// Configure et lance l'application Tauri.
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            // Le tapis a besoin d'un AppHandle pour émettre `treadmill-state`.
            // Rien n'est ouvert en Bluetooth avant le premier `connect`.
            app.manage(Treadmill::new(app.handle().clone()));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_state,
            commands::connect,
            commands::disconnect,
            commands::start,
            commands::stop,
            commands::set_target_speed,
            commands::set_target_incline,
            commands::set_speed_cap,
        ])
        .run(tauri::generate_context!())
        .expect("erreur au lancement de l'application");
}
