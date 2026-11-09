import { api, errorMessage } from "./api";
import { smoothDistance, type DistanceSmoothing } from "./controls";
import { emptySnapshot, type TreadmillSnapshot } from "./types";

// État partagé de l'écran : le dernier `TreadmillSnapshot` reçu du Rust (ou du
// tapis simulé), plus ce qui n'appartient qu'à l'interface (erreur de la
// dernière commande, distance lissée). L'interface ne décide de rien : elle
// affiche l'instantané et appelle des commandes.

const SMOOTH_TICK_MS = 250;
/** Durée d'affichage de l'erreur d'une commande (celle du Rust, `lastError`, reste). */
export const COMMAND_ERROR_MS = 10_000;
/**
 * Au-delà, une pause envoyée sans effet visible (bande toujours en marche) est
 * oubliée : le bouton redevient PAUSE. Délai mesuré envoi → relecture : ~1,4 s.
 */
const PAUSE_REQUEST_MS = 5_000;

class TreadmillStore {
  snap = $state<TreadmillSnapshot>(emptySnapshot());
  /** Erreur renvoyée par la dernière commande, effacée au succès suivant (ou après 10 s). */
  commandError = $state<string | null>(null);
  /**
   * Échec du dernier STOP (« Arrêt NON envoyé… »). Persistant : effacé par un
   * STOP réussi, par la fermeture manuelle, ou quand la bande est vue arrêtée.
   */
  stopError = $state<string | null>(null);
  /** STOP accepté pendant une reconnexion : le Rust l'enverra à la reconnexion. */
  stopPending = $state(false);
  /**
   * Arrêt accepté pendant que la bande tournait, pas encore vu dans l'état :
   * le bouton affiche déjà STOP (voir `stopButtonMode`). Effacé dès que la
   * bande est vue arrêtée (ou la liaison perdue), sinon au bout de 5 s.
   */
  pauseRequested = $state(false);
  /**
   * Démarrage envoyé, pas encore terminé. Un démarrage peut prendre ~15 s côté
   * Rust (essai, puis remise à zéro et nouvel essai si le tapis l'ignore) : le
   * bouton reste bloqué pendant ce temps.
   */
  startPending = $state(false);
  /** Distance affichée (mètres), extrapolée entre deux à-coups du tapis. */
  shownDistanceM = $state<number | null>(null);

  private smoothing: DistanceSmoothing | null = null;
  private lastSmooth = 0;
  private started = false;
  private errorTimer: ReturnType<typeof setTimeout> | null = null;
  private pauseTimer: ReturnType<typeof setTimeout> | null = null;

  /** Abonnement à l'événement d'état puis lecture de l'état courant. */
  async init(): Promise<void> {
    if (this.started) return;
    this.started = true;
    this.lastSmooth = performance.now();
    await api.onState((s) => this.receive(s));
    try {
      this.receive(await api.getState());
    } catch (e) {
      this.setCommandError(errorMessage(e));
    }
    setInterval(() => this.smooth(), SMOOTH_TICK_MS);
  }

  /** Exécute une commande ; son erreur éventuelle reste affichée jusqu'au succès suivant. */
  async run(command: () => Promise<void>): Promise<void> {
    try {
      await command();
      this.setCommandError(null);
    } catch (e) {
      this.setCommandError(errorMessage(e));
    }
  }

  /** Démarrer ou reprendre : voir `startPending`. */
  async start(): Promise<void> {
    this.startPending = true;
    try {
      await this.run(api.start);
    } finally {
      this.startPending = false;
    }
  }

  /** STOP : toujours tenté, quel que soit l'état ; son échec est mis en évidence. */
  async stop(): Promise<void> {
    const reconnecting = this.snap.connection === "reconnecting";
    try {
      await api.stop();
      // Bande encore vue en marche : la pause est partie, l'état suivra.
      if (this.snap.connection === "connected" && this.snap.running === true) {
        this.setPauseRequested(true);
      }
      this.stopError = null;
      this.stopPending = reconnecting && this.snap.connection === "reconnecting";
      this.setCommandError(null);
    } catch (e) {
      this.stopError = errorMessage(e);
    }
  }

  dismissStopError(): void {
    this.stopError = null;
  }

  private setPauseRequested(value: boolean): void {
    if (this.pauseTimer) clearTimeout(this.pauseTimer);
    this.pauseTimer = null;
    this.pauseRequested = value;
    if (value) this.pauseTimer = setTimeout(() => (this.pauseRequested = false), PAUSE_REQUEST_MS);
  }

  private setCommandError(message: string | null): void {
    if (this.errorTimer) clearTimeout(this.errorTimer);
    this.errorTimer = null;
    this.commandError = message;
    if (message) {
      this.errorTimer = setTimeout(() => (this.commandError = null), COMMAND_ERROR_MS);
    }
  }

  private receive(s: TreadmillSnapshot): void {
    this.snap = s;
    // Liaison rétablie (ou abandonnée) : l'arrêt mémorisé est parti (ou annulé).
    if (s.connection !== "reconnecting") this.stopPending = false;
    // Pause vue (ou état devenu inconnu) : le libellé suit à nouveau l'instantané.
    if (this.pauseRequested && (s.connection !== "connected" || s.running !== true)) {
      this.setPauseRequested(false);
    }
    // Bande vue arrêtée : l'alerte d'échec du STOP n'a plus lieu d'être.
    if (this.stopError && s.connection === "connected" && s.running === false) {
      this.stopError = null;
    }
    this.smooth();
  }

  private smooth(): void {
    const now = performance.now();
    const dt = (now - this.lastSmooth) / 1000;
    this.lastSmooth = now;
    const s = this.snap;
    // Liaison absente : les valeurs sont figées, on n'extrapole pas.
    const speed = s.connection === "connected" ? s.speedKmh : 0;
    this.smoothing = smoothDistance(this.smoothing, s.distanceM, speed, dt);
    this.shownDistanceM = this.smoothing ? Math.floor(this.smoothing.shown) : null;
  }
}

export const treadmill = new TreadmillStore();
