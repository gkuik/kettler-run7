//! Point d'entrée de la bibliothèque applicative (partagé desktop/mobile).

mod commands;
mod dto;
mod kettler;
mod treadmill;

use tauri::{Emitter, Manager};

use dto::EVENT_STATE;
use treadmill::profile::ProfileLibrary;
use treadmill::sim::Simulator;
use treadmill::{StateEmitter, Treadmill};

/// Vrai si l'application doit piloter le tapis simulé (`KETTLER_SIM=1`).
fn simulation_requested() -> bool {
    std::env::var("KETTLER_SIM").is_ok_and(|v| matches!(v.trim(), "1" | "true" | "oui"))
}

/// Configure et lance l'application Tauri.
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            // Chaque changement d'état part vers l'interface (`treadmill-state`).
            // Rien n'est ouvert en Bluetooth avant le premier `connect`.
            let handle = app.handle().clone();
            let emit: StateEmitter = Box::new(move |snap| {
                let _ = handle.emit(EVENT_STATE, snap);
            });
            let sim = simulation_requested().then(Simulator::new);
            if sim.is_some() {
                eprintln!("[sim] MODE SIMULATEUR : aucun Bluetooth, tapis simulé");
                // Impossible de confondre avec le vrai tapis.
                if let Some(w) = app.get_webview_window("main") {
                    let _ = w.set_title("Kettler RUN 7 — SIMULATEUR");
                }
            }
            // Profils de course : `profiles.json` dans le dossier de données de l'appli.
            let library = match app.path().app_data_dir() {
                Ok(dir) => ProfileLibrary::load(dir.join("profiles.json")),
                Err(e) => {
                    eprintln!("[profils] dossier de données indisponible ({e}) : profils non enregistrés");
                    ProfileLibrary::in_memory()
                }
            };
            app.manage(Treadmill::new(emit, sim, library));
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
            commands::list_profiles,
            commands::save_profile,
            commands::delete_profile,
            commands::select_profile,
            commands::sim_status,
            commands::sim_action,
        ])
        .run(tauri::generate_context!())
        .expect("erreur au lancement de l'application");
}
