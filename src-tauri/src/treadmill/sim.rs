//! Tapis RUN 7 simulé, pour développer sans le tapis (lancement avec
//! `KETTLER_SIM=1`, voir `npm run app:sim`).
//!
//! Il remplace le Bluetooth SOUS la session : l'application lui envoie les
//! mêmes trames qu'au vrai tapis et il répond avec les mêmes octets. Toute la
//! session (handshake, polling, silence autour des écritures, reconnexion,
//! nouvel essai de démarrage) tourne donc exactement comme avec le RUN 7.
//!
//! Comportements reproduits (voir CLAUDE.md, « Comportements observés ») :
//! - handshake obligatoire : muet sans lui, liaison coupée au bout de ~10 s ;
//! - la console traite une requête à la fois (~7/s) ; au-delà de 12 requêtes en
//!   attente, les suivantes sont perdues ;
//! - une écriture collée à une autre requête (< 150 ms avant ou après) est perdue ;
//! - aucune réponse aux écritures (sauf au handshake) ;
//! - réponses renvoyées par paquets toutes les ~480 ms, découpées en
//!   notifications de 20 octets (une trame peut être coupée en deux) ;
//! - Online : 0 = en marche ; démarrage = compte à rebours ~5 s (écritures
//!   ignorées) puis 0,8 km/h ; arrêt = décélération, compteurs gardés ; un
//!   second arrêt les remet à zéro ; démarrage ignoré après une longue pause ;
//! - rampes : +0,2 km/h/s, −0,4 km/h/s à l'arrêt, ~0,33 %/s pour l'inclinaison ;
//! - SpeedGet oscille juste sous la consigne (29–30 pour 30) ;
//! - distance en mètres, remontée par à-coups toutes les 3 s ; énergie en 0,1 kcal ;
//! - Time (17) et TimeMode (18) ne répondent pas, comme les numéros inconnus ;
//! - perte de liaison : la bande CONTINUE ; retrait de la clé : arrêt net et
//!   module coupé jusqu'à la remise de la clé.
//!
//! Les incidents se provoquent depuis l'interface (`SimAction`).
//!
//! Le temps est celui de tokio : les tests peuvent le mettre en pause et
//! l'avancer à volonté.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use futures::Stream;
use tokio::sync::{mpsc, watch};
use tokio::time::{sleep, sleep_until, Instant};

use crate::dto::{SimAction, SimStatus};
use crate::kettler::protocol::{answer_frame, handshake_data, FrameDecoder, Message};

// ── Console (physique du tapis, pure) ─────────────────────────────────────

/// Durée du compte à rebours 3-2-1 après Online = 0.
const COUNTDOWN: Duration = Duration::from_secs(5);
const SPEED_MIN: u16 = 8;
const SPEED_MAX: u16 = 200;
const INCLINE_MAX: u16 = 150;
/// Rampes, en dixièmes par seconde.
const RAMP_SPEED: f64 = 2.0;
const RAMP_STOP: f64 = 4.0;
const RAMP_INCLINE: f64 = 3.3;
/// La distance lue ne bouge que par à-coups.
const DISTANCE_REPORT: Duration = Duration::from_secs(3);
/// Pause au-delà de laquelle Online = 0 est ignoré (kdri parle de 5 min).
pub const LONG_PAUSE: Duration = Duration::from_secs(5 * 60);
/// Pas d'intégration de la physique.
const STEP: Duration = Duration::from_millis(100);

/// Rapproche `from` de `to` d'au plus `step`.
fn approach(from: f64, to: f64, step: f64) -> f64 {
    if (to - from).abs() <= step {
        to
    } else {
        from + step.copysign(to - from)
    }
}

/// Le tapis lui-même : bande, moteur d'inclinaison, compteurs, clé.
/// Vitesses en dixièmes de km/h, inclinaisons en dixièmes de %.
#[derive(Debug, Clone)]
pub struct Console {
    running: bool,
    countdown: Option<Duration>,
    speed_set: u16,
    speed: f64,
    incline_set: u16,
    incline: f64,
    /// Mètres parcourus (exacts) et valeur exposée (par à-coups).
    distance: f64,
    distance_shown: u16,
    since_report: Duration,
    /// Dixièmes de kcal.
    energy: f64,
    /// Temps passé à l'arrêt avec des compteurs non nuls.
    paused_for: Duration,
    key_in: bool,
    hands_on: bool,
    rng: u32,
}

impl Default for Console {
    fn default() -> Self {
        Self::new()
    }
}

impl Console {
    pub fn new() -> Self {
        Console {
            running: false,
            countdown: None,
            speed_set: 0,
            speed: 0.0,
            incline_set: 0,
            incline: 0.0,
            distance: 0.0,
            distance_shown: 0,
            since_report: Duration::ZERO,
            energy: 0.0,
            paused_for: Duration::ZERO,
            key_in: true,
            hands_on: false,
            rng: 0x2545_f491,
        }
    }

    /// Petit générateur pseudo-aléatoire (xorshift), sans dépendance.
    fn random(&mut self) -> u32 {
        let mut x = self.rng;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.rng = x;
        x
    }

    fn has_counters(&self) -> bool {
        self.distance > 0.0 || self.energy > 0.0
    }

    fn long_pause(&self) -> bool {
        !self.running && self.has_counters() && self.paused_for >= LONG_PAUSE
    }

    /// Fait avancer la physique de `dt`.
    pub fn advance(&mut self, mut dt: Duration) {
        while !dt.is_zero() {
            let h = dt.min(STEP);
            dt -= h;
            self.step(h);
        }
    }

    fn step(&mut self, h: Duration) {
        let secs = h.as_secs_f64();
        if self.running {
            self.paused_for = Duration::ZERO;
            if let Some(left) = self.countdown {
                let left = left.saturating_sub(h);
                if left.is_zero() {
                    // Fin du compte à rebours : la bande part à la vitesse minimale.
                    self.countdown = None;
                    self.speed_set = SPEED_MIN;
                    self.speed = f64::from(SPEED_MIN);
                } else {
                    self.countdown = Some(left);
                }
            }
        } else if self.has_counters() {
            self.paused_for += h;
        }

        let desired = if self.running && self.countdown.is_none() {
            f64::from(self.speed_set)
        } else {
            0.0
        };
        let rate = if self.running { RAMP_SPEED } else { RAMP_STOP };
        self.speed = approach(self.speed, desired, rate * secs);
        self.incline = approach(self.incline, f64::from(self.incline_set), RAMP_INCLINE * secs);

        let kmh = self.speed / 10.0;
        self.distance += kmh / 3.6 * secs;
        if kmh > 0.0 {
            // ~10,5 kcal en 3 min 40 à 3 km/h, ordre de grandeur relevé sur le tapis.
            let kcal_per_min = 0.95 * kmh * (1.0 + self.incline / 100.0);
            self.energy += kcal_per_min * 10.0 / 60.0 * secs;
        }
        self.since_report += h;
        if self.since_report >= DISTANCE_REPORT {
            self.since_report = Duration::ZERO;
            self.distance_shown = self.distance.floor().min(f64::from(u16::MAX)) as u16;
        }
    }

    /// Online = 0 (application ou console).
    fn start(&mut self, from_console: bool) {
        if !self.key_in || self.running {
            return;
        }
        if !from_console && self.long_pause() {
            eprintln!("[sim] démarrage IGNORÉ : longue pause avec compteurs (comme le RUN 7)");
            return;
        }
        self.running = true;
        self.countdown = Some(COUNTDOWN);
        self.speed_set = 0;
        self.paused_for = Duration::ZERO;
    }

    /// Online = 1 : pause si la bande tourne, sinon remise à zéro des compteurs.
    fn stop(&mut self) {
        if self.running {
            self.running = false;
            self.countdown = None;
            self.speed_set = 0;
        } else {
            self.distance = 0.0;
            self.distance_shown = 0;
            self.since_report = Duration::ZERO;
            self.energy = 0.0;
        }
        self.paused_for = Duration::ZERO;
    }

    /// Les consignes ne sont acceptées que bande lancée, hors compte à rebours.
    fn accepts_setpoints(&self) -> bool {
        self.running && self.countdown.is_none()
    }

    fn set_speed(&mut self, raw: i32) {
        if self.accepts_setpoints() {
            self.speed_set = raw.clamp(i32::from(SPEED_MIN), i32::from(SPEED_MAX)) as u16;
        }
    }

    fn set_incline(&mut self, raw: i32) {
        if self.accepts_setpoints() {
            self.incline_set = raw.clamp(0, i32::from(INCLINE_MAX)) as u16;
        }
    }

    /// Écriture reçue de l'application. Rien n'est renvoyé (comme le RUN 7).
    pub fn write(&mut self, id: u16, data: &[u8]) {
        let value = match data {
            [b] => i32::from(*b),
            [hi, lo] => i32::from(u16::from_be_bytes([*hi, *lo])),
            _ => return,
        };
        match id {
            38 => match value {
                0 => self.start(false),
                1 => self.stop(),
                _ => {}
            },
            34 => self.set_speed(value),
            30 => self.set_incline(value),
            _ => {}
        }
    }

    /// Vitesse réelle telle que lue : oscille juste sous la consigne.
    fn speed_get(&mut self) -> u16 {
        let raw = self.speed.round().max(0.0) as u16;
        let settled = self.accepts_setpoints() && (self.speed - f64::from(self.speed_set)).abs() < 0.5;
        if settled && raw > SPEED_MIN && self.random() % 10 < 3 {
            raw - 1
        } else {
            raw
        }
    }

    fn pulse(&mut self) -> u16 {
        if !self.hands_on {
            return 0;
        }
        let noise = f64::from(self.random() % 4);
        (88.0 + self.speed / 10.0 * 6.0 + self.incline / 10.0 * 2.0 + noise).round() as u16
    }

    /// Données renvoyées pour une lecture ; `None` = le tapis ne répond pas.
    pub fn read(&mut self, id: u16) -> Option<Vec<u8>> {
        let num = |v: u16| Some(v.to_be_bytes().to_vec());
        let text = |t: &str| Some(t.as_bytes().to_vec());
        match id {
            1 => Some(Vec::new()),
            2 => num(314),
            6 => num(1),
            8 => {
                let p = self.pulse();
                num(p)
            }
            15 => num(self.distance_shown),
            16 => num(self.energy.round().min(f64::from(u16::MAX)) as u16),
            20 => text("RUN 7"),
            21 => Some(vec![0x06, 0xac, 0xfc, 0x01]),
            22 => text("07883-600"),
            23 => text("001"),
            24 => text("T"),
            25 => text("STB1T0158"),
            30 => num(self.incline_set),
            31 => num(self.incline.round() as u16),
            32 => num(0),
            33 => num(INCLINE_MAX),
            34 => num(self.speed_set),
            35 => {
                let s = self.speed_get();
                num(s)
            }
            36 => num(SPEED_MIN),
            37 => num(SPEED_MAX),
            38 => Some(vec![if self.running { 0 } else { 1 }]),
            // 17 (Time), 18 (TimeMode) et tout le reste : muets.
            _ => None,
        }
    }

    /// Retrait de la clé : arrêt net (pas de décélération).
    fn pull_key(&mut self) {
        self.key_in = false;
        self.running = false;
        self.countdown = None;
        self.speed_set = 0;
        self.speed = 0.0;
    }
}

// ── Liaison simulée ───────────────────────────────────────────────────────

/// Sans handshake dans ce délai, le tapis coupe la liaison.
const HANDSHAKE_DEADLINE: Duration = Duration::from_secs(10);
/// Temps de traitement d'une requête par la console (~7 par seconde).
const PROCESS_TIME: Duration = Duration::from_millis(140);
/// Une écriture à moins de ce délai d'une autre requête est perdue.
const WRITE_GAP: Duration = Duration::from_millis(150);
/// Requêtes en attente au plus ; au-delà, elles sont perdues.
const QUEUE_MAX: usize = 12;
/// Le module BLE ne renvoie les réponses que par paquets, à cette cadence.
const BATCH_PERIOD: Duration = Duration::from_millis(480);
/// Taille des notifications.
const CHUNK: usize = 20;
/// Durée minimale d'un scan qui trouve le tapis, et d'une connexion.
const SCAN_DELAY: Duration = Duration::from_millis(1200);
const CONNECT_DELAY: Duration = Duration::from_millis(600);

const ID_HANDSHAKE: u16 = 1;
const READ: u8 = 1;
const WRITE: u8 = 2;

/// Le handshake de kdri porte l'instruction 1 avec 18 octets de données : on ne
/// regarde que l'identifiant et les données.
fn is_handshake(msg: &Message) -> bool {
    msg.value_id == ID_HANDSHAKE && msg.data == handshake_data()
}

/// Rend une trame de réponse invalide (CRC faux) sans toucher à sa structure :
/// l'octet de poids fort de l'identifiant vaut toujours 0x00 pour nos valeurs.
fn corrupt(frame: &mut [u8]) {
    if frame.len() > 1 {
        frame[1] ^= 0x01;
    }
}

struct ActiveLink {
    gen: u64,
    cut: watch::Sender<bool>,
}

struct World {
    console: Console,
    /// Instant jusqu'où la physique a été calculée.
    last: Instant,
    /// Coupure radio : le tapis est invisible jusqu'à cet instant.
    unreachable_until: Option<Instant>,
    mute_until: Option<Instant>,
    corrupt_next: bool,
    fail_next_write: bool,
    link: Option<ActiveLink>,
    next_gen: u64,
}

impl World {
    fn advance(&mut self) -> Instant {
        let now = Instant::now();
        let dt = now.saturating_duration_since(self.last);
        self.last = now;
        self.console.advance(dt);
        now
    }

    fn reachable(&self, now: Instant) -> bool {
        self.console.key_in && self.unreachable_until.is_none_or(|t| now >= t)
    }

    fn muted(&self, now: Instant) -> bool {
        self.mute_until.is_some_and(|t| now < t)
    }

    /// Coupe la liaison en cours (sa tâche s'arrête, l'application voit la perte).
    fn cut_link(&mut self) {
        if let Some(link) = self.link.take() {
            let _ = link.cut.send(true);
        }
    }
}

/// Le tapis simulé, partagé entre la session et les commandes de l'interface.
#[derive(Clone)]
pub struct Simulator(Arc<Mutex<World>>);

impl Default for Simulator {
    fn default() -> Self {
        Self::new()
    }
}

impl Simulator {
    pub fn new() -> Self {
        Simulator(Arc::new(Mutex::new(World {
            console: Console::new(),
            last: Instant::now(),
            unreachable_until: None,
            mute_until: None,
            corrupt_next: false,
            fail_next_write: false,
            link: None,
            next_gen: 0,
        })))
    }

    fn world(&self) -> MutexGuard<'_, World> {
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Attend que le tapis soit visible, au plus `limit`. Même message
    /// d'erreur que le scan Bluetooth.
    pub async fn scan(&self, limit: Duration) -> Result<(), String> {
        let end = Instant::now() + limit;
        sleep(SCAN_DELAY.min(limit)).await;
        loop {
            let now = Instant::now();
            if self.world().reachable(now) {
                return Ok(());
            }
            if now >= end {
                return Err(format!(
                    "Tapis RUN 7 introuvable après {} s de recherche : vérifiez qu'il est allumé, \
                     à portée, et qu'aucune autre application n'y est connectée.",
                    limit.as_secs()
                ));
            }
            sleep(Duration::from_millis(200)).await;
        }
    }

    /// Ouvre une liaison : rend le port d'écriture et le flux des notifications.
    pub async fn connect(
        &self,
    ) -> Result<(SimPort, impl Stream<Item = Vec<u8>> + Send + 'static), String> {
        sleep(CONNECT_DELAY).await;
        let (to_sim, from_app) = mpsc::unbounded_channel::<Vec<u8>>();
        let (to_app, from_sim) = mpsc::unbounded_channel::<Vec<u8>>();
        let (cut_tx, cut_rx) = watch::channel(false);
        let gen = {
            let mut w = self.world();
            if !w.reachable(Instant::now()) {
                return Err("Connexion Bluetooth impossible : le tapis a disparu".into());
            }
            // Le tapis n'accepte qu'une liaison : la précédente tombe.
            w.cut_link();
            w.next_gen += 1;
            let gen = w.next_gen;
            w.link = Some(ActiveLink { gen, cut: cut_tx });
            gen
        };
        let task = LinkTask {
            sim: self.clone(),
            gen,
            from_app,
            to_app,
            cut: cut_rx,
        };
        tokio::spawn(task.run());
        let port = SimPort {
            sim: self.clone(),
            gen,
            to_sim,
        };
        let notifications = futures::stream::unfold(from_sim, |mut rx| async move {
            rx.recv().await.map(|bytes| (bytes, rx))
        });
        Ok((port, notifications))
    }

    fn cut_link_gen(&self, gen: u64) {
        let mut w = self.world();
        if w.link.as_ref().is_some_and(|l| l.gen == gen) {
            w.cut_link();
        }
    }

    /// État du tapis « vu de la salle ».
    pub fn status(&self) -> SimStatus {
        let mut w = self.world();
        let now = w.advance();
        let secs_until = |t: Option<Instant>| {
            t.filter(|t| *t > now)
                .map(|t| t.duration_since(now).as_secs_f64().ceil() as u32)
        };
        let link_back_in_s = secs_until(w.unreachable_until);
        let muted_for_s = secs_until(w.mute_until);
        let reachable = w.reachable(now);
        let link_active = w.link.is_some();
        let c = &w.console;
        SimStatus {
            key_in: c.key_in,
            reachable,
            link_active,
            link_back_in_s,
            muted_for_s,
            running: c.running,
            countdown_s: c.countdown.map(|d| d.as_secs_f64().ceil() as u32),
            speed_kmh: (c.speed.round() / 10.0) as f32,
            target_speed_kmh: f32::from(c.speed_set) / 10.0,
            incline_pct: (c.incline.round() / 10.0) as f32,
            target_incline_pct: f32::from(c.incline_set) / 10.0,
            distance_m: c.distance.floor() as u32,
            energy_kcal: (c.energy.round() / 10.0) as f32,
            hands_on: c.hands_on,
            long_pause: c.long_pause(),
        }
    }

    /// Provoque un événement sur le tapis simulé.
    pub fn act(&self, action: SimAction) -> Result<(), String> {
        let mut w = self.world();
        let now = w.advance();
        eprintln!("[sim] {action:?}");
        match action {
            SimAction::LinkLoss { seconds } => {
                w.unreachable_until = Some(now + Duration::from_secs(u64::from(seconds)));
                w.cut_link();
            }
            SimAction::PullKey => {
                w.console.pull_key();
                w.cut_link();
            }
            SimAction::InsertKey => w.console.key_in = true,
            SimAction::ConsoleStart => w.console.start(true),
            SimAction::ConsoleStop => w.console.stop(),
            SimAction::ConsoleSpeed { delta } => {
                let raw = i32::from(w.console.speed_set) + (delta * 10.0).round() as i32;
                w.console.set_speed(raw);
            }
            SimAction::ConsoleIncline { delta } => {
                let raw = i32::from(w.console.incline_set) + (delta * 10.0).round() as i32;
                w.console.set_incline(raw);
            }
            SimAction::HandsOn { on } => w.console.hands_on = on,
            SimAction::AgePause => {
                if w.console.running || !w.console.has_counters() {
                    return Err(
                        "Pas de pause en cours : il faut des compteurs et la bande arrêtée".into(),
                    );
                }
                w.console.paused_for = LONG_PAUSE;
            }
            SimAction::Mute { seconds } => {
                w.mute_until = Some(now + Duration::from_secs(u64::from(seconds)));
            }
            SimAction::CorruptNextAnswer => w.corrupt_next = true,
            SimAction::FailNextWrite => w.fail_next_write = true,
        }
        Ok(())
    }
}

/// Côté application d'une liaison simulée (équivalent de la caractéristique FFF2).
#[derive(Clone)]
pub struct SimPort {
    sim: Simulator,
    gen: u64,
    to_sim: mpsc::UnboundedSender<Vec<u8>>,
}

impl SimPort {
    /// Envoie une trame en morceaux de 20 octets, comme sur le vrai module.
    pub async fn write(&self, frame: &[u8]) -> Result<(), String> {
        let is_command = FrameDecoder::new()
            .push(frame)
            .into_iter()
            .flatten()
            .any(|m| m.instruction == WRITE && m.value_id != ID_HANDSHAKE);
        if is_command {
            let mut w = self.sim.world();
            if w.fail_next_write {
                w.fail_next_write = false;
                return Err("échec simulé".into());
            }
        }
        for chunk in frame.chunks(CHUNK) {
            self.to_sim
                .send(chunk.to_vec())
                .map_err(|_| "liaison coupée".to_string())?;
        }
        Ok(())
    }

    /// Déconnexion demandée par l'application.
    pub fn disconnect(&self) {
        self.sim.cut_link_gen(self.gen);
    }
}

struct Pending {
    msg: Message,
    at: Instant,
}

impl Pending {
    /// Une écriture n'est traitée qu'après un silence (sinon elle peut encore être perdue).
    fn ready_at(&self) -> Instant {
        if self.msg.instruction == WRITE {
            self.at + WRITE_GAP
        } else {
            self.at
        }
    }
}

/// Tâche d'une liaison : file de la console et envoi des réponses par paquets.
struct LinkTask {
    sim: Simulator,
    gen: u64,
    from_app: mpsc::UnboundedReceiver<Vec<u8>>,
    to_app: mpsc::UnboundedSender<Vec<u8>>,
    cut: watch::Receiver<bool>,
}

#[derive(Default)]
struct LinkState {
    handshaken: bool,
    queue: VecDeque<Pending>,
    last_rx: Option<Instant>,
    outbox: Vec<u8>,
}

impl LinkTask {
    async fn run(mut self) {
        let start = Instant::now();
        let deadline = start + HANDSHAKE_DEADLINE;
        let mut decoder = FrameDecoder::new();
        let mut st = LinkState::default();
        let mut busy_until = start;
        let mut next_flush = start + BATCH_PERIOD;

        loop {
            let mut wake = next_flush;
            if !st.handshaken {
                wake = wake.min(deadline);
            }
            if let Some(p) = st.queue.front() {
                wake = wake.min(busy_until.max(p.ready_at()));
            }
            tokio::select! {
                biased;
                _ = self.cut.changed() => break,
                bytes = self.from_app.recv() => {
                    let Some(bytes) = bytes else { break };
                    let now = Instant::now();
                    // Les trames au CRC invalide sont ignorées, comme sur la console.
                    for msg in decoder.push(&bytes).into_iter().flatten() {
                        self.receive(&mut st, msg, now);
                    }
                }
                _ = sleep_until(wake) => {
                    let now = Instant::now();
                    if !st.handshaken && now >= deadline {
                        eprintln!("[sim] pas de handshake en 10 s : liaison coupée");
                        break;
                    }
                    if let Some(p) = st.queue.front() {
                        if now >= busy_until.max(p.ready_at()) {
                            let p = st.queue.pop_front().expect("file non vide");
                            self.process(&mut st, p.msg);
                            busy_until = now + PROCESS_TIME;
                        }
                    }
                    if now >= next_flush {
                        while next_flush <= now {
                            next_flush += BATCH_PERIOD;
                        }
                        if !st.outbox.is_empty() {
                            for chunk in std::mem::take(&mut st.outbox).chunks(CHUNK) {
                                if self.to_app.send(chunk.to_vec()).is_err() {
                                    break;
                                }
                            }
                        }
                    }
                }
            }
        }
        self.sim.cut_link_gen(self.gen);
    }

    /// Requête reçue : mise en file (ou perdue, selon les règles de la console).
    fn receive(&self, st: &mut LinkState, msg: Message, now: Instant) {
        if self.sim.world().muted(now) {
            return;
        }
        if !st.handshaken {
            // Avant le handshake, le tapis ignore tout le reste.
            if is_handshake(&msg) {
                st.handshaken = true;
                st.outbox
                    .extend(answer_frame(ID_HANDSHAKE, handshake_data()));
            }
            st.last_rx = Some(now);
            return;
        }
        let too_close = st
            .last_rx
            .is_some_and(|t| now.saturating_duration_since(t) < WRITE_GAP);
        st.last_rx = Some(now);
        if msg.instruction == WRITE && too_close {
            eprintln!("[sim] écriture n° {} PERDUE : collée à la requête précédente", msg.value_id);
            return;
        }
        // Une écriture qui attend encore son silence est perdue si une requête la suit.
        if st
            .queue
            .back()
            .is_some_and(|p| p.msg.instruction == WRITE && now < p.ready_at())
        {
            let lost = st.queue.pop_back().expect("file non vide");
            eprintln!(
                "[sim] écriture n° {} PERDUE : suivie de trop près par une autre requête",
                lost.msg.value_id
            );
        }
        if st.queue.len() >= QUEUE_MAX {
            eprintln!("[sim] file de la console pleine : requête n° {} perdue", msg.value_id);
            return;
        }
        st.queue.push_back(Pending { msg, at: now });
    }

    /// Traitement d'une requête par la console.
    fn process(&self, st: &mut LinkState, msg: Message) {
        let mut w = self.sim.world();
        w.advance();
        match msg.instruction {
            _ if is_handshake(&msg) => {
                st.outbox.extend(answer_frame(ID_HANDSHAKE, handshake_data()));
            }
            READ => {
                if let Some(data) = w.console.read(msg.value_id) {
                    let mut frame = answer_frame(msg.value_id, &data);
                    if w.corrupt_next {
                        w.corrupt_next = false;
                        corrupt(&mut frame);
                        eprintln!("[sim] réponse n° {} envoyée avec un CRC invalide", msg.value_id);
                    }
                    st.outbox.extend(frame);
                }
            }
            WRITE => {
                eprintln!("[sim] écriture n° {} = {:02x?}", msg.value_id, msg.data);
                w.console.write(msg.value_id, &msg.data);
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kettler::protocol::{
        handshake_frame, read_frame, write_u16_frame, write_u8_frame, ValueId,
    };
    use futures::StreamExt;

    fn secs(s: f64) -> Duration {
        Duration::from_secs_f64(s)
    }

    fn u16_of(c: &mut Console, id: u16) -> u16 {
        let d = c.read(id).unwrap();
        u16::from_be_bytes([d[0], d[1]])
    }

    fn online(c: &mut Console) -> u8 {
        c.read(38).unwrap()[0]
    }

    /// Console lancée, compte à rebours terminé, consigne à `raw`, vitesse atteinte.
    fn running_at(raw: u16) -> Console {
        let mut c = Console::new();
        c.write(38, &[0]);
        c.advance(COUNTDOWN);
        c.write(34, &raw.to_be_bytes());
        c.advance(secs(60.0));
        c
    }

    #[test]
    fn demarrage_compte_a_rebours_puis_vitesse_minimale() {
        let mut c = Console::new();
        assert_eq!(online(&mut c), 1);
        c.write(38, &[0]);
        assert_eq!(online(&mut c), 0);
        // Pendant le compte à rebours : bande immobile, consignes ignorées.
        c.write(34, &30u16.to_be_bytes());
        c.advance(secs(4.0));
        assert_eq!(u16_of(&mut c, 34), 0);
        assert_eq!(u16_of(&mut c, 35), 0);
        c.advance(secs(1.0));
        assert_eq!(u16_of(&mut c, 34), 8);
        assert_eq!(u16_of(&mut c, 35), 8);
    }

    #[test]
    fn rampe_de_vitesse_et_oscillation_sous_la_consigne() {
        let mut c = running_at(8);
        c.write(34, &30u16.to_be_bytes());
        c.advance(secs(5.0));
        let mid = u16_of(&mut c, 35);
        assert!((17..=19).contains(&mid), "0,2 km/h/s : {mid}");
        c.advance(secs(10.0));
        for _ in 0..20 {
            let v = u16_of(&mut c, 35);
            assert!(v == 29 || v == 30, "{v}");
        }
    }

    #[test]
    fn consignes_bornees() {
        let mut c = running_at(30);
        c.write(34, &500u16.to_be_bytes());
        assert_eq!(u16_of(&mut c, 34), SPEED_MAX);
        c.write(34, &2u16.to_be_bytes());
        assert_eq!(u16_of(&mut c, 34), SPEED_MIN);
        c.write(30, &400u16.to_be_bytes());
        assert_eq!(u16_of(&mut c, 30), INCLINE_MAX);
    }

    #[test]
    fn arret_puis_second_arret_remet_les_compteurs_a_zero() {
        let mut c = running_at(30);
        c.write(38, &[1]);
        assert_eq!(online(&mut c), 1);
        assert_eq!(u16_of(&mut c, 34), 0);
        c.advance(secs(10.0));
        assert_eq!(u16_of(&mut c, 35), 0);
        let d = u16_of(&mut c, 15);
        assert!(d > 0);
        assert!(u16_of(&mut c, 16) > 0);
        c.write(38, &[1]);
        assert_eq!(u16_of(&mut c, 15), 0);
        assert_eq!(u16_of(&mut c, 16), 0);
    }

    #[test]
    fn demarrage_ignore_apres_une_longue_pause() {
        let mut c = running_at(30);
        c.write(38, &[1]);
        c.advance(LONG_PAUSE + secs(10.0));
        c.write(38, &[0]);
        assert_eq!(online(&mut c), 1, "ignoré");
        // Après une remise à zéro, il repart.
        c.write(38, &[1]);
        c.write(38, &[0]);
        assert_eq!(online(&mut c), 0);
    }

    #[test]
    fn distance_par_a_coups_et_energie_en_dixiemes_de_kcal() {
        let mut c = running_at(30);
        c.write(38, &[1]);
        c.write(38, &[1]);
        c.write(38, &[0]);
        c.advance(COUNTDOWN);
        c.write(34, &30u16.to_be_bytes());
        c.advance(secs(12.0)); // rampe 0,8 → 3,0 km/h
        // Juste après un à-coup, la valeur reste figée pendant ~3 s.
        let before = u16_of(&mut c, 15);
        while u16_of(&mut c, 15) == before {
            c.advance(secs(0.1));
        }
        let jumped = u16_of(&mut c, 15);
        c.advance(secs(2.5));
        assert_eq!(u16_of(&mut c, 15), jumped, "figée entre deux à-coups");
        c.advance(secs(220.0));
        let d = u16_of(&mut c, 15);
        assert!((180..=200).contains(&d), "~3 min 40 à 3 km/h : {d} m");
        let e = u16_of(&mut c, 16);
        assert!((95..=115).contains(&e), "~10,5 kcal : {e}");
    }

    #[test]
    fn pouls_seulement_mains_sur_les_capteurs() {
        let mut c = running_at(30);
        assert_eq!(u16_of(&mut c, 8), 0);
        c.hands_on = true;
        let p = u16_of(&mut c, 8);
        assert!((100..=115).contains(&p), "{p}");
    }

    #[test]
    fn valeurs_muettes_et_statiques() {
        let mut c = Console::new();
        assert_eq!(c.read(17), None);
        assert_eq!(c.read(18), None);
        assert_eq!(c.read(99), None);
        assert_eq!(c.read(20).unwrap(), b"RUN 7");
        assert_eq!(c.read(25).unwrap(), b"STB1T0158");
        assert_eq!(u16_of(&mut c, 37), 200);
    }

    #[test]
    fn cle_retiree_arret_net_et_demarrage_impossible() {
        let mut c = running_at(30);
        c.pull_key();
        assert_eq!(online(&mut c), 1);
        assert_eq!(u16_of(&mut c, 35), 0);
        c.write(38, &[0]);
        assert_eq!(online(&mut c), 1);
    }

    // ── Liaison (temps tokio en pause) ──────────────────────────────────────

    type Rx = std::pin::Pin<Box<dyn Stream<Item = Vec<u8>> + Send>>;

    async fn open(sim: &Simulator) -> (SimPort, Rx) {
        sim.scan(Duration::from_secs(5)).await.unwrap();
        let (port, rx) = sim.connect().await.unwrap();
        (port, Box::pin(rx))
    }

    /// Messages reçus pendant `wait`.
    async fn collect(rx: &mut Rx, wait: Duration) -> Vec<Message> {
        let mut decoder = FrameDecoder::new();
        let mut out = Vec::new();
        let end = Instant::now() + wait;
        while let Ok(Some(bytes)) = tokio::time::timeout_at(end, rx.next()).await {
            assert!(bytes.len() <= CHUNK);
            out.extend(decoder.push(&bytes).into_iter().flatten());
        }
        out
    }

    #[tokio::test(start_paused = true)]
    async fn muet_sans_handshake_puis_coupe() {
        let sim = Simulator::new();
        let (port, mut rx) = open(&sim).await;
        port.write(&read_frame(ValueId::SpeedGet)).await.unwrap();
        assert!(collect(&mut rx, secs(2.0)).await.is_empty());
        // Coupure au bout de ~10 s : le flux se termine.
        let end = tokio::time::timeout(secs(10.0), rx.next()).await;
        assert!(matches!(end, Ok(None)));
        assert!(!sim.status().link_active);
    }

    #[tokio::test(start_paused = true)]
    async fn handshake_puis_reponses_par_paquets() {
        let sim = Simulator::new();
        let (port, mut rx) = open(&sim).await;
        port.write(&handshake_frame()).await.unwrap();
        let hs = collect(&mut rx, secs(0.6)).await;
        assert_eq!(hs.len(), 1);
        assert_eq!(hs[0].value_id, 1);
        assert_eq!(hs[0].instruction, 3);
        assert_eq!(hs[0].data, handshake_data());

        for id in [ValueId::DeviceName, ValueId::DeviceId, ValueId::Online] {
            port.write(&read_frame(id)).await.unwrap();
        }
        // Les notifications n'arrivent que par paquets, espacés d'au moins ~480 ms.
        let mut decoder = FrameDecoder::new();
        let mut got = Vec::new();
        let mut arrivals: Vec<Instant> = Vec::new();
        let end = Instant::now() + secs(1.5);
        while let Ok(Some(b)) = tokio::time::timeout_at(end, rx.next()).await {
            arrivals.push(Instant::now());
            got.extend(decoder.push(&b).into_iter().flatten());
        }
        arrivals.dedup();
        for pair in arrivals.windows(2) {
            assert!(pair[1] - pair[0] >= Duration::from_millis(470), "{arrivals:?}");
        }
        let ids: Vec<u16> = got.iter().map(|m| m.value_id).collect();
        assert_eq!(ids, vec![20, 25, 38]);
        assert_eq!(got[0].as_text().as_deref(), Some("RUN 7"));
    }

    #[tokio::test(start_paused = true)]
    async fn ecriture_collee_a_une_lecture_perdue() {
        let sim = Simulator::new();
        let (port, mut rx) = open(&sim).await;
        port.write(&handshake_frame()).await.unwrap();
        collect(&mut rx, secs(1.0)).await;

        port.write(&read_frame(ValueId::Online)).await.unwrap();
        port.write(&write_u8_frame(ValueId::Online, 0)).await.unwrap();
        collect(&mut rx, secs(1.0)).await;
        assert!(!sim.status().running, "écriture collée : perdue");

        // Avec du silence avant et après, elle passe.
        port.write(&write_u8_frame(ValueId::Online, 0)).await.unwrap();
        collect(&mut rx, secs(1.0)).await;
        assert!(sim.status().running);
    }

    #[tokio::test(start_paused = true)]
    async fn crc_corrompu_et_echec_d_ecriture() {
        let sim = Simulator::new();
        let (port, mut rx) = open(&sim).await;
        port.write(&handshake_frame()).await.unwrap();
        collect(&mut rx, secs(1.0)).await;

        sim.act(SimAction::CorruptNextAnswer).unwrap();
        port.write(&read_frame(ValueId::Online)).await.unwrap();
        assert!(collect(&mut rx, secs(1.0)).await.is_empty(), "CRC invalide : ignorée");
        port.write(&read_frame(ValueId::Online)).await.unwrap();
        assert_eq!(collect(&mut rx, secs(1.0)).await.len(), 1);

        sim.act(SimAction::FailNextWrite).unwrap();
        // Les lectures ne sont pas concernées.
        port.write(&read_frame(ValueId::Online)).await.unwrap();
        assert!(port.write(&write_u16_frame(ValueId::SpeedSet, 30)).await.is_err());
        assert!(port.write(&write_u16_frame(ValueId::SpeedSet, 30)).await.is_ok());
    }

    #[tokio::test(start_paused = true)]
    async fn coupure_radio_bande_continue_tapis_invisible() {
        let sim = Simulator::new();
        sim.act(SimAction::ConsoleStart).unwrap();
        let (_port, mut rx) = open(&sim).await;
        sim.act(SimAction::LinkLoss { seconds: 6 }).unwrap();
        assert!(matches!(rx.next().await, None));
        assert!(sim.scan(Duration::from_secs(3)).await.is_err());
        assert!(sim.status().running, "la bande continue");
        assert!(sim.scan(Duration::from_secs(5)).await.is_ok());
    }
}
