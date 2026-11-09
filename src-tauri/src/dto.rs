//! Types échangés avec le frontend (contrat d'interface).
//!
//! Tout est sérialisé en camelCase ; le miroir TypeScript est `src/lib/types.ts`.
//! Les valeurs brutes du tapis sont converties ici en unités lisibles : km/h,
//! %, mètres, kcal, secondes. `None` = pas encore lu (ou non disponible).

use serde::{Deserialize, Serialize};

/// Nom de l'événement Tauri qui porte un `TreadmillSnapshot` à chaque changement.
pub const EVENT_STATE: &str = "treadmill-state";

/// État de la liaison Bluetooth.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum ConnectionState {
    #[default]
    Disconnected,
    Scanning,
    Connecting,
    /// Liaison établie et handshake Kettler accepté.
    Connected,
    /// Liaison perdue alors qu'on ne l'avait pas demandé ; reconnexion en cours.
    Reconnecting,
}

/// Bornes annoncées par le tapis (SpeedMin/Max, InclineMin/Max).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Limits {
    pub speed_min_kmh: f32,
    pub speed_max_kmh: f32,
    pub incline_min_pct: f32,
    pub incline_max_pct: f32,
}

/// Identité lue une fois après le handshake.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceInfo {
    /// DeviceName (20), ex. « RUN 7 ».
    pub name: String,
    /// DeviceId (25), ex. « STB1T0158 ».
    pub id: String,
    /// Référence d'article (22), ex. « 07883-600 ».
    pub article: Option<String>,
}

/// Photographie complète de l'état du tapis, émise sur `EVENT_STATE`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct TreadmillSnapshot {
    pub connection: ConnectionState,
    /// Vrai si la liaison a été perdue pendant que la bande tournait : la bande
    /// CONTINUE de tourner dans ce cas (vérifié), l'interface doit alerter.
    pub lost_while_running: bool,
    /// Online (38) : brut 0 = en marche, 1 = arrêté.
    pub running: Option<bool>,
    /// SpeedGet (35) / 10 : vitesse réelle de la bande.
    pub speed_kmh: Option<f32>,
    /// SpeedSet (34) / 10 : consigne.
    pub target_speed_kmh: Option<f32>,
    /// InclineGet (31) / 10 : inclinaison réelle.
    pub incline_pct: Option<f32>,
    /// InclineSet (30) / 10 : consigne d'inclinaison.
    pub target_incline_pct: Option<f32>,
    /// Distance (15), en mètres (vérifié).
    pub distance_m: Option<u32>,
    /// Energy (16) / 10 ; unité 0,1 kcal probable mais non confirmée.
    pub energy_kcal: Option<f32>,
    /// Pulse (8) ; `None` quand le tapis renvoie 0 (pas de contact).
    pub pulse_bpm: Option<u16>,
    /// Temps de course calculé par l'application (le tapis ne le fournit pas).
    pub elapsed_s: u32,
    pub limits: Option<Limits>,
    pub device: Option<DeviceInfo>,
    /// Plafond de vitesse appliqué par l'application à toute consigne envoyée.
    pub speed_cap_kmh: f32,
    /// Dernière erreur lisible (connexion, commande refusée…), effacée au succès suivant.
    pub last_error: Option<String>,
}
