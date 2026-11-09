// Logique pure de l'écran de pilotage (testée dans controls.test.ts) :
// ce qu'on a le droit de faire, consignes suivantes, raccourcis clavier,
// lissage de la distance.
//
// Le Rust reste seul juge (il arrondit, borne et refuse lui-même) : tout ce qui
// est ici sert à ne pas proposer à l'utilisateur une action vouée à l'échec et à
// lui montrer la valeur qui sera réellement appliquée.

import type { TreadmillSnapshot } from "./types";

/** Bornes du plafond de vitesse acceptées par le Rust. */
export const CAP_MIN_KMH = 1;
export const CAP_MAX_KMH = 20;

/** Valeurs de repli tant que le tapis n'a pas annoncé ses bornes (RUN 7 vérifié). */
const FALLBACK_SPEED_MIN = 0.8;
const FALLBACK_SPEED_MAX = 20;
const FALLBACK_INCLINE_MIN = 0;
const FALLBACK_INCLINE_MAX = 15;

/** Arrondi au dixième, sans les artefacts flottants (0.1 + 0.2). */
export function round1(x: number): number {
  return Math.round((x + Math.sign(x) * 1e-9) * 10) / 10;
}

export function clamp(x: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, x));
}

/** Liaison établie (handshake fait) : seules les commandes y ont un sens. */
export function isConnected(s: TreadmillSnapshot): boolean {
  return s.connection === "connected";
}

/** Démarrer : connecté et bande à l'arrêt. */
export function canStart(s: TreadmillSnapshot): boolean {
  return isConnected(s) && s.running === false;
}

/**
 * Séance en pause : bande arrêtée, compteurs gardés par la console. Sur le
 * RUN 7, un premier STOP met en pause ; un second STOP termine la séance et
 * remet distance et calories à zéro (vérifié).
 */
export function isPaused(s: TreadmillSnapshot): boolean {
  return (
    isConnected(s) &&
    s.running === false &&
    ((s.distanceM ?? 0) > 0 || (s.energyKcal ?? 0) > 0 || s.elapsedS > 0)
  );
}

// ── Bouton d'arrêt (PAUSE / STOP) ──────────────────────────────────────────

/** Ce qu'affiche le bouton d'arrêt ; les deux envoient le même arrêt (Online = 1). */
export type StopButtonMode = "pause" | "stop";

/**
 * Libellé du bouton d'arrêt, ou `null` quand il n'y a rien à arrêter.
 *
 * - Bande en marche (compte à rebours compris) → « pause » : un premier arrêt
 *   met la séance en pause, compteurs gardés.
 * - Séance en pause → « stop » : un second arrêt la termine (distance et
 *   calories remises à zéro par le tapis).
 * - `pauseRequested` : la pause vient d'être envoyée mais la bande est encore
 *   vue en marche (l'état a ~1,4 s de retard) → « stop » tout de suite, pour
 *   qu'un second appui fasse bien ce qu'annonce le bouton.
 * - État inconnu → bouton TOUJOURS affiché, selon le dernier état connu :
 *   reconnexion (la bande continue de tourner après une coupure, vérifié), ou
 *   liaison établie mais Online pas encore relu.
 * - Connecté, bande arrêtée sans séance, ou non connecté → `null`.
 *
 * Échap et Espace envoient l'arrêt quel que soit ce résultat.
 */
export function stopButtonMode(
  s: TreadmillSnapshot,
  pauseRequested = false,
): StopButtonMode | null {
  if (s.connection === "reconnecting") {
    if (s.lostWhileRunning || s.running !== false) return "pause";
    return "stop";
  }
  if (!isConnected(s)) return null;
  if (s.running === null) return "pause";
  if (s.running) return pauseRequested ? "stop" : "pause";
  return isPaused(s) ? "stop" : null;
}

/** Changer vitesse ou inclinaison : connecté et bande en marche (le Rust refuse sinon). */
export function canAdjust(s: TreadmillSnapshot): boolean {
  return isConnected(s) && s.running === true;
}

/**
 * Compte à rebours de démarrage : la bande est « en marche » mais la consigne
 * n'a pas encore atteint la vitesse minimale. Le tapis ignore alors les écritures.
 */
export function isStarting(s: TreadmillSnapshot): boolean {
  return s.running === true && (s.targetSpeedKmh ?? 0) < speedBounds(s).min - 0.05;
}

/** Bornes de consigne de vitesse : `[speedMin, min(plafond, speedMax)]`. */
export function speedBounds(s: TreadmillSnapshot): { min: number; max: number } {
  const min = s.limits?.speedMinKmh ?? FALLBACK_SPEED_MIN;
  const max = Math.min(s.speedCapKmh, s.limits?.speedMaxKmh ?? FALLBACK_SPEED_MAX);
  return { min, max: Math.max(min, max) };
}

/** Graduation haute de la jauge de vitesse : la vitesse maximale du tapis. */
export function speedScaleMax(s: TreadmillSnapshot): number {
  return s.limits?.speedMaxKmh ?? FALLBACK_SPEED_MAX;
}

/**
 * Écart à partir duquel la bande est vue en rampe. La vitesse réelle oscille de
 * 0,1 km/h autour de la consigne (29–30 pour 30) : en dessous, elle l'a atteinte.
 */
const SPEED_RAMP_MIN_GAP_KMH = 0.15;

/** La bande rejoint-elle sa consigne ? `null` si l'une des deux vitesses manque. */
export function speedTrend(s: TreadmillSnapshot): "up" | "down" | "steady" | null {
  if (s.speedKmh === null || s.targetSpeedKmh === null) return null;
  const gap = s.targetSpeedKmh - s.speedKmh;
  if (Math.abs(gap) < SPEED_RAMP_MIN_GAP_KMH) return "steady";
  return gap > 0 ? "up" : "down";
}

/** Bornes de consigne d'inclinaison annoncées par le tapis. */
export function inclineBounds(s: TreadmillSnapshot): { min: number; max: number } {
  return {
    min: s.limits?.inclineMinPct ?? FALLBACK_INCLINE_MIN,
    max: s.limits?.inclineMaxPct ?? FALLBACK_INCLINE_MAX,
  };
}

/** Consigne de vitesse bornée et arrondie, telle que le Rust l'appliquera. */
export function clampSpeed(s: TreadmillSnapshot, kmh: number): number {
  const { min, max } = speedBounds(s);
  return round1(clamp(round1(kmh), min, max));
}

/** Consigne d'inclinaison bornée et arrondie. */
export function clampIncline(s: TreadmillSnapshot, pct: number): number {
  const { min, max } = inclineBounds(s);
  return round1(clamp(round1(pct), min, max));
}

/**
 * Consigne de vitesse après un pas de `delta` km/h. On part de la consigne
 * courante (relue sur le tapis : la console a pu la changer), à défaut de la
 * vitesse réelle, à défaut du minimum.
 */
export function nextSpeed(s: TreadmillSnapshot, delta: number): number {
  const { min } = speedBounds(s);
  const base = s.targetSpeedKmh || s.speedKmh || min;
  return clampSpeed(s, base + delta);
}

/** Consigne d'inclinaison après un pas de `delta` %. */
export function nextIncline(s: TreadmillSnapshot, delta: number): number {
  const base = s.targetInclinePct ?? s.inclinePct ?? inclineBounds(s).min;
  return clampIncline(s, base + delta);
}

/** Un pas de vitesse changerait-il réellement la consigne ? */
export function speedStepUseful(s: TreadmillSnapshot, delta: number): boolean {
  return canAdjust(s) && !isStarting(s) && nextSpeed(s, delta) !== round1(s.targetSpeedKmh ?? -1);
}

/** Un pas d'inclinaison changerait-il réellement la consigne ? */
export function inclineStepUseful(s: TreadmillSnapshot, delta: number): boolean {
  return (
    canAdjust(s) && !isStarting(s) && nextIncline(s, delta) !== round1(s.targetInclinePct ?? -1)
  );
}

/** Un préréglage est proposé s'il ne dépasse pas le plafond ni la vitesse max. */
export function presetAllowed(s: TreadmillSnapshot, kmh: number): boolean {
  const { min, max } = speedBounds(s);
  return canAdjust(s) && !isStarting(s) && kmh >= min - 1e-6 && kmh <= max + 1e-6;
}

/** Plafond de vitesse borné à 1–20 km/h, au dixième. */
export function clampCap(kmh: number): number {
  return round1(clamp(round1(kmh), CAP_MIN_KMH, CAP_MAX_KMH));
}

// ── Raccourcis clavier ─────────────────────────────────────────────────────

export type KeyAction = "stop" | "speedUp" | "speedDown";

/**
 * Action associée à une touche. Échap et Espace arrêtent TOUJOURS, même depuis
 * un champ de saisie (Espace ne doit jamais activer un bouton qui aurait le
 * focus, « Démarrer » par exemple). Les flèches sont laissées aux champs de
 * saisie quand l'un d'eux a le focus.
 */
export function keyAction(key: string, inEditableField: boolean): KeyAction | null {
  if (key === "Escape" || key === " " || key === "Spacebar") return "stop";
  if (inEditableField) return null;
  if (key === "ArrowUp") return "speedUp";
  if (key === "ArrowDown") return "speedDown";
  return null;
}

// ── Lissage de la distance ─────────────────────────────────────────────────
// Le tapis fait avancer sa distance par à-coups (figée quelques secondes puis
// un bond d'une quarantaine de mètres). On l'extrapole donc à partir de la
// vitesse réelle, sans jamais reculer ni s'écarter de plus de `MAX_LEAD_M` de
// la dernière valeur lue.

export const MAX_LEAD_M = 40;

export interface DistanceSmoothing {
  /** Distance affichée, en mètres (non entière). */
  shown: number;
  /** Dernière distance lue sur le tapis. */
  reported: number;
}

export function smoothDistance(
  prev: DistanceSmoothing | null,
  reported: number | null,
  speedKmh: number | null,
  dtS: number,
): DistanceSmoothing | null {
  if (reported === null) return null;
  // Première valeur, ou compteur remis à zéro par le tapis : on repart de la mesure.
  if (prev === null || reported < prev.reported) return { shown: reported, reported };
  const estimate = prev.shown + (Math.max(0, speedKmh ?? 0) / 3.6) * Math.max(0, dtS);
  // `prev.shown` ne dépasse jamais `prev.reported + MAX_LEAD_M` ≤ `reported + MAX_LEAD_M` :
  // le bornage ci-dessous ne peut donc pas faire reculer l'affichage.
  return { shown: clamp(estimate, reported, reported + MAX_LEAD_M), reported };
}
