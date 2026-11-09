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

/// Version de l'application (Cargo.toml ; identique à package.json et tauri.conf.json).
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Configure et lance l'application Tauri.
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    eprintln!("Kettler RUN 7 v{VERSION}");
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

#[cfg(test)]
mod tests {
    /// Version écrite dans un fichier JSON (`"version": "…"`, première occurrence).
    fn json_version(text: &str) -> String {
        let v: serde_json::Value = serde_json::from_str(text).expect("JSON valide");
        v["version"].as_str().expect("champ version").to_string()
    }

    /// Les trois sources de la version doivent concorder
    /// (`npm run version:set -- X.Y.Z` les écrit toutes).
    #[test]
    fn version_identique_partout() {
        let tauri = json_version(include_str!("../tauri.conf.json"));
        let npm = json_version(include_str!("../../package.json"));
        assert_eq!(super::VERSION, tauri, "Cargo.toml / tauri.conf.json");
        assert_eq!(super::VERSION, npm, "Cargo.toml / package.json");
    }
}
