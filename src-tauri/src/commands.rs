//! Commandes Tauri exposées au frontend.
//!
//! Couche fine : chaque commande délègue à `treadmill::Treadmill`, qui possède
//! tout l'état et toutes les règles de sécurité. Les erreurs sont des `String`
//! lisibles en français, affichables telles quelles.

use tauri::State;

use crate::dto::{SimAction, SimStatus, TreadmillSnapshot};
use crate::treadmill::Treadmill;

/// Journal de diagnostic (terminal de `tauri dev`) : chaque commande et son issue.
fn log_result(what: &str, r: &Result<(), String>) {
    match r {
        Ok(()) => eprintln!("[cmd] {what} -> ok"),
        Err(e) => eprintln!("[cmd] {what} -> ERREUR : {e}"),
    }
}

/// Photographie courante de l'état du tapis.
#[tauri::command]
pub async fn get_state(treadmill: State<'_, Treadmill>) -> Result<TreadmillSnapshot, String> {
    Ok(treadmill.snapshot())
}

/// Recherche le tapis, s'y connecte et lance le polling.
#[tauri::command]
pub async fn connect(treadmill: State<'_, Treadmill>) -> Result<(), String> {
    let r = treadmill.connect().await;
    log_result("connect", &r);
    r
}

/// Déconnexion voulue (pas de reconnexion automatique).
#[tauri::command]
pub async fn disconnect(treadmill: State<'_, Treadmill>) -> Result<(), String> {
    let r = treadmill.disconnect().await;
    log_result("disconnect", &r);
    r
}

/// Démarre la bande (compte à rebours 3-2-1 sur la console).
#[tauri::command]
pub async fn start(treadmill: State<'_, Treadmill>) -> Result<(), String> {
    let r = treadmill.start().await;
    log_result("start", &r);
    r
}

/// Arrête la bande. Prioritaire sur toute autre écriture.
#[tauri::command]
pub async fn stop(treadmill: State<'_, Treadmill>) -> Result<(), String> {
    let r = treadmill.stop().await;
    log_result("stop", &r);
    r
}

/// Consigne de vitesse (km/h), arrondie, bornée et plafonnée côté Rust.
#[tauri::command]
pub async fn set_target_speed(kmh: f32, treadmill: State<'_, Treadmill>) -> Result<(), String> {
    let r = treadmill.set_target_speed(kmh).await;
    log_result(&format!("set_target_speed({kmh})"), &r);
    r
}

/// Consigne d'inclinaison (%), arrondie et bornée côté Rust.
#[tauri::command]
pub async fn set_target_incline(pct: f32, treadmill: State<'_, Treadmill>) -> Result<(), String> {
    let r = treadmill.set_target_incline(pct).await;
    log_result(&format!("set_target_incline({pct})"), &r);
    r
}

/// Plafond de vitesse (1–20 km/h) appliqué à toute consigne.
#[tauri::command]
pub async fn set_speed_cap(kmh: f32, treadmill: State<'_, Treadmill>) -> Result<(), String> {
    let r = treadmill.set_speed_cap(kmh).await;
    log_result(&format!("set_speed_cap({kmh})"), &r);
    r
}

// ── Simulateur ──────────────────────────────────────────────────────────────

/// État du tapis simulé ; `None` quand l'application pilote le vrai tapis.
#[tauri::command]
pub async fn sim_status(treadmill: State<'_, Treadmill>) -> Result<Option<SimStatus>, String> {
    Ok(treadmill.simulator().map(|sim| sim.status()))
}

/// Provoque un événement sur le tapis simulé (refusé avec le vrai tapis).
#[tauri::command]
pub async fn sim_action(action: SimAction, treadmill: State<'_, Treadmill>) -> Result<(), String> {
    let sim = treadmill
        .simulator()
        .ok_or("Pas de simulateur : l'application pilote le vrai tapis")?;
    sim.act(action)
}
