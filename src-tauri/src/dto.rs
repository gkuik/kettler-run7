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
    /// Dernière erreur lisible (connexion, commande refusée…), effacée au succès suivant.
    pub last_error: Option<String>,
    /// Profil de course choisi et sa progression ; `None` = pas de profil.
    pub profile: Option<ProfileProgress>,
}

// ── Profils de course ───────────────────────────────────────────────────────

/// Un segment de profil : sur `distance_m` mètres, telle vitesse et telle inclinaison.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileSegment {
    pub distance_m: u32,
    pub speed_kmh: f32,
    pub incline_pct: f32,
}

/// Profil de course : suite de segments parcourus dans l'ordre.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunProfile {
    /// Identifiant stable ; vide à l'enregistrement d'un nouveau profil.
    pub id: String,
    pub name: String,
    /// Profil de départ fourni par l'application : lecture seule (à dupliquer).
    #[serde(default)]
    pub builtin: bool,
    pub segments: Vec<ProfileSegment>,
}

/// Où en est le profil choisi (calculé par le Rust, qui seul le déroule).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileProgress {
    pub profile_id: String,
    pub profile_name: String,
    pub segment_count: u32,
    /// Segment en cours (0…). Vaut `segment_count` une fois le profil terminé.
    pub segment_index: u32,
    /// Distance parcourue depuis le début du profil (mètres).
    pub done_m: u32,
    pub total_m: u32,
    /// Mètres restants dans le segment en cours (0 une fois terminé).
    pub segment_remaining_m: u32,
    /// Consignes du segment en cours (ou de la récupération une fois terminé).
    pub current: ProfileSegment,
    /// Segment suivant ; `None` pendant le dernier segment et après.
    pub next: Option<ProfileSegment>,
    /// Tous les segments parcourus : récupération lente, sans arrêt automatique.
    pub finished: bool,
    /// Consigne changée à la main : le profil reprend la main au segment suivant.
    pub manual: bool,
}

// ── Simulateur (lancement avec KETTLER_SIM=1, voir `treadmill/sim.rs`) ──────

/// Ce que ferait le vrai tapis, vu « de la salle » : la console, la clé, la
/// radio. Indépendant de ce que l'application a lu (utile pendant une coupure).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SimStatus {
    /// Clé de sécurité en place.
    pub key_in: bool,
    /// Le tapis répond au scan (clé en place, pas de coupure radio en cours).
    pub reachable: bool,
    /// Une liaison est ouverte avec l'application.
    pub link_active: bool,
    /// Coupure radio en cours : secondes avant que le tapis soit de nouveau visible.
    pub link_back_in_s: Option<u32>,
    /// Console muette (ne répond plus) : secondes restantes.
    pub muted_for_s: Option<u32>,
    /// Online : bande en marche (compte à rebours compris).
    pub running: bool,
    /// Compte à rebours 3-2-1 en cours : secondes restantes.
    pub countdown_s: Option<u32>,
    pub speed_kmh: f32,
    pub target_speed_kmh: f32,
    pub incline_pct: f32,
    pub target_incline_pct: f32,
    pub distance_m: u32,
    pub energy_kcal: f32,
    /// Mains sur les capteurs de pouls.
    pub hands_on: bool,
    /// En pause depuis assez longtemps pour que le tapis ignore un démarrage.
    pub long_pause: bool,
}

/// Événement provoqué sur le tapis simulé (menu « Simulateur » de l'interface).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum SimAction {
    /// Coupure radio : la liaison tombe et le tapis reste invisible `seconds` s.
    /// La bande, elle, continue (comme sur le vrai tapis).
    LinkLoss { seconds: u32 },
    /// Retrait de la clé : bande arrêtée net, module BLE coupé.
    PullKey,
    InsertKey,
    /// Boutons de la console.
    ConsoleStart,
    ConsoleStop,
    ConsoleSpeed { delta: f32 },
    ConsoleIncline { delta: f32 },
    /// Mains sur les capteurs de pouls (ou non).
    HandsOn { on: bool },
    /// Fait comme si la pause durait depuis longtemps (démarrage alors ignoré).
    AgePause,
    /// La console ne répond plus pendant `seconds` s (liaison gardée).
    Mute { seconds: u32 },
    /// La prochaine réponse aura un CRC invalide.
    CorruptNextAnswer,
    /// La prochaine écriture (commande) de l'application échoue côté Bluetooth.
    FailNextWrite,
}
