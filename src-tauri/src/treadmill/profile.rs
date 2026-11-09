//! Profils de course : profils de départ, validation, stockage sur disque, et
//! déroulement d'un profil pendant la séance (logique pure, sans I/O tapis).
//!
//! Déroulement (`ProfileRun`) :
//! - la progression est la SOMME des avancées du compteur de distance du tapis,
//!   pas sa valeur brute : quand le tapis remet son compteur à zéro pendant une
//!   séquence de démarrage (démarrage ignoré après une longue pause, voir
//!   `Treadmill::start`), le profil continue là où il en était. Une remise à
//!   zéro hors démarrage (second STOP = fin de séance) le ramène au début ;
//! - à chaque changement de segment, les consignes du segment sont envoyées une
//!   fois, puis vérifiées à la relecture ; sans confirmation, elles sont
//!   renvoyées (au plus `MAX_ATTEMPTS` envois). Ensuite le profil ne touche plus
//!   à rien jusqu'au segment suivant : un réglage manuel (appli ou console) vaut
//!   jusqu'à la fin du segment ;
//! - après le dernier segment : récupération lente (`RECOVERY_SPEED_KMH`, 0 %),
//!   sans arrêt automatique ;
//! - bande vue arrêtée (pause) : les consignes du segment en cours seront
//!   renvoyées à la reprise (le tapis remet SpeedSet à 0 à l'arrêt).

use std::path::PathBuf;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::dto::{ProfileProgress, ProfileSegment, RunProfile};

/// Récupération après le dernier segment : vitesse (au plus celle du dernier
/// segment, on ne fait jamais accélérer) et inclinaison.
pub const RECOVERY_SPEED_KMH: f32 = 3.0;
pub const RECOVERY_INCLINE_PCT: f32 = 0.0;

/// Bornes d'un segment. Vitesse et inclinaison : celles du RUN 7 (le profil
/// s'édite aussi tapis déconnecté) ; le Rust borne encore aux limites lues à l'envoi.
pub const SEGMENT_MIN_M: u32 = 50;
pub const SEGMENT_MAX_M: u32 = 10_000;
pub const SPEED_MIN_KMH: f32 = 0.8;
pub const SPEED_MAX_KMH: f32 = 20.0;
pub const INCLINE_MIN_PCT: f32 = 0.0;
pub const INCLINE_MAX_PCT: f32 = 15.0;
pub const MAX_SEGMENTS: usize = 60;
pub const NAME_MAX_CHARS: usize = 40;

/// Délai pour voir les consignes envoyées relues sur le tapis (mesuré : ~1,4 s).
const CONFIRM: Duration = Duration::from_secs(5);
/// Envois au plus des consignes d'un même segment (écriture perdue).
const MAX_ATTEMPTS: u8 = 3;

const FILE_VERSION: u32 = 1;

const ERR_BUILTIN: &str = "Profil de départ en lecture seule : dupliquez-le pour le modifier.";
const ERR_UNKNOWN: &str = "Profil introuvable.";

/// Nombre à une décimale avec une virgule (messages d'erreur).
fn fr(x: f32) -> String {
    format!("{x:.1}").replace('.', ",")
}

fn round1(x: f32) -> f32 {
    (x * 10.0).round() / 10.0
}

fn seg(distance_m: u32, speed_kmh: f32, incline_pct: f32) -> ProfileSegment {
    ProfileSegment {
        distance_m,
        speed_kmh,
        incline_pct,
    }
}

/// Profils fournis avec l'application (marche rapide : 6,0 km/h au plus).
pub fn starter_profiles() -> Vec<RunProfile> {
    let builtin = |id: &str, name: &str, segments: Vec<ProfileSegment>| RunProfile {
        id: id.into(),
        name: name.into(),
        builtin: true,
        segments,
    };
    let mut fractionne = vec![seg(400, 4.0, 0.0)];
    for _ in 0..4 {
        fractionne.push(seg(200, 6.0, 0.0));
        fractionne.push(seg(200, 4.0, 0.0));
    }
    fractionne.push(seg(300, 3.5, 0.0));
    vec![
        builtin(
            "depart-marche-douce",
            "Marche douce",
            vec![
                seg(300, 3.0, 0.0),
                seg(500, 4.0, 0.0),
                seg(700, 4.5, 1.0),
                seg(300, 3.5, 0.0),
            ],
        ),
        builtin(
            "depart-collines",
            "Collines",
            vec![
                seg(300, 4.0, 0.0),
                seg(300, 4.5, 3.0),
                seg(200, 4.5, 0.0),
                seg(300, 4.5, 5.0),
                seg(200, 4.5, 0.0),
                seg(300, 4.0, 7.0),
                seg(400, 3.5, 0.0),
            ],
        ),
        builtin("depart-fractionne", "Fractionné", fractionne),
    ]
}

/// Vérifie un profil et le normalise (nom sans espaces autour, consignes au
/// dixième). Les erreurs sont lisibles telles quelles.
pub fn validate(profile: &RunProfile) -> Result<RunProfile, String> {
    let name = profile.name.trim().to_string();
    if name.is_empty() {
        return Err("Donnez un nom au profil.".into());
    }
    if name.chars().count() > NAME_MAX_CHARS {
        return Err(format!("Nom trop long ({NAME_MAX_CHARS} caractères au plus)."));
    }
    if profile.segments.is_empty() {
        return Err("Le profil doit avoir au moins un segment.".into());
    }
    if profile.segments.len() > MAX_SEGMENTS {
        return Err(format!("{MAX_SEGMENTS} segments au plus."));
    }
    let mut segments = Vec::with_capacity(profile.segments.len());
    for (i, s) in profile.segments.iter().enumerate() {
        let n = i + 1;
        if !(SEGMENT_MIN_M..=SEGMENT_MAX_M).contains(&s.distance_m) {
            return Err(format!(
                "Segment {n} : distance entre {SEGMENT_MIN_M} et {SEGMENT_MAX_M} m."
            ));
        }
        let speed = round1(s.speed_kmh);
        if !speed.is_finite() || !(SPEED_MIN_KMH..=SPEED_MAX_KMH).contains(&speed) {
            return Err(format!(
                "Segment {n} : vitesse entre {} et {} km/h.",
                fr(SPEED_MIN_KMH),
                fr(SPEED_MAX_KMH)
            ));
        }
        let incline = round1(s.incline_pct);
        if !incline.is_finite() || !(INCLINE_MIN_PCT..=INCLINE_MAX_PCT).contains(&incline) {
            return Err(format!(
                "Segment {n} : inclinaison entre {} et {} %.",
                fr(INCLINE_MIN_PCT),
                fr(INCLINE_MAX_PCT)
            ));
        }
        segments.push(seg(s.distance_m, speed, incline));
    }
    Ok(RunProfile {
        id: profile.id.clone(),
        name,
        builtin: false,
        segments,
    })
}

// ── Déroulement ─────────────────────────────────────────────────────────────

/// Consignes envoyées pour le segment en cours, pas encore relues.
#[derive(Debug, Clone)]
struct Pending {
    sent_at: Instant,
    attempts: u8,
}

/// Profil choisi et sa progression dans la séance.
#[derive(Debug, Clone)]
pub struct ProfileRun {
    profile: RunProfile,
    /// Mètres parcourus depuis le début du profil.
    done_m: u32,
    /// Dernière valeur lue du compteur de distance du tapis.
    last_distance: Option<u32>,
    /// Segment dont les consignes ont été envoyées (ou trouvées déjà en place).
    applied: Option<usize>,
    pending: Option<Pending>,
    /// Consigne changée à la main pendant le segment en cours.
    manual: bool,
}

impl ProfileRun {
    pub fn new(profile: RunProfile) -> Self {
        ProfileRun {
            profile,
            done_m: 0,
            last_distance: None,
            applied: None,
            pending: None,
            manual: false,
        }
    }

    pub fn profile(&self) -> &RunProfile {
        &self.profile
    }

    /// Profil modifié (bande arrêtée) : la progression est gardée, les
    /// consignes seront renvoyées.
    pub fn replace_profile(&mut self, profile: RunProfile) {
        self.profile = profile;
        self.applied = None;
        self.pending = None;
    }

    pub fn total_m(&self) -> u32 {
        self.profile.segments.iter().map(|s| s.distance_m).sum()
    }

    /// Segment en cours ; `segments.len()` une fois le profil terminé.
    pub fn index(&self) -> usize {
        let mut end = 0u32;
        for (i, s) in self.profile.segments.iter().enumerate() {
            end += s.distance_m;
            if self.done_m < end {
                return i;
            }
        }
        self.profile.segments.len()
    }

    /// Fin de séance (second arrêt, bande déjà arrêtée) : retour au début. Le
    /// compteur du tapis repart de zéro, la prochaine lecture sert de départ.
    pub fn restart(&mut self) {
        self.done_m = 0;
        self.last_distance = None;
        self.applied = None;
        self.pending = None;
        self.manual = false;
    }

    /// Consignes voulues pour le segment `idx` (récupération au-delà du dernier).
    pub fn target(&self, idx: usize) -> ProfileSegment {
        match self.profile.segments.get(idx) {
            Some(s) => s.clone(),
            None => {
                let last = self.profile.segments.last().map_or(RECOVERY_SPEED_KMH, |s| s.speed_kmh);
                seg(0, last.min(RECOVERY_SPEED_KMH), RECOVERY_INCLINE_PCT)
            }
        }
    }

    /// Nouvelle lecture du compteur de distance du tapis. `in_start_sequence` :
    /// une séquence de démarrage de l'application est en cours (sa remise à
    /// zéro éventuelle n'est pas une fin de séance).
    pub fn on_distance(&mut self, d: u32, in_start_sequence: bool) {
        match self.last_distance {
            None => {} // première lecture : point de départ
            Some(prev) if d >= prev => self.done_m = self.done_m.saturating_add(d - prev),
            Some(_) if in_start_sequence => {
                // Compteur remis à zéro pour pouvoir redémarrer : on continue.
                self.done_m = self.done_m.saturating_add(d);
            }
            Some(_) => {
                // Fin de séance (second arrêt) ou tapis rallumé : retour au début.
                self.done_m = d;
                self.applied = None;
                self.pending = None;
                self.manual = false;
            }
        }
        self.last_distance = Some(d);
    }

    /// Bande vue arrêtée : à la reprise, les consignes du segment seront renvoyées.
    pub fn on_stopped(&mut self) {
        self.applied = None;
        self.pending = None;
        self.manual = false;
    }

    /// Consigne changée à la main : plus rien n'est renvoyé jusqu'au segment suivant.
    pub fn on_manual(&mut self) {
        self.pending = None;
        self.manual = true;
    }

    /// Faut-il (r)envoyer les consignes du segment en cours ? `matches` : les
    /// consignes relues sur le tapis valent déjà celles voulues (bornées).
    pub fn step(&mut self, now: Instant, matches: bool) -> bool {
        let idx = self.index();
        if self.applied != Some(idx) {
            self.applied = Some(idx);
            self.manual = false;
            if matches {
                self.pending = None;
                return false;
            }
            self.pending = Some(Pending {
                sent_at: now,
                attempts: 1,
            });
            return true;
        }
        let Some(p) = self.pending.as_mut() else {
            return false;
        };
        if matches {
            self.pending = None;
            return false;
        }
        if now.saturating_duration_since(p.sent_at) < CONFIRM {
            return false;
        }
        if p.attempts >= MAX_ATTEMPTS {
            // Toujours pas relues : sans doute changées à la console. On s'arrête là.
            self.pending = None;
            return false;
        }
        p.attempts += 1;
        p.sent_at = now;
        true
    }

    pub fn progress(&self) -> ProfileProgress {
        let idx = self.index();
        let count = self.profile.segments.len();
        let segment_end: u32 = self.profile.segments.iter().take(idx + 1).map(|s| s.distance_m).sum();
        ProfileProgress {
            profile_id: self.profile.id.clone(),
            profile_name: self.profile.name.clone(),
            segment_count: count as u32,
            segment_index: idx as u32,
            done_m: self.done_m,
            total_m: self.total_m(),
            segment_remaining_m: if idx < count {
                segment_end.saturating_sub(self.done_m)
            } else {
                0
            },
            current: self.target(idx),
            next: self.profile.segments.get(idx + 1).cloned(),
            finished: idx >= count,
            manual: self.manual,
        }
    }
}

// ── Stockage ────────────────────────────────────────────────────────────────

/// Contenu du fichier `profiles.json` (dossier de données de l'application).
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StoredFile {
    version: u32,
    selected: Option<String>,
    profiles: Vec<RunProfile>,
}

/// Profils de l'utilisateur (sur disque) et profils de départ (en mémoire).
#[derive(Debug)]
pub struct ProfileLibrary {
    /// `None` : rien n'est écrit (tests).
    path: Option<PathBuf>,
    user: Vec<RunProfile>,
    selected: Option<String>,
}

impl ProfileLibrary {
    /// Bibliothèque sans fichier (tests, dossier de données indisponible).
    pub fn in_memory() -> Self {
        ProfileLibrary {
            path: None,
            user: Vec::new(),
            selected: None,
        }
    }

    /// Charge `path`. Fichier absent : bibliothèque vide. Fichier illisible : mis
    /// de côté (`.illisible`) pour ne pas l'écraser, et bibliothèque vide.
    pub fn load(path: PathBuf) -> Self {
        let mut lib = ProfileLibrary {
            path: Some(path.clone()),
            ..Self::in_memory()
        };
        let Ok(text) = std::fs::read_to_string(&path) else {
            return lib;
        };
        match serde_json::from_str::<StoredFile>(&text) {
            Ok(file) => {
                let mut user: Vec<RunProfile> = Vec::new();
                for p in file.profiles {
                    match validate(&p) {
                        Ok(v) if !v.id.is_empty() && !lib.is_builtin(&v.id) => {
                            if !user.iter().any(|u| u.id == v.id) {
                                user.push(v);
                            }
                        }
                        _ => eprintln!("[profils] profil ignoré (invalide) : {}", p.name),
                    }
                }
                lib.user = user;
                lib.selected = file.selected.filter(|id| lib.get(id).is_some());
            }
            Err(e) => {
                let aside = path.with_extension("json.illisible");
                eprintln!("[profils] {} illisible ({e}) : mis de côté", path.display());
                let _ = std::fs::rename(&path, aside);
            }
        }
        lib
    }

    fn is_builtin(&self, id: &str) -> bool {
        starter_profiles().iter().any(|p| p.id == id)
    }

    /// Profils de départ, puis ceux de l'utilisateur par ordre alphabétique.
    pub fn list(&self) -> Vec<RunProfile> {
        let mut user = self.user.clone();
        user.sort_by_key(|p| p.name.to_lowercase());
        let mut all = starter_profiles();
        all.extend(user);
        all
    }

    pub fn get(&self, id: &str) -> Option<RunProfile> {
        starter_profiles()
            .into_iter()
            .chain(self.user.iter().cloned())
            .find(|p| p.id == id)
    }

    pub fn selected(&self) -> Option<&str> {
        self.selected.as_deref()
    }

    /// Enregistre (crée si `id` est vide) ; rend le profil tel qu'enregistré.
    pub fn save(&mut self, profile: &RunProfile) -> Result<RunProfile, String> {
        let mut p = validate(profile)?;
        if self.is_builtin(&p.id) {
            return Err(ERR_BUILTIN.into());
        }
        if p.id.is_empty() {
            p.id = self.new_id();
            self.user.push(p.clone());
        } else {
            let slot = self.user.iter_mut().find(|u| u.id == p.id).ok_or(ERR_UNKNOWN)?;
            *slot = p.clone();
        }
        self.persist()?;
        Ok(p)
    }

    pub fn delete(&mut self, id: &str) -> Result<(), String> {
        if self.is_builtin(id) {
            return Err("Les profils de départ ne peuvent pas être supprimés.".into());
        }
        let before = self.user.len();
        self.user.retain(|p| p.id != id);
        if self.user.len() == before {
            return Err(ERR_UNKNOWN.into());
        }
        if self.selected.as_deref() == Some(id) {
            self.selected = None;
        }
        self.persist()
    }

    pub fn set_selected(&mut self, id: Option<String>) -> Result<(), String> {
        if let Some(id) = &id {
            if self.get(id).is_none() {
                return Err(ERR_UNKNOWN.into());
            }
        }
        self.selected = id;
        self.persist()
    }

    fn new_id(&self) -> String {
        let ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_millis());
        let mut n = 0;
        loop {
            let id = format!("p{ms}-{n}");
            if self.get(&id).is_none() {
                return id;
            }
            n += 1;
        }
    }

    /// Écrit le fichier (via un fichier temporaire renommé : jamais à moitié écrit).
    fn persist(&self) -> Result<(), String> {
        let Some(path) = &self.path else {
            return Ok(());
        };
        let file = StoredFile {
            version: FILE_VERSION,
            selected: self.selected.clone(),
            profiles: self.user.clone(),
        };
        let fail = |e: String| format!("Profils non enregistrés sur le disque : {e}");
        let json = serde_json::to_string_pretty(&file).map_err(|e| fail(e.to_string()))?;
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| fail(e.to_string()))?;
        }
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, json).map_err(|e| fail(e.to_string()))?;
        std::fs::rename(&tmp, path).map_err(|e| fail(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn profile(segments: Vec<ProfileSegment>) -> RunProfile {
        RunProfile {
            id: "t".into(),
            name: "Test".into(),
            builtin: false,
            segments,
        }
    }

    #[test]
    fn profils_de_depart_valides() {
        let all = starter_profiles();
        assert_eq!(all.len(), 3);
        for p in &all {
            assert!(p.builtin);
            validate(p).unwrap_or_else(|e| panic!("{} : {e}", p.name));
            assert!(p.segments.iter().all(|s| s.speed_kmh <= 6.0), "{}", p.name);
        }
    }

    #[test]
    fn validation() {
        let ok = profile(vec![seg(100, 3.04, 1.06)]);
        let v = validate(&RunProfile {
            name: "  Matin  ".into(),
            ..ok.clone()
        })
        .unwrap();
        assert_eq!(v.name, "Matin");
        assert_eq!(v.segments[0].speed_kmh, 3.0);
        assert_eq!(v.segments[0].incline_pct, 1.1);

        let err = |p: RunProfile| validate(&p).unwrap_err();
        assert!(err(RunProfile { name: " ".into(), ..ok.clone() }).contains("nom"));
        assert!(err(profile(vec![])).contains("au moins un segment"));
        assert!(err(profile(vec![seg(100, 3.0, 0.0), seg(20, 3.0, 0.0)])).starts_with("Segment 2"));
        assert!(err(profile(vec![seg(100, 0.5, 0.0)])).contains("vitesse entre 0,8 et 20,0"));
        assert!(err(profile(vec![seg(100, 3.0, 16.0)])).contains("inclinaison"));
        assert!(err(profile(vec![seg(100, f32::NAN, 0.0)])).contains("vitesse"));
        // Un profil reçu « de départ » est enregistré comme profil de l'utilisateur.
        assert!(!validate(&RunProfile { builtin: true, ..ok }).unwrap().builtin);
    }

    #[test]
    fn progression_et_segments() {
        let mut run = ProfileRun::new(profile(vec![seg(100, 3.0, 0.0), seg(200, 4.0, 2.0)]));
        assert_eq!(run.total_m(), 300);
        run.on_distance(500, false); // point de départ, quel que soit le compteur
        assert_eq!(run.index(), 0);
        run.on_distance(599, false);
        let p = run.progress();
        assert_eq!((p.done_m, p.segment_index, p.segment_remaining_m), (99, 0, 1));
        assert_eq!(p.next, Some(seg(200, 4.0, 2.0)));
        run.on_distance(600, false);
        assert_eq!(run.index(), 1);
        assert_eq!(run.progress().next, None);
        run.on_distance(800, false);
        assert_eq!(run.index(), 2);
        let p = run.progress();
        assert!(p.finished);
        assert_eq!((p.segment_index, p.segment_remaining_m), (2, 0));
        assert_eq!(p.current, seg(0, 3.0, 0.0), "récupération");
    }

    #[test]
    fn recuperation_jamais_plus_rapide_que_le_dernier_segment() {
        let run = ProfileRun::new(profile(vec![seg(100, 2.0, 4.0)]));
        assert_eq!(run.target(1), seg(0, 2.0, 0.0));
    }

    #[test]
    fn remises_a_zero_du_compteur() {
        let mut run = ProfileRun::new(profile(vec![seg(100, 3.0, 0.0), seg(200, 4.0, 2.0)]));
        run.on_distance(0, false);
        run.on_distance(150, false);
        assert_eq!(run.index(), 1);
        // Remise à zéro pendant un démarrage (longue pause) : on continue.
        run.on_distance(3, true);
        assert_eq!(run.progress().done_m, 153);
        run.on_distance(10, false);
        assert_eq!(run.progress().done_m, 160);
        // Remise à zéro hors démarrage : fin de séance, retour au début.
        run.on_distance(0, false);
        assert_eq!(run.progress().done_m, 0);
        assert_eq!(run.index(), 0);
        // Fin de séance signalée (compteur déjà à 0) : idem.
        run.on_distance(40, false);
        run.restart();
        run.on_distance(0, false);
        assert_eq!(run.progress().done_m, 0);
    }

    #[test]
    fn envoi_confirmation_et_renvois() {
        let t0 = Instant::now();
        let s = Duration::from_secs;
        let mut run = ProfileRun::new(profile(vec![seg(100, 3.0, 0.0), seg(200, 4.0, 2.0)]));
        run.on_distance(0, false);

        assert!(run.step(t0, false), "premier envoi");
        assert!(!run.step(t0 + s(1), false), "on attend la relecture");
        assert!(!run.step(t0 + s(2), true), "relu : confirmé");
        assert!(!run.step(t0 + s(20), false), "puis plus rien (console)");

        // Segment suivant : jamais confirmé → 3 envois au plus.
        run.on_distance(100, false);
        assert!(run.step(t0 + s(30), false));
        assert!(!run.step(t0 + s(33), false));
        assert!(run.step(t0 + s(36), false), "2e envoi");
        assert!(run.step(t0 + s(42), false), "3e envoi");
        assert!(!run.step(t0 + s(48), false), "abandon");
        assert!(!run.step(t0 + s(60), false));
    }

    #[test]
    fn deja_en_place_rien_a_envoyer() {
        let mut run = ProfileRun::new(profile(vec![seg(100, 3.0, 0.0)]));
        assert!(!run.step(Instant::now(), true));
    }

    #[test]
    fn reglage_manuel_jusqu_au_segment_suivant() {
        let t0 = Instant::now();
        let mut run = ProfileRun::new(profile(vec![seg(100, 3.0, 0.0), seg(200, 4.0, 2.0)]));
        run.on_distance(0, false);
        assert!(run.step(t0, false));
        run.on_manual();
        assert!(run.progress().manual);
        assert!(!run.step(t0 + Duration::from_secs(10), false), "pas de renvoi");
        run.on_distance(100, false);
        assert!(run.step(t0 + Duration::from_secs(11), false), "segment suivant");
        assert!(!run.progress().manual);
    }

    #[test]
    fn pause_puis_reprise_renvoie_le_segment() {
        let t0 = Instant::now();
        let mut run = ProfileRun::new(profile(vec![seg(100, 3.0, 0.0)]));
        assert!(run.step(t0, false));
        assert!(!run.step(t0, true));
        run.on_stopped();
        assert!(run.step(t0 + Duration::from_secs(30), false));
    }

    fn temp_path(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "kettler-profils-{name}-{}",
            SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()
        ));
        dir.join("profiles.json")
    }

    #[test]
    fn bibliotheque_sur_disque() {
        let path = temp_path("disque");
        let mut lib = ProfileLibrary::load(path.clone());
        assert_eq!(lib.list().len(), 3, "profils de départ seulement");

        let new = RunProfile {
            id: String::new(),
            ..profile(vec![seg(400, 5.0, 1.0)])
        };
        let saved = lib.save(&new).unwrap();
        assert!(!saved.id.is_empty());
        lib.set_selected(Some(saved.id.clone())).unwrap();

        let mut edited = saved.clone();
        edited.name = "Autre".into();
        lib.save(&edited).unwrap();

        let reloaded = ProfileLibrary::load(path.clone());
        assert_eq!(reloaded.selected(), Some(saved.id.as_str()));
        assert_eq!(reloaded.get(&saved.id).unwrap().name, "Autre");
        assert_eq!(reloaded.list().len(), 4);

        lib.delete(&saved.id).unwrap();
        assert_eq!(lib.selected(), None, "choix effacé avec le profil");
        assert_eq!(ProfileLibrary::load(path.clone()).list().len(), 3);
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn profils_de_depart_proteges() {
        let mut lib = ProfileLibrary::in_memory();
        let starter = lib.list().remove(0);
        assert!(lib.save(&starter).unwrap_err().contains("lecture seule"));
        assert!(lib.delete(&starter.id).is_err());
        assert!(lib.set_selected(Some(starter.id.clone())).is_ok());
        assert!(lib.set_selected(Some("inconnu".into())).is_err());
        assert!(lib
            .save(&RunProfile {
                id: "inconnu".into(),
                ..profile(vec![seg(100, 3.0, 0.0)])
            })
            .is_err());
    }

    #[test]
    fn fichier_illisible_mis_de_cote() {
        let path = temp_path("illisible");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, "{ pas du json").unwrap();
        let lib = ProfileLibrary::load(path.clone());
        assert_eq!(lib.list().len(), 3);
        assert!(path.with_extension("json.illisible").exists());
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }
}
