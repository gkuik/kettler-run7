//! Logique d'état PURE du tapis : aucune I/O, aucun Bluetooth, entièrement testable.
//!
//! - conversion des réponses du tapis (`Reading`) en `TreadmillSnapshot` ;
//! - accumulation du temps de course (`elapsed_s`) et remise à zéro quand le
//!   compteur de distance du tapis repart en arrière ;
//! - transitions de connexion (dont `lost_while_running`) ;
//! - préparation des consignes : arrondi à 0,1, bornage aux limites du tapis,
//!   refus si la bande ne tourne pas ;
//! - déroulement du profil de course choisi (`profile::ProfileRun`) : quelles
//!   consignes envoyer, et quand.
//!
//! La session Bluetooth (`session.rs`) ne fait que transporter des octets : toute
//! décision de sécurité sur les valeurs envoyées est prise ici.

use std::time::{Duration, Instant};

use super::profile::ProfileRun;
use crate::dto::{ConnectionState, DeviceInfo, Limits, RunProfile, TreadmillSnapshot};
use crate::kettler::protocol::{Instruction, Message, ValueId};

/// Messages d'erreur partagés (affichés tels quels par l'interface).
pub const ERR_NOT_CONNECTED: &str = "Tapis non connecté";
pub const ERR_NOT_RUNNING: &str = "La bande ne tourne pas : démarrez-la d'abord";
pub const ERR_RUNNING_UNKNOWN: &str =
    "État marche/arrêt du tapis pas encore lu : réessayez dans un instant";
pub const ERR_ALREADY_RUNNING: &str = "La bande tourne déjà";
pub const ERR_LIMITS_UNKNOWN: &str = "Limites du tapis pas encore lues : réessayez dans un instant";
pub const ERR_INVALID_NUMBER: &str = "Valeur invalide";

/// Valeur lue sur le tapis, déjà typée (valeurs brutes du protocole).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Reading {
    /// Online (38), brut : 0 = en marche, 1 = arrêté.
    Online(u16),
    SpeedGet(u16),
    SpeedSet(u16),
    InclineGet(u16),
    InclineSet(u16),
    Distance(u16),
    Energy(u16),
    Pulse(u16),
    SpeedMin(u16),
    SpeedMax(u16),
    InclineMin(u16),
    InclineMax(u16),
    DeviceName(String),
    DeviceId(String),
    Article(String),
}

/// Vrai si le nom BLE annoncé est celui du RUN 7 (« RUN 7 », espaces et casse ignorés).
pub fn is_treadmill_name(name: &str) -> bool {
    let compact: String = name.chars().filter(|c| !c.is_whitespace()).collect();
    compact.eq_ignore_ascii_case("run7")
}

/// Vrai si le message est la réponse du tapis au handshake.
pub fn is_handshake_answer(msg: &Message) -> bool {
    msg.value_id == ValueId::Handshake as u16 && msg.instruction == Instruction::Answer as u8
}

/// Convertit un message reçu en `Reading`. Seules les réponses (instruction 3)
/// des valeurs connues de l'application sont retenues ; le reste est ignoré.
pub fn decode(msg: &Message) -> Option<Reading> {
    if msg.instruction != Instruction::Answer as u8 {
        return None;
    }
    let id = ValueId::from_u16(msg.value_id)?;
    let num = || msg.as_u16();
    let text = || msg.as_text().map(|t| t.trim().to_string());
    Some(match id {
        // Online est sur 1 octet ; on accepte aussi 2 octets par prudence.
        ValueId::Online => Reading::Online(msg.as_u8().map(u16::from).or_else(num)?),
        ValueId::SpeedGet => Reading::SpeedGet(num()?),
        ValueId::SpeedSet => Reading::SpeedSet(num()?),
        ValueId::InclineGet => Reading::InclineGet(num()?),
        ValueId::InclineSet => Reading::InclineSet(num()?),
        ValueId::Distance => Reading::Distance(num()?),
        ValueId::Energy => Reading::Energy(num()?),
        ValueId::Pulse => Reading::Pulse(num()?),
        ValueId::SpeedMin => Reading::SpeedMin(num()?),
        ValueId::SpeedMax => Reading::SpeedMax(num()?),
        ValueId::InclineMin => Reading::InclineMin(num()?),
        ValueId::InclineMax => Reading::InclineMax(num()?),
        ValueId::DeviceName => Reading::DeviceName(text()?),
        ValueId::DeviceId => Reading::DeviceId(text()?),
        ValueId::Article => Reading::Article(text()?),
        ValueId::Handshake | ValueId::DeviceState | ValueId::DeviceType => return None,
    })
}

/// Valeurs lues à CHAQUE cycle : ce qui reflète une commande (marche,
/// consignes) et la vitesse réelle. La console ne traite que ~6 à 8 requêtes
/// par seconde : on garde le cycle court pour que les commandes passent vite.
pub const FAST_IDS: [ValueId; 5] = [
    ValueId::Online,
    ValueId::SpeedSet,
    ValueId::SpeedGet,
    ValueId::InclineSet,
    ValueId::InclineGet,
];

/// Valeurs lues un cycle sur `SLOW_EVERY` (compteurs, pouls).
pub const SLOW_IDS: [ValueId; 3] = [ValueId::Distance, ValueId::Energy, ValueId::Pulse];
pub const SLOW_EVERY: u32 = 2;

/// Arrondit une valeur en unités de 0,1 (km/h ou %). `None` si non finie.
fn to_tenths(v: f32) -> Option<i64> {
    if !v.is_finite() {
        return None;
    }
    Some((f64::from(v) * 10.0).round() as i64)
}

fn tenths(raw: u16) -> f32 {
    f32::from(raw) / 10.0
}

/// État complet du tapis vu par l'application.
#[derive(Debug, Clone)]
pub struct Model {
    snap: TreadmillSnapshot,
    speed_min: Option<u16>,
    speed_max: Option<u16>,
    incline_min: Option<u16>,
    incline_max: Option<u16>,
    name: Option<String>,
    id: Option<String>,
    article: Option<String>,
    last_distance: Option<u32>,
    elapsed: Duration,
    last_tick: Option<Instant>,
    last_published: Option<TreadmillSnapshot>,
    /// Profil de course choisi et sa progression.
    profile: Option<ProfileRun>,
    /// Une séquence de démarrage de l'application est en cours : une remise à
    /// zéro du compteur de distance n'y est pas une fin de séance.
    start_sequence: bool,
}

/// Consignes à envoyer pour le profil (brutes, déjà bornées aux limites du tapis) ;
/// `None` = déjà en place sur le tapis.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProfileOrder {
    pub speed: Option<u16>,
    pub incline: Option<u16>,
}

impl Default for Model {
    fn default() -> Self {
        Self::new()
    }
}

impl Model {
    pub fn new() -> Self {
        Model {
            snap: TreadmillSnapshot::default(),
            speed_min: None,
            speed_max: None,
            incline_min: None,
            incline_max: None,
            name: None,
            id: None,
            article: None,
            last_distance: None,
            elapsed: Duration::ZERO,
            last_tick: None,
            last_published: None,
            profile: None,
            start_sequence: false,
        }
    }

    /// Photographie courante.
    pub fn snapshot(&self) -> TreadmillSnapshot {
        let mut s = self.snap.clone();
        s.elapsed_s = self.elapsed.as_secs() as u32;
        s.profile = self.profile.as_ref().map(ProfileRun::progress);
        s
    }

    /// Rend la photographie si elle a changé depuis la dernière publication
    /// (et la mémorise comme publiée).
    pub fn take_changed(&mut self) -> Option<TreadmillSnapshot> {
        let s = self.snapshot();
        if self.last_published.as_ref() == Some(&s) {
            return None;
        }
        self.last_published = Some(s.clone());
        Some(s)
    }

    pub fn running(&self) -> Option<bool> {
        self.snap.running
    }

    pub fn set_error(&mut self, error: Option<String>) {
        self.snap.last_error = error;
    }

    /// Changement d'étape pendant une connexion voulue (Scanning, Connecting).
    pub fn set_connection(&mut self, state: ConnectionState) {
        self.snap.connection = state;
        if state != ConnectionState::Connected {
            self.last_tick = None;
        }
    }

    /// Liaison établie et handshake accepté. L'état marche/arrêt est oublié :
    /// aucune consigne n'est acceptée avant une lecture fraîche d'Online
    /// (la console a pu être utilisée pendant la coupure).
    pub fn on_connected(&mut self) {
        self.snap.connection = ConnectionState::Connected;
        self.snap.running = None;
        self.snap.lost_while_running = false;
        self.snap.last_error = None;
        self.last_tick = None;
    }

    /// Perte de liaison NON voulue : on passe en reconnexion et on retient si la
    /// bande tournait (elle continue de tourner dans ce cas, vérifié).
    pub fn on_link_lost(&mut self) {
        self.snap.connection = ConnectionState::Reconnecting;
        self.snap.lost_while_running = self.snap.running == Some(true);
        self.last_tick = None;
    }

    /// Déconnexion voulue (ou échec de connexion) : on oublie les valeurs
    /// lues, mais on garde le temps de course et la dernière distance (pour
    /// détecter une remise à zéro à la reconnexion).
    pub fn on_disconnected(&mut self) {
        let last_error = self.snap.last_error.take();
        self.snap = TreadmillSnapshot {
            last_error,
            ..TreadmillSnapshot::default()
        };
        self.speed_min = None;
        self.speed_max = None;
        self.incline_min = None;
        self.incline_max = None;
        self.name = None;
        self.id = None;
        self.article = None;
        self.last_tick = None;
    }

    /// Intègre une valeur lue sur le tapis.
    pub fn apply(&mut self, reading: Reading) {
        let s = &mut self.snap;
        match reading {
            Reading::Online(raw) => {
                s.running = match raw {
                    0 => Some(true),
                    1 => Some(false),
                    _ => s.running, // valeur inconnue : on garde l'état précédent
                };
                if s.running == Some(false) {
                    if let Some(run) = self.profile.as_mut() {
                        run.on_stopped();
                    }
                }
            }
            Reading::SpeedGet(raw) => s.speed_kmh = Some(tenths(raw)),
            Reading::SpeedSet(raw) => s.target_speed_kmh = Some(tenths(raw)),
            Reading::InclineGet(raw) => s.incline_pct = Some(tenths(raw)),
            Reading::InclineSet(raw) => s.target_incline_pct = Some(tenths(raw)),
            Reading::Distance(raw) => {
                let d = u32::from(raw);
                // Le compteur du tapis repart en arrière : nouvelle séance.
                if self.last_distance.is_some_and(|prev| d < prev) {
                    self.elapsed = Duration::ZERO;
                }
                self.last_distance = Some(d);
                s.distance_m = Some(d);
                if let Some(run) = self.profile.as_mut() {
                    run.on_distance(d, self.start_sequence);
                }
            }
            Reading::Energy(raw) => s.energy_kcal = Some(tenths(raw)),
            Reading::Pulse(raw) => s.pulse_bpm = (raw != 0).then_some(raw),
            Reading::SpeedMin(raw) => self.speed_min = Some(raw),
            Reading::SpeedMax(raw) => self.speed_max = Some(raw),
            Reading::InclineMin(raw) => self.incline_min = Some(raw),
            Reading::InclineMax(raw) => self.incline_max = Some(raw),
            Reading::DeviceName(t) => self.name = Some(t),
            Reading::DeviceId(t) => self.id = Some(t),
            Reading::Article(t) => self.article = Some(t),
        }
        self.refresh_statics();
    }

    fn refresh_statics(&mut self) {
        if let (Some(smin), Some(smax), Some(imin), Some(imax)) = (
            self.speed_min,
            self.speed_max,
            self.incline_min,
            self.incline_max,
        ) {
            self.snap.limits = Some(Limits {
                speed_min_kmh: tenths(smin),
                speed_max_kmh: tenths(smax),
                incline_min_pct: tenths(imin),
                incline_max_pct: tenths(imax),
            });
        }
        if let (Some(name), Some(id)) = (&self.name, &self.id) {
            self.snap.device = Some(DeviceInfo {
                name: name.clone(),
                id: id.clone(),
                article: self.article.clone(),
            });
        }
    }

    /// Valeurs statiques encore inconnues, à ajouter au cycle de polling.
    pub fn missing_static_ids(&self, include_article: bool) -> Vec<ValueId> {
        let mut ids = Vec::new();
        let pairs = [
            (self.name.is_none(), ValueId::DeviceName),
            (self.id.is_none(), ValueId::DeviceId),
            (include_article && self.article.is_none(), ValueId::Article),
            (self.speed_min.is_none(), ValueId::SpeedMin),
            (self.speed_max.is_none(), ValueId::SpeedMax),
            (self.incline_min.is_none(), ValueId::InclineMin),
            (self.incline_max.is_none(), ValueId::InclineMax),
        ];
        for (missing, id) in pairs {
            if missing {
                ids.push(id);
            }
        }
        ids
    }

    /// Avance l'horloge : le temps de course s'accumule tant que la liaison est
    /// établie et que la bande tourne.
    pub fn tick(&mut self, now: Instant) {
        if self.snap.connection != ConnectionState::Connected {
            self.last_tick = None;
            return;
        }
        if let Some(prev) = self.last_tick {
            if self.snap.running == Some(true) {
                self.elapsed += now.saturating_duration_since(prev);
            }
        }
        self.last_tick = Some(now);
    }

    // ── Profil de course ────────────────────────────────────────────────────

    /// Choisit le profil à dérouler (`None` : plus de profil, les consignes
    /// restent telles quelles). Le même profil, modifié, garde sa progression.
    pub fn select_profile(&mut self, profile: Option<RunProfile>) {
        self.profile = match (self.profile.take(), profile) {
            (Some(mut run), Some(p)) if run.profile().id == p.id => {
                run.replace_profile(p);
                Some(run)
            }
            (_, Some(p)) => Some(ProfileRun::new(p)),
            (_, None) => None,
        };
    }

    pub fn profile_id(&self) -> Option<&str> {
        self.profile.as_ref().map(|r| r.profile().id.as_str())
    }

    pub fn set_start_sequence(&mut self, on: bool) {
        self.start_sequence = on;
    }

    /// Second arrêt, bande arrêtée : le tapis remet ses compteurs à zéro, la
    /// séance est finie et le profil repart du début.
    pub fn profile_session_ended(&mut self) {
        if let Some(run) = self.profile.as_mut() {
            run.restart();
        }
    }

    /// L'utilisateur a changé une consigne : le profil la laisse jusqu'au segment suivant.
    pub fn profile_manual(&mut self) {
        if let Some(run) = self.profile.as_mut() {
            run.on_manual();
        }
    }

    /// Consignes à envoyer maintenant pour le profil, s'il y en a. Rien tant que
    /// la bande ne tourne pas vraiment : liaison établie, Online relu à « en
    /// marche », limites connues, et compte à rebours fini (consigne relue au
    /// moins à la vitesse minimale : le tapis ignore les écritures avant).
    pub fn profile_order(&mut self, now: Instant) -> Option<ProfileOrder> {
        self.profile.as_ref()?;
        self.ensure_running().ok()?;
        let min = self.speed_min?;
        let target_speed = self.snap.target_speed_kmh.and_then(to_tenths)?;
        if target_speed < i64::from(min) {
            return None;
        }
        let run = self.profile.as_ref()?;
        let wanted = run.target(run.index());
        let speed = self.speed_command(wanted.speed_kmh).ok()?;
        let incline = self.incline_command(wanted.incline_pct).ok()?;
        let target_incline = self.snap.target_incline_pct.and_then(to_tenths);
        let speed_ok = target_speed == i64::from(speed);
        let incline_ok = target_incline == Some(i64::from(incline));
        let run = self.profile.as_mut()?;
        if !run.step(now, speed_ok && incline_ok) {
            return None;
        }
        Some(ProfileOrder {
            speed: (!speed_ok).then_some(speed),
            incline: (!incline_ok).then_some(incline),
        })
    }

    // ── Préparation des commandes (aucune écriture ici) ─────────────────────

    fn ensure_connected(&self) -> Result<(), String> {
        if self.snap.connection == ConnectionState::Connected {
            Ok(())
        } else {
            Err(ERR_NOT_CONNECTED.into())
        }
    }

    fn ensure_running(&self) -> Result<(), String> {
        self.ensure_connected()?;
        match self.snap.running {
            Some(true) => Ok(()),
            Some(false) => Err(ERR_NOT_RUNNING.into()),
            None => Err(ERR_RUNNING_UNKNOWN.into()),
        }
    }

    /// Vérifie qu'un démarrage est possible (bande connue et arrêtée).
    pub fn check_start(&self) -> Result<(), String> {
        self.ensure_connected()?;
        match self.snap.running {
            Some(false) => Ok(()),
            Some(true) => Err(ERR_ALREADY_RUNNING.into()),
            None => Err(ERR_RUNNING_UNKNOWN.into()),
        }
    }

    /// Consigne de vitesse brute (0,1 km/h) à écrire : arrondie, bornée à
    /// `[SpeedMin, SpeedMax]`. Refusée si la bande ne tourne pas.
    pub fn speed_command(&self, kmh: f32) -> Result<u16, String> {
        self.ensure_running()?;
        let wanted = to_tenths(kmh).ok_or(ERR_INVALID_NUMBER)?;
        let (Some(min), Some(max)) = (self.speed_min, self.speed_max) else {
            return Err(ERR_LIMITS_UNKNOWN.into());
        };
        let hi = i64::from(max);
        let lo = i64::from(min).min(hi);
        Ok(wanted.clamp(lo, hi) as u16)
    }

    /// Consigne d'inclinaison brute (0,1 %) : arrondie, bornée aux limites du
    /// tapis. Refusée si la bande ne tourne pas.
    pub fn incline_command(&self, pct: f32) -> Result<u16, String> {
        self.ensure_running()?;
        let wanted = to_tenths(pct).ok_or(ERR_INVALID_NUMBER)?;
        let (Some(min), Some(max)) = (self.incline_min, self.incline_max) else {
            return Err(ERR_LIMITS_UNKNOWN.into());
        };
        let lo = i64::from(min.min(max));
        Ok(wanted.clamp(lo, i64::from(max)) as u16)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Modèle connecté, bande en marche, limites du RUN 7 lues.
    fn running_model() -> Model {
        let mut m = Model::new();
        m.on_connected();
        for r in [
            Reading::SpeedMin(8),
            Reading::SpeedMax(200),
            Reading::InclineMin(0),
            Reading::InclineMax(150),
            Reading::Online(0),
        ] {
            m.apply(r);
        }
        m
    }

    fn approx(a: Option<f32>, b: f32) -> bool {
        a.is_some_and(|a| (a - b).abs() < 1e-4)
    }

    /// Décode une réponse (instruction 3) construite comme le tapis l'enverrait.
    fn answer(id: u16, data: &[u8]) -> Message {
        use crate::kettler::protocol::{encode_frame, FrameDecoder};
        let mut payload = id.to_be_bytes().to_vec();
        payload.push(3);
        payload.extend_from_slice(&(data.len() as u16).to_be_bytes());
        payload.extend_from_slice(data);
        let mut msgs = FrameDecoder::new().push(&encode_frame(&payload));
        assert_eq!(msgs.len(), 1);
        msgs.remove(0).expect("trame valide")
    }

    #[test]
    fn decodage_des_reponses() {
        assert_eq!(decode(&answer(35, &[0, 25])), Some(Reading::SpeedGet(25)));
        assert_eq!(decode(&answer(15, &[0, 182])), Some(Reading::Distance(182)));
        assert_eq!(decode(&answer(38, &[0])), Some(Reading::Online(0)));
        assert_eq!(decode(&answer(38, &[1])), Some(Reading::Online(1)));
        assert_eq!(
            decode(&answer(20, b"RUN 7")),
            Some(Reading::DeviceName("RUN 7".into()))
        );
        // Valeurs ignorées : DeviceType, identifiant inconnu, données mal formées.
        assert_eq!(decode(&answer(24, b"T")), None);
        assert_eq!(decode(&answer(2, &[1, 58])), None);
        assert_eq!(decode(&answer(35, &[25])), None);
        // Une écriture (instruction 2) n'est pas une réponse.
        let mut write = answer(34, &[0, 25]);
        write.instruction = 2;
        assert_eq!(decode(&write), None);
    }

    #[test]
    fn reponse_au_handshake() {
        let msg = answer(1, &[0x2d; 18]);
        assert!(is_handshake_answer(&msg));
        assert_eq!(decode(&msg), None);
        assert!(!is_handshake_answer(&answer(35, &[0, 25])));
    }

    #[test]
    fn nom_du_tapis() {
        assert!(is_treadmill_name("RUN 7"));
        assert!(is_treadmill_name("run7"));
        assert!(is_treadmill_name(" Run  7 "));
        assert!(!is_treadmill_name("RUN 70"));
        assert!(!is_treadmill_name("TOUR"));
        assert!(!is_treadmill_name(""));
    }

    #[test]
    fn conversion_des_valeurs() {
        let mut m = Model::new();
        m.apply(Reading::SpeedGet(29));
        m.apply(Reading::SpeedSet(30));
        m.apply(Reading::InclineGet(5));
        m.apply(Reading::InclineSet(10));
        m.apply(Reading::Distance(182));
        m.apply(Reading::Energy(105));
        m.apply(Reading::Pulse(89));
        let s = m.snapshot();
        assert!(approx(s.speed_kmh, 2.9));
        assert!(approx(s.target_speed_kmh, 3.0));
        assert!(approx(s.incline_pct, 0.5));
        assert!(approx(s.target_incline_pct, 1.0));
        assert_eq!(s.distance_m, Some(182));
        assert!(approx(s.energy_kcal, 10.5));
        assert_eq!(s.pulse_bpm, Some(89));

        m.apply(Reading::Pulse(0));
        assert_eq!(m.snapshot().pulse_bpm, None, "0 = pas de contact");
    }

    #[test]
    fn online_est_inverse() {
        let mut m = Model::new();
        assert_eq!(m.running(), None);
        m.apply(Reading::Online(0));
        assert_eq!(m.running(), Some(true));
        m.apply(Reading::Online(1));
        assert_eq!(m.running(), Some(false));
        m.apply(Reading::Online(7)); // inconnu : ignoré
        assert_eq!(m.running(), Some(false));
    }

    #[test]
    fn limites_et_identite_completes_seulement() {
        let mut m = Model::new();
        m.apply(Reading::SpeedMin(8));
        m.apply(Reading::SpeedMax(200));
        m.apply(Reading::InclineMin(0));
        assert!(m.snapshot().limits.is_none());
        m.apply(Reading::InclineMax(150));
        let l = m.snapshot().limits.unwrap();
        assert!(approx(Some(l.speed_min_kmh), 0.8));
        assert!(approx(Some(l.speed_max_kmh), 20.0));
        assert!(approx(Some(l.incline_max_pct), 15.0));

        m.apply(Reading::DeviceName("RUN 7".into()));
        assert!(m.snapshot().device.is_none());
        m.apply(Reading::DeviceId("STB1T0158".into()));
        let d = m.snapshot().device.unwrap();
        assert_eq!(d.name, "RUN 7");
        assert_eq!(d.article, None);
        m.apply(Reading::Article("07883-600".into()));
        assert_eq!(
            m.snapshot().device.unwrap().article.as_deref(),
            Some("07883-600")
        );
    }

    #[test]
    fn valeurs_statiques_manquantes() {
        let mut m = Model::new();
        assert_eq!(m.missing_static_ids(true).len(), 7);
        assert_eq!(m.missing_static_ids(false).len(), 6);
        m.apply(Reading::DeviceName("RUN 7".into()));
        m.apply(Reading::SpeedMax(200));
        let ids = m.missing_static_ids(true);
        assert!(!ids.contains(&ValueId::DeviceName));
        assert!(!ids.contains(&ValueId::SpeedMax));
        assert!(ids.contains(&ValueId::Article));
    }

    #[test]
    fn vitesse_arrondie_et_bornee() {
        let m = running_model();
        assert_eq!(m.speed_command(2.54), Ok(25));
        assert_eq!(m.speed_command(2.56), Ok(26));
        // Sous le minimum du tapis : ramenée à 0,8 km/h.
        assert_eq!(m.speed_command(0.0), Ok(8));
        assert_eq!(m.speed_command(-3.0), Ok(8));
        // Au-dessus du maximum du tapis : ramenée à 20,0 km/h.
        assert_eq!(m.speed_command(12.0), Ok(120));
        assert_eq!(m.speed_command(25.0), Ok(200));
        assert!(m.speed_command(f32::NAN).is_err());
        assert!(m.speed_command(f32::INFINITY).is_err());
    }

    #[test]
    fn vitesse_max_annoncee_par_le_tapis() {
        let mut m = running_model();
        m.apply(Reading::SpeedMax(50)); // tapis bridé à 5,0 km/h
        assert_eq!(m.speed_command(18.0), Ok(50));
    }

    #[test]
    fn inclinaison_arrondie_bornee() {
        let m = running_model();
        assert_eq!(m.incline_command(1.04), Ok(10));
        assert_eq!(m.incline_command(-2.0), Ok(0));
        assert_eq!(m.incline_command(40.0), Ok(150));
        assert!(m.incline_command(f32::NAN).is_err());
    }

    #[test]
    fn consignes_refusees_si_la_bande_ne_tourne_pas() {
        let mut m = running_model();
        m.apply(Reading::Online(1));
        assert_eq!(m.speed_command(3.0), Err(ERR_NOT_RUNNING.into()));
        assert_eq!(m.incline_command(1.0), Err(ERR_NOT_RUNNING.into()));

        // État inconnu : refusé aussi.
        let mut m = Model::new();
        m.on_connected();
        m.apply(Reading::SpeedMin(8));
        assert_eq!(m.speed_command(3.0), Err(ERR_RUNNING_UNKNOWN.into()));

        // Non connecté (même si la dernière valeur lue disait « en marche »).
        let mut m = running_model();
        m.on_link_lost();
        assert_eq!(m.speed_command(3.0), Err(ERR_NOT_CONNECTED.into()));
        assert_eq!(m.incline_command(1.0), Err(ERR_NOT_CONNECTED.into()));
    }

    #[test]
    fn consigne_refusee_sans_limites() {
        let mut m = Model::new();
        m.on_connected();
        m.apply(Reading::Online(0));
        assert_eq!(m.speed_command(3.0), Err(ERR_LIMITS_UNKNOWN.into()));
        assert_eq!(m.incline_command(1.0), Err(ERR_LIMITS_UNKNOWN.into()));
    }

    #[test]
    fn demarrage() {
        let mut m = Model::new();
        assert_eq!(m.check_start(), Err(ERR_NOT_CONNECTED.into()));
        m.on_connected();
        assert_eq!(m.check_start(), Err(ERR_RUNNING_UNKNOWN.into()));
        m.apply(Reading::Online(1));
        assert_eq!(m.check_start(), Ok(()));
        m.apply(Reading::Online(0));
        assert_eq!(m.check_start(), Err(ERR_ALREADY_RUNNING.into()));
    }

    #[test]
    fn temps_de_course() {
        let t0 = Instant::now();
        let s = Duration::from_secs;
        let mut m = Model::new();
        m.on_connected();
        m.apply(Reading::Online(1));
        m.tick(t0);
        m.tick(t0 + s(10));
        assert_eq!(m.snapshot().elapsed_s, 0, "bande arrêtée");

        m.apply(Reading::Online(0));
        m.tick(t0 + s(12));
        m.tick(t0 + s(15));
        assert_eq!(m.snapshot().elapsed_s, 5);

        // Perte de liaison : rien ne s'accumule, et la reprise ne compte pas le trou.
        m.on_link_lost();
        m.tick(t0 + s(30));
        m.on_connected();
        assert_eq!(m.running(), None, "état à relire après reconnexion");
        m.apply(Reading::Online(0));
        m.tick(t0 + s(40));
        assert_eq!(m.snapshot().elapsed_s, 5);
        m.tick(t0 + s(41));
        assert_eq!(m.snapshot().elapsed_s, 6);

        m.apply(Reading::Online(1));
        m.tick(t0 + s(50));
        assert_eq!(m.snapshot().elapsed_s, 6, "pause");
    }

    #[test]
    fn remise_a_zero_de_la_distance() {
        let t0 = Instant::now();
        let mut m = Model::new();
        m.on_connected();
        m.apply(Reading::Online(0));
        m.apply(Reading::Distance(100));
        m.tick(t0);
        m.tick(t0 + Duration::from_secs(30));
        m.apply(Reading::Distance(140));
        m.apply(Reading::Distance(140));
        assert_eq!(m.snapshot().elapsed_s, 30, "distance stable ou croissante");
        m.apply(Reading::Distance(0));
        assert_eq!(m.snapshot().elapsed_s, 0, "compteur remis à zéro");
        assert_eq!(m.snapshot().distance_m, Some(0));
    }

    #[test]
    fn remise_a_zero_detectee_apres_deconnexion() {
        let t0 = Instant::now();
        let mut m = Model::new();
        m.on_connected();
        m.apply(Reading::Online(0));
        m.apply(Reading::Distance(500));
        m.tick(t0);
        m.tick(t0 + Duration::from_secs(60));
        m.on_disconnected();
        assert_eq!(m.snapshot().elapsed_s, 60, "temps gardé");
        m.on_connected();
        m.apply(Reading::Distance(20));
        assert_eq!(m.snapshot().elapsed_s, 0);
    }

    #[test]
    fn perte_de_liaison_bande_en_marche() {
        let mut m = running_model();
        m.on_link_lost();
        let s = m.snapshot();
        assert_eq!(s.connection, ConnectionState::Reconnecting);
        assert!(s.lost_while_running);
        m.on_connected();
        assert!(!m.snapshot().lost_while_running, "effacé au succès");

        let mut m = running_model();
        m.apply(Reading::Online(1));
        m.on_link_lost();
        assert!(!m.snapshot().lost_while_running, "bande arrêtée");

        let mut m = Model::new();
        m.on_link_lost();
        assert!(!m.snapshot().lost_while_running, "état inconnu");
    }

    #[test]
    fn deconnexion_voulue() {
        let mut m = running_model();
        m.apply(Reading::SpeedGet(30));
        m.on_link_lost();
        m.on_disconnected();
        let s = m.snapshot();
        assert_eq!(s.connection, ConnectionState::Disconnected);
        assert!(!s.lost_while_running);
        assert_eq!(s.running, None);
        assert_eq!(s.speed_kmh, None);
        assert!(s.limits.is_none());
    }

    #[test]
    fn connexion_efface_l_erreur() {
        let mut m = Model::new();
        m.set_error(Some("oups".into()));
        m.on_connected();
        assert_eq!(m.snapshot().last_error, None);
    }

    fn profil(segments: &[(u32, f32, f32)]) -> RunProfile {
        use crate::dto::ProfileSegment;
        RunProfile {
            id: "p".into(),
            name: "Profil".into(),
            builtin: false,
            segments: segments
                .iter()
                .map(|&(distance_m, speed_kmh, incline_pct)| ProfileSegment {
                    distance_m,
                    speed_kmh,
                    incline_pct,
                })
                .collect(),
        }
    }

    #[test]
    fn profil_attend_la_fin_du_compte_a_rebours() {
        let t0 = Instant::now();
        let mut m = running_model();
        m.select_profile(Some(profil(&[(100, 4.0, 2.0)])));
        m.apply(Reading::SpeedSet(0)); // compte à rebours : consigne encore à 0
        m.apply(Reading::InclineSet(0));
        assert_eq!(m.profile_order(t0), None);
        m.apply(Reading::SpeedSet(8)); // bande partie à 0,8 km/h
        assert_eq!(
            m.profile_order(t0),
            Some(ProfileOrder { speed: Some(40), incline: Some(20) })
        );
    }

    #[test]
    fn profil_borne_a_la_vitesse_max_et_n_envoie_que_ce_qui_differe() {
        let t0 = Instant::now();
        let mut m = running_model();
        m.apply(Reading::SpeedMax(50)); // tapis bridé à 5,0 km/h
        m.select_profile(Some(profil(&[(100, 8.0, 1.0)])));
        m.apply(Reading::SpeedSet(30));
        m.apply(Reading::InclineSet(10));
        assert_eq!(
            m.profile_order(t0),
            Some(ProfileOrder { speed: Some(50), incline: None }),
            "bornée à 5,0 ; inclinaison déjà en place"
        );
        m.apply(Reading::SpeedSet(50));
        assert_eq!(m.profile_order(t0 + Duration::from_secs(1)), None, "relue");
        assert_eq!(m.snapshot().profile.unwrap().segment_index, 0);
    }

    #[test]
    fn profil_rien_bande_arretee_ou_liaison_perdue() {
        let t0 = Instant::now();
        let mut m = running_model();
        m.select_profile(Some(profil(&[(100, 4.0, 0.0)])));
        m.apply(Reading::SpeedSet(8));
        m.apply(Reading::Online(1));
        assert_eq!(m.profile_order(t0), None);
        m.apply(Reading::Online(0));
        m.on_link_lost();
        assert_eq!(m.profile_order(t0), None);
    }

    #[test]
    fn profil_remise_a_zero_pendant_un_demarrage() {
        let mut m = running_model();
        m.select_profile(Some(profil(&[(100, 4.0, 0.0), (100, 5.0, 0.0)])));
        m.apply(Reading::Distance(0));
        m.apply(Reading::Distance(150));
        m.set_start_sequence(true);
        m.apply(Reading::Distance(0));
        m.set_start_sequence(false);
        assert_eq!(m.snapshot().profile.unwrap().done_m, 150);
        m.apply(Reading::Distance(0)); // déjà à 0 : rien ne change
        m.apply(Reading::Distance(30));
        m.apply(Reading::Distance(0)); // fin de séance
        assert_eq!(m.snapshot().profile.unwrap().done_m, 0);
    }

    #[test]
    fn choix_du_profil() {
        let mut m = running_model();
        assert!(m.snapshot().profile.is_none());
        m.select_profile(Some(profil(&[(100, 4.0, 0.0), (100, 5.0, 0.0)])));
        m.apply(Reading::Distance(0));
        m.apply(Reading::Distance(120));
        // Le même profil, modifié : progression gardée.
        m.select_profile(Some(profil(&[(100, 4.0, 0.0), (300, 5.0, 0.0)])));
        let p = m.snapshot().profile.unwrap();
        assert_eq!((p.done_m, p.total_m), (120, 400));
        assert_eq!(m.profile_id(), Some("p"));
        m.select_profile(None);
        assert!(m.snapshot().profile.is_none());
    }

    #[test]
    fn publication_seulement_si_changement() {
        let mut m = Model::new();
        assert!(m.take_changed().is_some());
        assert!(m.take_changed().is_none());
        m.apply(Reading::SpeedGet(10));
        assert!(m.take_changed().is_some());
        m.apply(Reading::SpeedGet(10));
        assert!(m.take_changed().is_none());
    }
}
