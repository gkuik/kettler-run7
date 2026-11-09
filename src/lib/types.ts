// Miroir TypeScript exact de `src-tauri/src/dto.rs` (contrat frontend ↔ Rust).
// Tout est en camelCase ; un `Option<T>` Rust devient `T | null`.
// Toute modification de dto.rs doit être reportée ici.

/** Nom de l'événement Tauri qui porte un `TreadmillSnapshot` à chaque changement. */
export const EVENT_STATE = "treadmill-state";

/** État de la liaison Bluetooth. */
export type ConnectionState =
  | "disconnected"
  | "scanning"
  | "connecting"
  /** Liaison établie et handshake Kettler accepté. */
  | "connected"
  /** Liaison perdue alors qu'on ne l'avait pas demandé ; reconnexion en cours. */
  | "reconnecting";

/** Bornes annoncées par le tapis (SpeedMin/Max, InclineMin/Max). */
export interface Limits {
  speedMinKmh: number;
  speedMaxKmh: number;
  inclineMinPct: number;
  inclineMaxPct: number;
}

/** Identité lue une fois après le handshake. */
export interface DeviceInfo {
  /** DeviceName (20), ex. « RUN 7 ». */
  name: string;
  /** DeviceId (25), ex. « STB1T0158 ». */
  id: string;
  /** Référence d'article (22), ex. « 07883-600 ». */
  article: string | null;
}

/** Photographie complète de l'état du tapis, émise sur `EVENT_STATE`. */
export interface TreadmillSnapshot {
  connection: ConnectionState;
  /**
   * Vrai si la liaison a été perdue pendant que la bande tournait : la bande
   * CONTINUE de tourner dans ce cas (vérifié), l'interface doit alerter.
   */
  lostWhileRunning: boolean;
  /** Online (38) : brut 0 = en marche, 1 = arrêté. */
  running: boolean | null;
  /** SpeedGet (35) / 10 : vitesse réelle de la bande. */
  speedKmh: number | null;
  /** SpeedSet (34) / 10 : consigne. */
  targetSpeedKmh: number | null;
  /** InclineGet (31) / 10 : inclinaison réelle. */
  inclinePct: number | null;
  /** InclineSet (30) / 10 : consigne d'inclinaison. */
  targetInclinePct: number | null;
  /** Distance (15), en mètres (vérifié). */
  distanceM: number | null;
  /** Energy (16) / 10 ; unité 0,1 kcal probable mais non confirmée. */
  energyKcal: number | null;
  /** Pulse (8) ; `null` quand le tapis renvoie 0 (pas de contact). */
  pulseBpm: number | null;
  /** Temps de course calculé par l'application (le tapis ne le fournit pas). */
  elapsedS: number;
  limits: Limits | null;
  device: DeviceInfo | null;
  /** Plafond de vitesse appliqué par l'application à toute consigne envoyée. */
  speedCapKmh: number;
  /** Dernière erreur lisible (connexion, commande refusée…), effacée au succès suivant. */
  lastError: string | null;
}

/** Plafond de vitesse par défaut (même valeur que côté Rust). */
export const DEFAULT_SPEED_CAP_KMH = 6.0;

/** État initial, équivalent du `TreadmillSnapshot::default()` Rust (plafond mis à part). */
export function emptySnapshot(): TreadmillSnapshot {
  return {
    connection: "disconnected",
    lostWhileRunning: false,
    running: null,
    speedKmh: null,
    targetSpeedKmh: null,
    inclinePct: null,
    targetInclinePct: null,
    distanceM: null,
    energyKcal: null,
    pulseBpm: null,
    elapsedS: 0,
    limits: null,
    device: null,
    speedCapKmh: DEFAULT_SPEED_CAP_KMH,
    lastError: null,
  };
}

// ── Simulateur (application lancée avec KETTLER_SIM=1) ──────────────────────

/** Le tapis simulé « vu de la salle » : console, clé, radio. */
export interface SimStatus {
  keyIn: boolean;
  /** Le tapis répond au scan (clé en place, pas de coupure radio en cours). */
  reachable: boolean;
  /** Une liaison est ouverte avec l'application. */
  linkActive: boolean;
  /** Coupure radio : secondes avant que le tapis soit de nouveau visible. */
  linkBackInS: number | null;
  /** Console muette : secondes restantes. */
  mutedForS: number | null;
  running: boolean;
  /** Compte à rebours 3-2-1 : secondes restantes. */
  countdownS: number | null;
  speedKmh: number;
  targetSpeedKmh: number;
  inclinePct: number;
  targetInclinePct: number;
  distanceM: number;
  energyKcal: number;
  handsOn: boolean;
  /** Pause assez longue pour que le tapis ignore un démarrage. */
  longPause: boolean;
}

/** Événement provoqué sur le tapis simulé (`SimAction` dans dto.rs). */
export type SimAction =
  | { kind: "linkLoss"; seconds: number }
  | { kind: "pullKey" }
  | { kind: "insertKey" }
  | { kind: "consoleStart" }
  | { kind: "consoleStop" }
  | { kind: "consoleSpeed"; delta: number }
  | { kind: "consoleIncline"; delta: number }
  | { kind: "handsOn"; on: boolean }
  | { kind: "agePause" }
  | { kind: "mute"; seconds: number }
  | { kind: "corruptNextAnswer" }
  | { kind: "failNextWrite" };
