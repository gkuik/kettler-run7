//! Session Bluetooth (btleplug) avec le tapis : scan, connexion, handshake,
//! polling, écritures, reconnexion automatique.
//!
//! Conception :
//! - Une **tâche de session** par connexion voulue (`connect` → `disconnect`).
//!   Elle établit la liaison, fait tourner la boucle de polling, et en cas de
//!   perte NON voulue passe en `Reconnecting` et réessaie toutes les 2 s. Elle
//!   possède le flux de notifications et le décodeur.
//! - La liaison active (`Link`) est partagée derrière un `tokio::sync::Mutex` qui
//!   sert aussi de **verrou d'écriture** : une trame complète (tous ses morceaux
//!   de 20 octets) part sous ce verrou, donc deux trames ne s'entrelacent jamais.
//!   La boucle de polling le relâche entre deux lectures : les écritures des
//!   commandes passent entre deux lectures.
//! - **Priorité du stop** : `stop` incrémente `stop_gen` AVANT de prendre le
//!   verrou. Toute écriture de commande lancée avant ce stop compare `stop_gen`
//!   une fois le verrou obtenu et abandonne si un stop est arrivé entre-temps.
//!   Le stop part donc au plus tard après la seule trame en cours d'envoi.
//! - Le modèle (`state::Model`) est sous un `std::sync::Mutex`, jamais tenu à
//!   travers un `.await`. Toute la logique de sécurité des valeurs y vit.
//! - Le transport est soit le Bluetooth (btleplug), soit le tapis simulé
//!   (`sim.rs`, lancement avec `KETTLER_SIM=1`) : tout le reste est commun.
//! - La reconnexion n'écrit que le handshake et des lectures. Seule exception,
//!   à la demande explicite de l'utilisateur : un STOP pressé pendant la coupure
//!   est mémorisé et envoyé à la reconnexion si la bande tourne encore.

use std::collections::VecDeque;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use btleplug::api::{
    Central, CentralEvent, CentralState, Characteristic, Manager as _, Peripheral as _, ScanFilter,
    WriteType,
};
use btleplug::platform::{Adapter, Manager, Peripheral};
use futures::{Stream, StreamExt};
use tokio::sync::{oneshot, watch, OnceCell};
use tokio::task::JoinHandle;
use tokio::time::{sleep, sleep_until, timeout, Instant};
use uuid::Uuid;

use super::sim::{SimPort, Simulator};
use super::state::{
    decode, is_handshake_answer, is_treadmill_name, Model, FAST_IDS, SLOW_EVERY, SLOW_IDS,
};
use crate::dto::{ConnectionState, TreadmillSnapshot};
use crate::kettler::protocol::{
    handshake_frame, read_frame, write_u16_frame, write_u8_frame, FrameDecoder, ValueId,
};

/// Caractéristique d'écriture (trames vers le tapis), write-without-response.
const TX_UUID: Uuid = Uuid::from_u128(0x0000fff2_0000_1000_8000_00805f9b34fb);
/// Caractéristique de notification (réponses du tapis).
const RX_UUID: Uuid = Uuid::from_u128(0x0000fff1_0000_1000_8000_00805f9b34fb);
/// Taille des morceaux d'écriture BLE (prudence MTU, vérifiée sur le tapis).
const CHUNK: usize = 20;

const SCAN_TIMEOUT: Duration = Duration::from_secs(20);
const RESCAN_TIMEOUT: Duration = Duration::from_secs(5);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const DISCOVER_TIMEOUT: Duration = Duration::from_secs(10);
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(4);
const WRITE_TIMEOUT: Duration = Duration::from_secs(2);
/// Silence imposé avant et après une ÉCRITURE (commande). La console traite
/// une requête à la fois : une écriture collée à une lecture est perdue sans
/// réponse (constaté sur le RUN 7 : Online = 0 ignoré). Les écritures vérifiées
/// par les scripts Python partaient toujours après ≥ 0,6 s de silence.
const WRITE_QUIET: Duration = Duration::from_millis(250);
/// Délai pour voir la bande partir après Online = 0 (compte à rebours ~5 s inclus).
const START_CONFIRM: Duration = Duration::from_secs(7);
/// Pause entre la remise à zéro (Online = 1) et le nouvel essai de démarrage.
const START_RESET_PAUSE: Duration = Duration::from_secs(1);
const DISCONNECT_TIMEOUT: Duration = Duration::from_secs(3);
const JOIN_TIMEOUT: Duration = Duration::from_secs(5);
/// Écart minimal entre deux lectures envoyées.
const READ_SPACING: Duration = Duration::from_millis(60);
/// Lectures en attente de réponse au plus. Constaté sur le RUN 7 :
/// - sans limite (≈ 16 lectures/s), la file de la console se remplit et les
///   commandes mettent 2 à 5 s à passer, voire sont perdues ;
/// - avec une seule, l'écran ne se rafraîchit que toutes les ~2 s, car le module
///   BLE ne renvoie les réponses que par paquets, toutes les ~480 ms.
const MAX_INFLIGHT: u32 = 3;
/// Sans réponse dans ce délai, les lectures en attente sont considérées perdues.
const ANSWER_TIMEOUT: Duration = Duration::from_millis(700);
/// Période minimale d'un cycle de polling complet.
const POLL_PERIOD: Duration = Duration::from_millis(500);
/// Sans aucune réponse valide pendant ce délai, la liaison est considérée perdue.
const WATCHDOG: Duration = Duration::from_secs(5);
/// Délai entre deux tentatives de reconnexion.
const RETRY_DELAY: Duration = Duration::from_secs(2);
/// Nombre de cycles pendant lesquels on insiste pour la référence d'article
/// (facultative), et pendant lesquels on relit à chaque cycle les valeurs
/// statiques manquantes (ensuite, un cycle sur dix).
const ARTICLE_CYCLES: u32 = 5;
const STATIC_CYCLES: u32 = 10;

const MSG_LINK_LOST: &str = "Liaison Bluetooth perdue";
const MSG_CANCELLED_BY_STOP: &str = "Commande annulée : un arrêt a été demandé";
const MSG_STOP_DEFERRED: &str = "Arrêt NON envoyé : le tapis n'est pas connecté. Il sera envoyé \
     dès la reconnexion si la bande tourne encore. Utilisez le bouton stop de la console ou la \
     clé de sécurité.";
const MSG_STOP_DISCONNECTED: &str = "Arrêt impossible : le tapis n'est pas connecté. Utilisez le \
     bouton stop de la console ou la clé de sécurité.";

/// Octets reçus du tapis (notifications FFF1).
type ByteStream = Pin<Box<dyn Stream<Item = Vec<u8>> + Send>>;
/// Un élément par perte de liaison signalée par le système.
type LostStream = Pin<Box<dyn Stream<Item = ()> + Send>>;

/// Reçoit chaque nouvel état du modèle (en production : événement Tauri).
pub type StateEmitter = Box<dyn Fn(TreadmillSnapshot) + Send + Sync>;

/// D'où viennent les octets : le vrai tapis ou le simulateur.
enum Backend {
    /// Gestionnaire et adaptateur Bluetooth, créés au premier `connect`.
    Ble(OnceCell<(Manager, Adapter)>),
    Sim(Simulator),
}

/// Côté écriture d'une liaison établie.
#[derive(Clone)]
enum Port {
    Ble {
        peripheral: Peripheral,
        tx: Characteristic,
    },
    Sim(SimPort),
}

impl Port {
    async fn write(&self, frame: &[u8]) -> Result<(), String> {
        match self {
            Port::Ble { peripheral, tx } => write_chunks(peripheral, tx, frame).await,
            Port::Sim(port) => port.write(frame).await,
        }
    }

    async fn disconnect(&self) {
        match self {
            Port::Ble { peripheral, .. } => {
                let _ = timeout(DISCONNECT_TIMEOUT, peripheral.disconnect()).await;
            }
            Port::Sim(port) => port.disconnect(),
        }
    }
}

/// Ce qui a été ouvert pendant l'établissement, à refermer en cas d'échec.
enum Opened {
    Ble(Peripheral),
    Sim(SimPort),
}

/// Liaison active, utilisable pour écrire.
struct Link {
    /// Génération de la session propriétaire (une ancienne session ne doit pas
    /// effacer la liaison d'une nouvelle).
    gen: u64,
    port: Port,
}

/// Session en cours (connexion, connectée ou en reconnexion).
struct SessionHandle {
    gen: u64,
    cancel: watch::Sender<bool>,
    task: JoinHandle<()>,
}

/// Liaison établie (handshake accepté), pas encore publiée.
struct Conn {
    port: Port,
    rx: ByteStream,
    lost: LostStream,
    decoder: FrameDecoder,
}

enum Failure {
    Cancelled,
    Error(String),
}

enum Exit {
    Cancelled,
    Lost(String),
}

enum WriteError {
    NoLink,
    CancelledByStop,
    Ble(String),
}

impl WriteError {
    fn message(&self) -> String {
        match self {
            WriteError::NoLink => super::state::ERR_NOT_CONNECTED.into(),
            WriteError::CancelledByStop => MSG_CANCELLED_BY_STOP.into(),
            WriteError::Ble(e) => format!("Écriture Bluetooth impossible : {e}"),
        }
    }
}

struct Shared {
    emit: StateEmitter,
    model: Mutex<Model>,
    link: tokio::sync::Mutex<Option<Link>>,
    session: Mutex<Option<SessionHandle>>,
    next_gen: AtomicU64,
    /// Incrémenté à chaque stop : invalide les écritures en attente.
    stop_gen: AtomicU64,
    /// Stop demandé pendant que la liaison était absente.
    pending_stop: AtomicBool,
    /// Diagnostic : journaliser les trames reçues jusqu'à cet instant (après une écriture).
    log_rx_until: Mutex<Option<std::time::Instant>>,
    /// Instant du dernier envoi (lecture ou écriture), pour espacer les écritures.
    last_tx: Mutex<Option<Instant>>,
    /// Une séquence de démarrage est en cours (refuse un second `start`).
    starting: AtomicBool,
    /// Valeur à relire en priorité après une écriture, pour que l'écran
    /// confirme la commande sans attendre son tour dans le cycle de polling.
    priority_read: Mutex<Option<ValueId>>,
    backend: Backend,
}

/// Point d'accès unique au tapis (état géré par Tauri).
#[derive(Clone)]
pub struct Treadmill(Arc<Shared>);

/// Attend l'annulation de la session (ou la disparition de son émetteur).
async fn cancelled(rx: &mut watch::Receiver<bool>) {
    let _ = rx.wait_for(|v| *v).await;
}

/// Horodatage du journal de diagnostic : secondes depuis le premier appel.
fn t() -> String {
    static START: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
    format!("{:8.3}", START.get_or_init(std::time::Instant::now).elapsed().as_secs_f64())
}

/// Octets en hexadécimal, pour le journal de diagnostic.
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect::<Vec<_>>().join(" ")
}

/// Valeur écrite par une trame d'écriture (pour la relire aussitôt).
fn written_value(frame: &[u8]) -> Option<ValueId> {
    FrameDecoder::new()
        .push(frame)
        .into_iter()
        .flatten()
        .find(|m| m.instruction == 2)
        .and_then(|m| ValueId::from_u16(m.value_id))
}

/// Vrai si la trame est une lecture (instruction 1) : on la décode pour le savoir.
fn is_read_frame(frame: &[u8]) -> bool {
    FrameDecoder::new()
        .push(frame)
        .into_iter()
        .flatten()
        .any(|m| m.instruction == 1)
}

/// Écrit une trame en morceaux de 20 octets, sans réponse.
async fn write_chunks(p: &Peripheral, tx: &Characteristic, frame: &[u8]) -> Result<(), String> {
    for chunk in frame.chunks(CHUNK) {
        match timeout(
            WRITE_TIMEOUT,
            p.write(tx, chunk, WriteType::WithoutResponse),
        )
        .await
        {
            Ok(Ok(())) => {}
            Ok(Err(e)) => return Err(e.to_string()),
            Err(_) => return Err("délai dépassé".into()),
        }
    }
    Ok(())
}

/// Cherche le tapis par son nom annoncé (« RUN 7 », espaces et casse ignorés).
/// Seuls les appareils vus pendant ce scan comptent : un appareil connu mais
/// hors de portée ne fait pas perdre de temps.
async fn find_treadmill(adapter: &Adapter, limit: Duration) -> Result<Peripheral, String> {
    let mut events = adapter
        .events()
        .await
        .map_err(|e| format!("Bluetooth indisponible : {e}"))?;
    adapter
        .start_scan(ScanFilter::default())
        .await
        .map_err(|e| format!("Impossible de lancer la recherche Bluetooth : {e}"))?;
    let search = async {
        while let Some(ev) = events.next().await {
            let id = match ev {
                CentralEvent::DeviceDiscovered(id) | CentralEvent::DeviceUpdated(id) => id,
                _ => continue,
            };
            let Ok(p) = adapter.peripheral(&id).await else {
                continue;
            };
            if let Ok(Some(props)) = p.properties().await {
                if props.local_name.as_deref().is_some_and(is_treadmill_name) {
                    return Some(p);
                }
            }
        }
        None
    };
    let found = timeout(limit, search).await;
    let _ = adapter.stop_scan().await;
    match found {
        Ok(Some(p)) => Ok(p),
        _ => Err(format!(
            "Tapis RUN 7 introuvable après {} s de recherche : vérifiez qu'il est allumé, \
             à portée, et qu'aucune autre application n'y est connectée.",
            limit.as_secs()
        )),
    }
}

impl Treadmill {
    /// `sim` : tapis simulé à la place du Bluetooth (rien n'est alors ouvert en BLE).
    pub fn new(emit: StateEmitter, sim: Option<Simulator>) -> Self {
        let backend = match sim {
            Some(sim) => Backend::Sim(sim),
            None => Backend::Ble(OnceCell::new()),
        };
        Treadmill(Arc::new(Shared {
            emit,
            model: Mutex::new(Model::new()),
            link: tokio::sync::Mutex::new(None),
            session: Mutex::new(None),
            next_gen: AtomicU64::new(0),
            stop_gen: AtomicU64::new(0),
            pending_stop: AtomicBool::new(false),
            log_rx_until: Mutex::new(None),
            last_tx: Mutex::new(None),
            starting: AtomicBool::new(false),
            priority_read: Mutex::new(None),
            backend,
        }))
    }

    /// Le tapis simulé, si l'application a été lancée en mode simulateur.
    pub fn simulator(&self) -> Option<Simulator> {
        match &self.0.backend {
            Backend::Sim(sim) => Some(sim.clone()),
            Backend::Ble(_) => None,
        }
    }

    // ── Outils internes ─────────────────────────────────────────────────────

    /// Verrou du modèle ; un mutex empoisonné est récupéré plutôt que de paniquer.
    fn model(&self) -> MutexGuard<'_, Model> {
        self.0.model.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn session_slot(&self) -> MutexGuard<'_, Option<SessionHandle>> {
        self.0.session.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Émet l'état s'il a changé depuis la dernière émission.
    fn publish(&self) {
        let changed = self.model().take_changed();
        if let Some(snap) = changed {
            (self.0.emit)(snap);
        }
    }

    /// Modifie le modèle puis publie.
    fn update<R>(&self, f: impl FnOnce(&mut Model) -> R) -> R {
        let r = f(&mut self.model());
        self.publish();
        r
    }

    async fn adapter(&self) -> Result<Adapter, String> {
        let Backend::Ble(ble) = &self.0.backend else {
            return Err("Bluetooth non utilisé en mode simulateur".into());
        };
        let pair = ble
            .get_or_try_init(|| async {
                let manager = Manager::new()
                    .await
                    .map_err(|e| format!("Bluetooth indisponible : {e}"))?;
                let adapter = manager
                    .adapters()
                    .await
                    .map_err(|e| format!("Bluetooth indisponible : {e}"))?
                    .into_iter()
                    .next()
                    .ok_or_else(|| "Aucun adaptateur Bluetooth trouvé".to_string())?;
                Ok::<_, String>((manager, adapter))
            })
            .await?;
        Ok(pair.1.clone())
    }

    /// Écrit une trame sur la liaison active, sous le verrou d'écriture.
    /// `guard` : valeur de `stop_gen` au moment où la commande a été lancée ;
    /// si un stop est arrivé depuis, l'écriture est abandonnée.
    async fn write_frame(&self, frame: &[u8], guard: Option<u64>) -> Result<(), WriteError> {
        let link = self.0.link.lock().await;
        let Some(link) = link.as_ref() else {
            return Err(WriteError::NoLink);
        };
        if guard.is_some_and(|g| g != self.0.stop_gen.load(Ordering::SeqCst)) {
            return Err(WriteError::CancelledByStop);
        }
        let is_write = !is_read_frame(frame);
        if is_write {
            // Laisser la console finir de répondre à la dernière lecture.
            let last = *self.0.last_tx.lock().unwrap();
            if let Some(last) = last {
                sleep_until(last + WRITE_QUIET).await;
            }
        }
        let r = link.port.write(frame).await.map_err(WriteError::Ble);
        *self.0.last_tx.lock().unwrap() = Some(Instant::now());
        if is_write {
            // Et ne rien envoyer d'autre pendant qu'elle traite l'écriture :
            // le verrou de liaison est gardé pendant ce silence.
            sleep(WRITE_QUIET).await;
            *self.0.priority_read.lock().unwrap() = written_value(frame);
        }
        // Diagnostic : on ne journalise que les écritures (pas les lectures du polling).
        if is_write {
            eprintln!("{} [ble] -> {} ({})", t(), hex(frame), if r.is_ok() { "envoyé" } else { "ÉCHEC" });
            *self.0.log_rx_until.lock().unwrap() =
                Some(std::time::Instant::now() + Duration::from_secs(3));
        }
        r
    }

    async fn clear_link(&self, gen: u64) {
        let mut link = self.0.link.lock().await;
        if link.as_ref().is_some_and(|l| l.gen == gen) {
            *link = None;
        }
    }

    /// Commande d'écriture ordinaire (jamais le stop) : vérifications dans le
    /// modèle, puis écriture annulable par un stop.
    async fn command_write(
        &self,
        prepare: impl FnOnce(&Model) -> Result<Vec<u8>, String>,
    ) -> Result<(), String> {
        let guard = self.0.stop_gen.load(Ordering::SeqCst);
        let frame = prepare(&self.model());
        let result = match frame {
            Ok(frame) => self
                .write_frame(&frame, Some(guard))
                .await
                .map_err(|e| e.message()),
            Err(e) => Err(e),
        };
        self.update(|m| m.set_error(result.as_ref().err().cloned()));
        result
    }

    // ── Commandes publiques ─────────────────────────────────────────────────

    pub fn snapshot(&self) -> TreadmillSnapshot {
        self.model().snapshot()
    }

    /// Connexion voulue. Sans effet si une session existe déjà (connexion en
    /// cours, connecté ou en reconnexion). Rend la main une fois connecté, ou
    /// avec l'erreur de la première tentative.
    pub async fn connect(&self) -> Result<(), String> {
        let (first_tx, first_rx) = oneshot::channel();
        {
            let mut slot = self.session_slot();
            if slot.is_some() {
                return Ok(());
            }
            let gen = self.0.next_gen.fetch_add(1, Ordering::SeqCst) + 1;
            let (cancel_tx, cancel_rx) = watch::channel(false);
            self.0.pending_stop.store(false, Ordering::SeqCst);
            {
                let mut m = self.model();
                m.set_error(None);
                m.set_connection(ConnectionState::Scanning);
            }
            let this = self.clone();
            let task = tokio::spawn(async move {
                this.session_task(gen, cancel_rx, first_tx).await;
            });
            *slot = Some(SessionHandle {
                gen,
                cancel: cancel_tx,
                task,
            });
        }
        self.publish();
        first_rx
            .await
            .unwrap_or_else(|_| Err("Connexion interrompue".into()))
    }

    /// Déconnexion voulue : arrête la session (y compris une reconnexion en
    /// cours). N'envoie rien au tapis.
    pub async fn disconnect(&self) -> Result<(), String> {
        let handle = self.session_slot().take();
        self.0.pending_stop.store(false, Ordering::SeqCst);
        if let Some(h) = handle {
            let _ = h.cancel.send(true);
            let mut task = h.task;
            if timeout(JOIN_TIMEOUT, &mut task).await.is_err() {
                // La tâche ne s'est pas arrêtée à temps : on la coupe.
                task.abort();
                self.clear_link(h.gen).await;
            }
        }
        // La tâche a normalement déjà tout remis à zéro ; on s'en assure, sauf
        // si une nouvelle connexion a démarré entre-temps.
        if self.session_slot().is_none() {
            self.update(|m| {
                m.on_disconnected();
                m.set_error(None);
            });
        }
        Ok(())
    }

    /// Démarrage : Online = 0 (compte à rebours 3-2-1 puis 0,8 km/h).
    ///
    /// Constaté sur le RUN 7 : resté longtemps en pause (compteurs gardés), le
    /// tapis ignore Online = 0 tant qu'un second arrêt (Online = 1) n'a pas remis
    /// ses compteurs à zéro. Si la bande n'est pas partie après `START_CONFIRM`,
    /// on fait donc cette remise à zéro puis un nouvel essai. Un STOP de
    /// l'utilisateur pendant la séquence l'interrompt aussitôt.
    pub async fn start(&self) -> Result<(), String> {
        if self.0.starting.swap(true, Ordering::SeqCst) {
            return Err("Démarrage déjà en cours.".into());
        }
        let r = self.start_sequence().await;
        self.0.starting.store(false, Ordering::SeqCst);
        r
    }

    async fn start_sequence(&self) -> Result<(), String> {
        let guard = self.0.stop_gen.load(Ordering::SeqCst);
        self.command_write(|m| {
            m.check_start()?;
            Ok(write_u8_frame(ValueId::Online, 0))
        })
        .await?;
        if self.wait_started(guard).await || self.stop_since(guard) {
            return Ok(());
        }
        eprintln!("[cmd] démarrage ignoré par le tapis : remise à zéro puis nouvel essai");
        let online = |v| write_u8_frame(ValueId::Online, v);
        let attempt = async {
            self.write_frame(&online(1), Some(guard)).await?;
            sleep(START_RESET_PAUSE).await;
            self.write_frame(&online(0), Some(guard)).await
        };
        match attempt.await {
            Ok(()) => {}
            Err(WriteError::CancelledByStop) => return Ok(()),
            Err(e) => {
                let msg = e.message();
                self.update(|m| m.set_error(Some(msg.clone())));
                return Err(msg);
            }
        }
        if self.wait_started(guard).await || self.stop_since(guard) {
            return Ok(());
        }
        let msg = "Le tapis n'a pas démarré. Vérifiez la clé de sécurité, ou démarrez depuis la console."
            .to_string();
        self.update(|m| m.set_error(Some(msg.clone())));
        Err(msg)
    }

    /// Vrai si un STOP a été demandé depuis `guard`.
    fn stop_since(&self, guard: u64) -> bool {
        self.0.stop_gen.load(Ordering::SeqCst) != guard
    }

    /// Attend que la bande parte (Online lu à 0, ou consigne passée au-dessus
    /// de 0), au plus `START_CONFIRM`. Faux si un STOP arrive entre-temps.
    async fn wait_started(&self, guard: u64) -> bool {
        let end = Instant::now() + START_CONFIRM;
        while Instant::now() < end {
            if self.stop_since(guard) {
                return false;
            }
            let snap = self.model().snapshot();
            if snap.running == Some(true) || snap.target_speed_kmh.unwrap_or(0.0) > 0.0 {
                return true;
            }
            sleep(Duration::from_millis(100)).await;
        }
        false
    }

    /// Arrêt : Online = 1. Prioritaire sur toute écriture en attente, aucune
    /// condition sur l'état de la bande.
    pub async fn stop(&self) -> Result<(), String> {
        // D'abord invalider les écritures en attente, ensuite prendre le verrou.
        self.0.stop_gen.fetch_add(1, Ordering::SeqCst);
        let frame = write_u8_frame(ValueId::Online, 1);
        match self.write_frame(&frame, None).await {
            Ok(()) => {
                self.0.pending_stop.store(false, Ordering::SeqCst);
                self.update(|m| m.set_error(None));
                Ok(())
            }
            Err(e) => {
                let has_session = self.session_slot().is_some();
                let msg = if has_session {
                    // Liaison absente ou cassée : on le retient pour la reconnexion.
                    self.0.pending_stop.store(true, Ordering::SeqCst);
                    match e {
                        WriteError::Ble(_) => format!("{} {MSG_STOP_DEFERRED}", e.message()),
                        _ => MSG_STOP_DEFERRED.to_string(),
                    }
                } else {
                    MSG_STOP_DISCONNECTED.to_string()
                };
                self.update(|m| m.set_error(Some(msg.clone())));
                Err(msg)
            }
        }
    }

    pub async fn set_target_speed(&self, kmh: f32) -> Result<(), String> {
        self.command_write(|m| Ok(write_u16_frame(ValueId::SpeedSet, m.speed_command(kmh)?)))
            .await
    }

    pub async fn set_target_incline(&self, pct: f32) -> Result<(), String> {
        self.command_write(|m| {
            Ok(write_u16_frame(
                ValueId::InclineSet,
                m.incline_command(pct)?,
            ))
        })
        .await
    }

    /// Règle le plafond ; si la consigne courante le dépasse (bande en marche),
    /// elle est ramenée au plafond (on ne fait que ralentir).
    pub async fn set_speed_cap(&self, kmh: f32) -> Result<(), String> {
        let guard = self.0.stop_gen.load(Ordering::SeqCst);
        let lower = self.model().set_speed_cap(kmh);
        let result = match lower {
            Err(e) => Err(e),
            Ok(None) => Ok(()),
            Ok(Some(raw)) => {
                let frame = write_u16_frame(ValueId::SpeedSet, raw);
                match self.write_frame(&frame, Some(guard)).await {
                    // Un stop est passé entre-temps : il prime, rien à abaisser.
                    Ok(()) | Err(WriteError::CancelledByStop) => Ok(()),
                    Err(e) => Err(format!(
                        "Plafond enregistré, mais la consigne n'a pas pu être abaissée : {}",
                        e.message()
                    )),
                }
            }
        };
        self.update(|m| m.set_error(result.as_ref().err().cloned()));
        result
    }

    // ── Tâche de session ────────────────────────────────────────────────────

    async fn session_task(
        self,
        gen: u64,
        mut cancel: watch::Receiver<bool>,
        first: oneshot::Sender<Result<(), String>>,
    ) {
        let mut first = Some(first);
        let mut conn = match self.establish(&mut cancel, true).await {
            Ok(c) => c,
            Err(f) => {
                let (msg, error) = match f {
                    Failure::Cancelled => ("Connexion annulée".to_string(), None),
                    Failure::Error(e) => (e.clone(), Some(e)),
                };
                self.finalize(gen, None, error).await;
                if let Some(tx) = first.take() {
                    let _ = tx.send(Err(msg));
                }
                return;
            }
        };

        loop {
            let port = conn.port.clone();
            *self.0.link.lock().await = Some(Link {
                gen,
                port: conn.port.clone(),
            });
            self.update(|m| m.on_connected());
            if let Some(tx) = first.take() {
                let _ = tx.send(Ok(()));
            }

            let exit = self.run_connected(&mut conn, &mut cancel).await;
            self.clear_link(gen).await;
            drop(conn);

            let reason = match exit {
                Exit::Cancelled => {
                    self.finalize(gen, Some(port), None).await;
                    return;
                }
                Exit::Lost(reason) => reason,
            };

            // Perte non voulue : la bande continue de tourner (vérifié).
            self.update(|m| {
                m.on_link_lost();
                m.set_error(Some(reason));
            });
            port.disconnect().await;

            // Reconnexion : uniquement handshake + lectures (voir `establish`).
            conn = loop {
                tokio::select! {
                    biased;
                    _ = cancelled(&mut cancel) => {
                        self.finalize(gen, None, None).await;
                        return;
                    }
                    _ = sleep(RETRY_DELAY) => {}
                }
                match self.establish(&mut cancel, false).await {
                    Ok(c) => break c,
                    Err(Failure::Cancelled) => {
                        self.finalize(gen, None, None).await;
                        return;
                    }
                    Err(Failure::Error(e)) => self.update(|m| {
                        m.set_error(Some(format!(
                            "{MSG_LINK_LOST}, reconnexion en cours… ({e})"
                        )))
                    }),
                }
            };
        }
    }

    /// Fin de session : libère la liaison et remet l'état à « déconnecté »,
    /// sauf si une session plus récente a pris le relais.
    async fn finalize(&self, gen: u64, port: Option<Port>, error: Option<String>) {
        self.clear_link(gen).await;
        if let Some(p) = port {
            p.disconnect().await;
        }
        let current = {
            let mut slot = self.session_slot();
            match slot.as_ref() {
                Some(h) if h.gen == gen => {
                    *slot = None;
                    true
                }
                Some(_) => false,
                None => true,
            }
        };
        if current {
            self.0.pending_stop.store(false, Ordering::SeqCst);
            self.update(|m| {
                m.on_disconnected();
                m.set_error(error);
            });
        }
    }

    /// Établit une liaison complète (scan, connexion, abonnement, handshake),
    /// annulable à tout moment. N'ÉCRIT QUE LE HANDSHAKE.
    async fn establish(
        &self,
        cancel: &mut watch::Receiver<bool>,
        initial: bool,
    ) -> Result<Conn, Failure> {
        let adapter = match &self.0.backend {
            Backend::Ble(_) => Some(self.adapter().await.map_err(Failure::Error)?),
            Backend::Sim(_) => None,
        };
        let mut opened: Option<Opened> = None;
        let outcome = {
            let attempt = self.establish_inner(adapter.as_ref(), initial, &mut opened);
            tokio::select! {
                biased;
                _ = cancelled(cancel) => None,
                r = attempt => Some(r),
            }
        };
        match outcome {
            Some(Ok(conn)) => Ok(conn),
            other => {
                // Échec ou annulation : ne pas laisser de liaison à moitié ouverte.
                if let Some(a) = &adapter {
                    let _ = a.stop_scan().await;
                }
                match opened {
                    Some(Opened::Ble(p)) => {
                        let _ = timeout(DISCONNECT_TIMEOUT, p.disconnect()).await;
                    }
                    Some(Opened::Sim(p)) => p.disconnect(),
                    None => {}
                }
                match other {
                    Some(Err(e)) => Err(Failure::Error(e)),
                    _ => Err(Failure::Cancelled),
                }
            }
        }
    }

    async fn establish_inner(
        &self,
        adapter: Option<&Adapter>,
        initial: bool,
        opened: &mut Option<Opened>,
    ) -> Result<Conn, String> {
        let limit = if initial {
            SCAN_TIMEOUT
        } else {
            RESCAN_TIMEOUT
        };
        let (port, mut rx, lost) = match (&self.0.backend, adapter) {
            (Backend::Sim(sim), _) => {
                sim.scan(limit).await?;
                if initial {
                    self.update(|m| m.set_connection(ConnectionState::Connecting));
                }
                let (port, rx) = sim.connect().await?;
                *opened = Some(Opened::Sim(port.clone()));
                let rx: ByteStream = Box::pin(rx);
                let lost: LostStream = Box::pin(futures::stream::pending());
                (Port::Sim(port), rx, lost)
            }
            (Backend::Ble(_), Some(adapter)) => self.open_ble(adapter, initial, limit, opened).await?,
            (Backend::Ble(_), None) => return Err("Aucun adaptateur Bluetooth trouvé".into()),
        };

        // Handshake obligatoire : sans lui le tapis reste muet et coupe la liaison.
        let mut decoder = FrameDecoder::new();
        port.write(&handshake_frame())
            .await
            .map_err(|e| format!("Envoi du message d'identification impossible : {e}"))?;
        let answered = timeout(HANDSHAKE_TIMEOUT, async {
            while let Some(bytes) = rx.next().await {
                // Les trames au CRC invalide sont ignorées.
                if decoder
                    .push(&bytes)
                    .into_iter()
                    .flatten()
                    .any(|m| is_handshake_answer(&m))
                {
                    return true;
                }
            }
            false
        })
        .await;
        match answered {
            Ok(true) => {}
            Ok(false) => return Err("Liaison coupée pendant l'identification".into()),
            Err(_) => return Err("Le tapis n'a pas répondu au message d'identification".into()),
        }

        Ok(Conn {
            port,
            rx,
            lost,
            decoder,
        })
    }

    /// Bluetooth : scan, connexion, découverte des services, abonnement à FFF1.
    async fn open_ble(
        &self,
        adapter: &Adapter,
        initial: bool,
        limit: Duration,
        opened: &mut Option<Opened>,
    ) -> Result<(Port, ByteStream, LostStream), String> {
        if let Ok(CentralState::PoweredOff) = adapter.adapter_state().await {
            return Err("Bluetooth désactivé : activez-le puis réessayez".into());
        }
        let peripheral = find_treadmill(adapter, limit).await?;
        *opened = Some(Opened::Ble(peripheral.clone()));
        if initial {
            self.update(|m| m.set_connection(ConnectionState::Connecting));
        }

        match timeout(CONNECT_TIMEOUT, peripheral.connect()).await {
            Ok(Ok(())) => {}
            Ok(Err(e)) => return Err(format!("Connexion Bluetooth impossible : {e}")),
            Err(_) => return Err("Le tapis ne répond pas à la demande de connexion".into()),
        }
        match timeout(DISCOVER_TIMEOUT, peripheral.discover_services()).await {
            Ok(Ok(())) => {}
            Ok(Err(e)) => return Err(format!("Lecture des services Bluetooth impossible : {e}")),
            Err(_) => return Err("Lecture des services Bluetooth trop longue".into()),
        }
        let chars = peripheral.characteristics();
        let find = |uuid: Uuid| chars.iter().find(|c| c.uuid == uuid).cloned();
        let (Some(tx), Some(rx)) = (find(TX_UUID), find(RX_UUID)) else {
            return Err(
                "Caractéristiques FFF1/FFF2 introuvables : l'appareil trouvé n'est pas un RUN 7 \
                 compatible"
                    .into(),
            );
        };

        let notifications = peripheral
            .notifications()
            .await
            .map_err(|e| format!("Abonnement aux réponses du tapis impossible : {e}"))?;
        peripheral
            .subscribe(&rx)
            .await
            .map_err(|e| format!("Abonnement aux réponses du tapis impossible : {e}"))?;
        let bytes: ByteStream = Box::pin(notifications.filter_map(|n| {
            futures::future::ready((n.uuid == RX_UUID).then_some(n.value))
        }));

        // Perte de liaison signalée par le système (le flux de notifications
        // ne se termine pas toujours).
        let id = peripheral.id();
        let lost: LostStream = match adapter.events().await {
            Ok(events) => Box::pin(events.filter_map(move |ev| {
                let gone = matches!(ev, CentralEvent::DeviceDisconnected(ref l) if *l == id);
                futures::future::ready(gone.then_some(()))
            })),
            Err(_) => Box::pin(futures::stream::pending()),
        };

        Ok((Port::Ble { peripheral, tx }, bytes, lost))
    }

    /// Décode les octets reçus et met le modèle à jour. Rend le nombre de
    /// trames valides arrivées.
    fn ingest(&self, decoder: &mut FrameDecoder, bytes: &[u8]) -> usize {
        if self.0.log_rx_until.lock().unwrap().is_some_and(|t| std::time::Instant::now() < t) {
            eprintln!("{} [ble] <- brut {}", t(), hex(bytes));
        }
        let mut count = 0;
        let mut m = self.model();
        // Une trame au CRC invalide est ignorée : le polling suivant la relira.
        for msg in decoder.push(bytes).into_iter().flatten() {
            count += 1;
            if let Some(reading) = decode(&msg) {
                // Diagnostic : changements de consignes et d'état de marche lus sur le tapis.
                let before = m.snapshot();
                m.apply(reading);
                let after = m.snapshot();
                if before.running != after.running
                    || before.target_speed_kmh != after.target_speed_kmh
                    || before.target_incline_pct != after.target_incline_pct
                {
                    eprintln!(
                        "{} [ble] <- running={:?} consigne={:?} km/h incl={:?} %",
                        t(),
                        after.running, after.target_speed_kmh, after.target_incline_pct
                    );
                }
            }
        }
        count
    }

    /// Envoie un stop mémorisé pendant une coupure, une fois Online relu et
    /// seulement si la bande tourne. (Un second stop remettrait peut-être les
    /// compteurs à zéro, d'où la vérification.)
    async fn flush_pending_stop(&self) -> Result<(), WriteError> {
        if !self.0.pending_stop.load(Ordering::SeqCst) {
            return Ok(());
        }
        let running = self.model().running();
        match running {
            None => Ok(()), // pas encore relu : cycle suivant
            Some(false) => {
                self.0.pending_stop.store(false, Ordering::SeqCst);
                self.update(|m| m.set_error(None));
                Ok(())
            }
            Some(true) => {
                self.0.pending_stop.store(false, Ordering::SeqCst);
                self.write_frame(&write_u8_frame(ValueId::Online, 1), None)
                    .await?;
                self.update(|m| m.set_error(None));
                Ok(())
            }
        }
    }

    /// Boucle de polling d'une liaison établie, jusqu'à annulation ou perte.
    async fn run_connected(&self, conn: &mut Conn, cancel: &mut watch::Receiver<bool>) -> Exit {
        let mut queue: VecDeque<ValueId> = VecDeque::new();
        let mut cycle_start = Instant::now();
        let mut next_read = Instant::now();
        let mut last_answer = Instant::now();
        let mut cycles: u32 = 0;
        // Lectures envoyées dont on attend encore la réponse, et dernier envoi.
        let mut inflight: u32 = 0;
        let mut last_sent = Instant::now();

        loop {
            tokio::select! {
                biased;
                _ = cancelled(cancel) => return Exit::Cancelled,
                Some(()) = conn.lost.next() => return Exit::Lost(MSG_LINK_LOST.into()),
                bytes = conn.rx.next() => match bytes {
                    None => return Exit::Lost(MSG_LINK_LOST.into()),
                    Some(bytes) => {
                        let answers = self.ingest(&mut conn.decoder, &bytes);
                        if answers > 0 {
                            last_answer = Instant::now();
                            inflight = inflight.saturating_sub(answers as u32);
                        }
                    }
                },
                _ = sleep_until(next_read) => {
                    let now = Instant::now();
                    if inflight >= MAX_INFLIGHT {
                        if now.duration_since(last_sent) < ANSWER_TIMEOUT {
                            // Fenêtre pleine : on attend des réponses.
                            next_read = now + READ_SPACING;
                            continue;
                        }
                        inflight = 0; // réponses perdues : on repart
                    }
                    // Une valeur tout juste écrite passe devant le reste du cycle.
                    let priority = self.0.priority_read.lock().unwrap().take();
                    let cycle_due = now >= cycle_start + POLL_PERIOD;
                    if queue.is_empty() && !cycle_due && priority.is_none() {
                        // Entre deux cycles : on se réveille souvent pour ne pas
                        // faire attendre une relecture prioritaire.
                        next_read = (cycle_start + POLL_PERIOD).min(now + Duration::from_millis(100));
                        continue;
                    }
                    if queue.is_empty() && cycle_due {
                        // Fin de cycle : horloge, publication, garde-fous.
                        let now = Instant::now();
                        self.update(|m| m.tick(now.into_std()));
                        if now.duration_since(last_answer) > WATCHDOG {
                            return Exit::Lost("Le tapis ne répond plus".into());
                        }
                        if let Err(e) = self.flush_pending_stop().await {
                            return Exit::Lost(e.message());
                        }
                        cycles += 1;
                        if cycles <= STATIC_CYCLES || cycles % STATIC_CYCLES == 0 {
                            queue.extend(self.model().missing_static_ids(cycles <= ARTICLE_CYCLES));
                        }
                        queue.extend(FAST_IDS);
                        if cycles % SLOW_EVERY == 0 {
                            queue.extend(SLOW_IDS);
                        }
                        cycle_start = now;
                    }
                    if let Some(value) = priority.or_else(|| queue.pop_front()) {
                        if let Err(e) = self.write_frame(&read_frame(value), None).await {
                            return Exit::Lost(e.message());
                        }
                        inflight += 1;
                        last_sent = Instant::now();
                    }
                    next_read = Instant::now() + READ_SPACING;
                }
            }
        }
    }
}

/// Session complète contre le tapis simulé (temps tokio en pause : les
/// minutes simulées passent en quelques millisecondes).
#[cfg(test)]
mod tests {
    use super::*;
    use crate::dto::SimAction;

    fn treadmill() -> (Treadmill, Simulator) {
        let sim = Simulator::new();
        (Treadmill::new(Box::new(|_| {}), Some(sim.clone())), sim)
    }

    /// Attend que `cond` soit vrai sur l'état publié, au plus `secs` secondes.
    async fn until(t: &Treadmill, secs: u64, what: &str, cond: impl Fn(&TreadmillSnapshot) -> bool) {
        let end = Instant::now() + Duration::from_secs(secs);
        loop {
            let snap = t.snapshot();
            if cond(&snap) {
                return;
            }
            assert!(Instant::now() < end, "{what} : pas atteint en {secs} s ({snap:?})");
            sleep(Duration::from_millis(50)).await;
        }
    }

    async fn connected_and_read(t: &Treadmill) {
        t.connect().await.expect("connexion au simulateur");
        until(t, 10, "valeurs lues", |s| {
            s.running.is_some() && s.limits.is_some() && s.device.is_some()
        })
        .await;
    }

    #[tokio::test(start_paused = true)]
    async fn seance_complete() {
        let (t, sim) = treadmill();
        connected_and_read(&t).await;
        let snap = t.snapshot();
        assert_eq!(snap.running, Some(false));
        let device = snap.device.unwrap();
        assert_eq!((device.name.as_str(), device.id.as_str()), ("RUN 7", "STB1T0158"));
        assert_eq!(device.article.as_deref(), Some("07883-600"));
        assert_eq!(snap.limits.unwrap().speed_max_kmh, 20.0);

        t.start().await.expect("démarrage");
        until(&t, 10, "bande partie", |s| s.speed_kmh == Some(0.8)).await;

        t.set_target_speed(3.0).await.expect("consigne");
        until(&t, 5, "consigne relue", |s| s.target_speed_kmh == Some(3.0)).await;
        until(&t, 20, "vitesse atteinte", |s| s.speed_kmh.unwrap_or(0.0) >= 2.9).await;

        // Plafond sous la consigne : la consigne est ramenée au plafond.
        t.set_speed_cap(2.0).await.expect("plafond");
        until(&t, 5, "consigne plafonnée", |s| s.target_speed_kmh == Some(2.0)).await;
        assert!(t.set_target_speed(5.0).await.is_ok());
        until(&t, 5, "consigne toujours plafonnée", |s| s.target_speed_kmh == Some(2.0)).await;

        t.set_target_incline(2.0).await.expect("inclinaison");
        until(&t, 15, "inclinaison atteinte", |s| s.incline_pct == Some(2.0)).await;

        sleep(Duration::from_secs(30)).await;
        until(&t, 5, "compteurs", |s| {
            s.distance_m.unwrap_or(0) > 10 && s.energy_kcal.unwrap_or(0.0) > 0.0 && s.elapsed_s >= 30
        })
        .await;

        // Pause : compteurs gardés.
        t.stop().await.expect("pause");
        until(&t, 5, "pause", |s| s.running == Some(false)).await;
        until(&t, 10, "bande arrêtée", |s| s.speed_kmh == Some(0.0)).await;
        let paused = t.snapshot();
        assert!(paused.distance_m.unwrap() > 10);
        assert!(!sim.status().running);

        // Second arrêt : fin de séance, compteurs et temps remis à zéro.
        t.stop().await.expect("fin de séance");
        until(&t, 5, "remise à zéro", |s| s.distance_m == Some(0) && s.elapsed_s == 0).await;

        t.disconnect().await.unwrap();
        assert_eq!(t.snapshot().connection, ConnectionState::Disconnected);
        assert!(!sim.status().link_active);
    }

    #[tokio::test(start_paused = true)]
    async fn coupure_bande_en_marche_puis_reconnexion() {
        let (t, sim) = treadmill();
        connected_and_read(&t).await;
        t.start().await.unwrap();
        until(&t, 10, "bande partie", |s| s.running == Some(true)).await;

        sim.act(SimAction::LinkLoss { seconds: 8 }).unwrap();
        until(&t, 5, "perte vue", |s| {
            s.connection == ConnectionState::Reconnecting && s.lost_while_running
        })
        .await;
        // Arrêt demandé pendant la coupure : refusé tout de suite, envoyé à la reconnexion.
        assert!(t.stop().await.is_err());
        assert!(sim.status().running, "la bande continue pendant la coupure");

        until(&t, 30, "reconnecté", |s| s.connection == ConnectionState::Connected).await;
        assert!(!t.snapshot().lost_while_running);
        until(&t, 10, "arrêt mémorisé envoyé", |s| s.running == Some(false)).await;
        assert!(!sim.status().running);
    }

    #[tokio::test(start_paused = true)]
    async fn cle_retiree_puis_remise() {
        let (t, sim) = treadmill();
        connected_and_read(&t).await;
        t.start().await.unwrap();
        until(&t, 10, "bande partie", |s| s.running == Some(true)).await;

        sim.act(SimAction::PullKey).unwrap();
        until(&t, 5, "perte vue", |s| s.connection == ConnectionState::Reconnecting).await;
        sleep(Duration::from_secs(20)).await;
        assert_eq!(t.snapshot().connection, ConnectionState::Reconnecting);

        sim.act(SimAction::InsertKey).unwrap();
        until(&t, 15, "reconnecté", |s| s.connection == ConnectionState::Connected).await;
        until(&t, 5, "bande vue arrêtée", |s| s.running == Some(false)).await;
    }

    #[tokio::test(start_paused = true)]
    async fn demarrage_apres_longue_pause_et_console() {
        let (t, sim) = treadmill();
        connected_and_read(&t).await;
        t.start().await.unwrap();
        until(&t, 10, "bande partie", |s| s.speed_kmh == Some(0.8)).await;
        sleep(Duration::from_secs(10)).await;
        t.stop().await.unwrap();
        until(&t, 10, "bande arrêtée", |s| {
            s.running == Some(false) && s.target_speed_kmh == Some(0.0)
        })
        .await;

        // Le tapis ignore Online = 0 : l'appli remet à zéro puis réessaie.
        sim.act(SimAction::AgePause).unwrap();
        t.start().await.expect("démarrage après remise à zéro");
        until(&t, 10, "bande repartie", |s| s.running == Some(true)).await;

        // Réglages faits à la console : relus par l'appli.
        until(&t, 10, "fin du compte à rebours", |s| s.target_speed_kmh == Some(0.8)).await;
        sim.act(SimAction::ConsoleSpeed { delta: 1.0 }).unwrap();
        until(&t, 5, "consigne console relue", |s| s.target_speed_kmh == Some(1.8)).await;
        sim.act(SimAction::ConsoleStop).unwrap();
        until(&t, 5, "arrêt console relu", |s| s.running == Some(false)).await;
    }

    #[tokio::test(start_paused = true)]
    async fn incidents_de_transport() {
        let (t, sim) = treadmill();
        connected_and_read(&t).await;
        t.start().await.unwrap();
        until(&t, 10, "bande partie", |s| s.speed_kmh == Some(0.8)).await;

        // Écriture ratée : erreur lisible, la session continue.
        sim.act(SimAction::FailNextWrite).unwrap();
        let err = t.set_target_speed(2.0).await.unwrap_err();
        assert!(err.contains("Écriture Bluetooth impossible"), "{err}");
        assert_eq!(t.snapshot().connection, ConnectionState::Connected);

        // Réponse au CRC invalide : ignorée, rien ne casse.
        sim.act(SimAction::CorruptNextAnswer).unwrap();
        sleep(Duration::from_secs(3)).await;
        assert_eq!(t.snapshot().connection, ConnectionState::Connected);

        // Console muette : le chien de garde coupe, puis la reconnexion aboutit.
        sim.act(SimAction::Mute { seconds: 8 }).unwrap();
        until(&t, 10, "silence détecté", |s| s.connection == ConnectionState::Reconnecting).await;
        until(&t, 30, "reconnecté", |s| s.connection == ConnectionState::Connected).await;
        until(&t, 5, "état relu", |s| s.running == Some(true)).await;
        t.stop().await.unwrap();
    }
}
