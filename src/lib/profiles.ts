// Profils de course, logique pure (testée dans profiles.test.ts) :
// bornes et validation (miroir de `treadmill/profile.rs`, pour signaler une
// erreur dans l'éditeur avant d'enregistrer), résumé d'un profil, et
// déroulement (`ProfileRunner`, utilisé seulement par le tapis simulé du
// navigateur : dans l'application, c'est le Rust qui déroule le profil).

import { decimal } from "./format";
import type { ProfileProgress, ProfileSegment, RunProfile } from "./types";

// Mêmes valeurs que dans profile.rs.
export const SEGMENT_MIN_M = 50;
export const SEGMENT_MAX_M = 10_000;
export const SPEED_MIN_KMH = 0.8;
export const SPEED_MAX_KMH = 20;
export const INCLINE_MIN_PCT = 0;
export const INCLINE_MAX_PCT = 15;
export const MAX_SEGMENTS = 60;
export const NAME_MAX_CHARS = 40;
export const RECOVERY_SPEED_KMH = 3;
export const RECOVERY_INCLINE_PCT = 0;

/** Pas des boutons ± de l'éditeur. */
export const DISTANCE_STEP_M = 50;
export const SPEED_STEP_KMH = 0.1;
export const INCLINE_STEP_PCT = 0.5;

/** Segment ajouté à un profil vide. */
const FIRST_SEGMENT: ProfileSegment = { distanceM: 300, speedKmh: 4, inclinePct: 0 };

const round1 = (x: number) => Math.round(x * 10) / 10;

/** Nombre décimal saisi à la française (« 4,5 » ou « 4.5 ») ; `null` si invalide. */
export function parseDecimal(text: string): number | null {
  const t = text.trim().replace(",", ".");
  if (!/^-?\d+(\.\d*)?$|^-?\.\d+$/.test(t)) return null;
  const n = Number(t);
  return Number.isFinite(n) ? n : null;
}

// ── Validation ─────────────────────────────────────────────────────────────

export interface SegmentErrors {
  distance?: string;
  speed?: string;
  incline?: string;
}

export function segmentErrors(s: ProfileSegment): SegmentErrors {
  const e: SegmentErrors = {};
  if (!Number.isInteger(s.distanceM) || s.distanceM < SEGMENT_MIN_M || s.distanceM > SEGMENT_MAX_M) {
    e.distance = `Entre ${SEGMENT_MIN_M} et ${SEGMENT_MAX_M} m`;
  }
  const v = round1(s.speedKmh);
  if (!Number.isFinite(v) || v < SPEED_MIN_KMH || v > SPEED_MAX_KMH) {
    e.speed = `Entre ${decimal(SPEED_MIN_KMH, 1)} et ${decimal(SPEED_MAX_KMH, 1)} km/h`;
  }
  const i = round1(s.inclinePct);
  if (!Number.isFinite(i) || i < INCLINE_MIN_PCT || i > INCLINE_MAX_PCT) {
    e.incline = `Entre ${decimal(INCLINE_MIN_PCT, 1)} et ${decimal(INCLINE_MAX_PCT, 1)} %`;
  }
  return e;
}

/** Première erreur du profil (mêmes règles que le Rust), `null` s'il est valide. */
export function profileError(p: Pick<RunProfile, "name" | "segments">): string | null {
  const name = p.name.trim();
  if (!name) return "Donnez un nom au profil.";
  if ([...name].length > NAME_MAX_CHARS) return `Nom trop long (${NAME_MAX_CHARS} caractères au plus).`;
  if (p.segments.length === 0) return "Le profil doit avoir au moins un segment.";
  if (p.segments.length > MAX_SEGMENTS) return `${MAX_SEGMENTS} segments au plus.`;
  for (const [i, s] of p.segments.entries()) {
    const e = segmentErrors(s);
    const first = e.distance ?? e.speed ?? e.incline;
    if (first) return `Segment ${i + 1} : ${first.charAt(0).toLowerCase()}${first.slice(1)}.`;
  }
  return null;
}

// ── Édition ────────────────────────────────────────────────────────────────

/** Segment suivant proposé : la copie du dernier (on change souvent une seule valeur). */
export function nextSegment(segments: ProfileSegment[]): ProfileSegment {
  const last = segments[segments.length - 1];
  return last ? { ...last } : { ...FIRST_SEGMENT };
}

export function clampDistance(m: number): number {
  return Math.min(SEGMENT_MAX_M, Math.max(SEGMENT_MIN_M, Math.round(m)));
}

export function clampSpeed(kmh: number): number {
  return round1(Math.min(SPEED_MAX_KMH, Math.max(SPEED_MIN_KMH, kmh)));
}

export function clampIncline(pct: number): number {
  return round1(Math.min(INCLINE_MAX_PCT, Math.max(INCLINE_MIN_PCT, pct)));
}

/** Copie profonde (le profil édité ne doit jamais partager ses segments). */
export function cloneProfile(p: RunProfile): RunProfile {
  return { ...p, segments: p.segments.map((s) => ({ ...s })) };
}

/** Deux profils ont-ils le même contenu (nom et segments) ? */
export function sameContent(a: RunProfile, b: RunProfile): boolean {
  return (
    a.name.trim() === b.name.trim() &&
    a.segments.length === b.segments.length &&
    a.segments.every(
      (s, i) =>
        s.distanceM === b.segments[i].distanceM &&
        round1(s.speedKmh) === round1(b.segments[i].speedKmh) &&
        round1(s.inclinePct) === round1(b.segments[i].inclinePct),
    )
  );
}

/** Nom libre pour une copie : « Collines (copie) », « Collines (copie 2) »… */
export function copyName(name: string, taken: string[]): string {
  const base = `${name.trim()} (copie`;
  for (let n = 1; ; n++) {
    const candidate = n === 1 ? `${base})` : `${base} ${n})`;
    if (!taken.includes(candidate)) return candidate.slice(0, NAME_MAX_CHARS);
  }
}

// ── Résumé ─────────────────────────────────────────────────────────────────

export interface ProfileSummary {
  totalM: number;
  /** Durée estimée (s) aux vitesses du profil, rampes ignorées. */
  durationS: number;
  /** Dénivelé positif cumulé (m). */
  gainM: number;
  maxSpeedKmh: number;
  maxInclinePct: number;
}

export function summarize(segments: ProfileSegment[]): ProfileSummary {
  let totalM = 0;
  let durationS = 0;
  let gainM = 0;
  let maxSpeedKmh = 0;
  let maxInclinePct = 0;
  for (const s of segments) {
    const d = Math.max(0, s.distanceM || 0);
    totalM += d;
    if (s.speedKmh > 0) durationS += d / (s.speedKmh / 3.6);
    gainM += (d * Math.max(0, s.inclinePct || 0)) / 100;
    maxSpeedKmh = Math.max(maxSpeedKmh, s.speedKmh || 0);
    maxInclinePct = Math.max(maxInclinePct, s.inclinePct || 0);
  }
  return { totalM, durationS, gainM, maxSpeedKmh, maxInclinePct };
}

/** Durée approximative : « ≈ 24 min », « ≈ 1 h 05 ». */
export function formatDuration(seconds: number): string {
  const min = Math.max(0, Math.round(seconds / 60));
  if (min < 60) return `≈ ${min} min`;
  return `≈ ${Math.floor(min / 60)} h ${String(min % 60).padStart(2, "0")}`;
}

/** Distance d'un profil : « 850 m », « 2,2 km ». */
export function formatProfileDistance(m: number): string {
  return m < 1000 ? `${Math.round(m)} m` : `${decimal(m / 1000, 1)} km`;
}

// ── Déroulement (tapis simulé du navigateur seulement) ─────────────────────

/**
 * Miroir simplifié de `ProfileRun` (profile.rs) : progression par somme des
 * avancées du compteur, segment en cours, récupération, réglage manuel. Pas de
 * renvoi des consignes : le tapis simulé ne perd pas d'écriture.
 */
export class ProfileRunner {
  private doneM = 0;
  private lastDistance: number | null = null;
  private applied: number | null = null;
  private manual = false;

  constructor(public profile: RunProfile) {}

  /** Profil modifié (bande arrêtée) : progression gardée, consignes à renvoyer. */
  replace(profile: RunProfile): void {
    this.profile = profile;
    this.applied = null;
  }

  totalM(): number {
    return this.profile.segments.reduce((sum, s) => sum + s.distanceM, 0);
  }

  index(): number {
    let end = 0;
    for (const [i, s] of this.profile.segments.entries()) {
      end += s.distanceM;
      if (this.doneM < end) return i;
    }
    return this.profile.segments.length;
  }

  target(idx: number): ProfileSegment {
    const s = this.profile.segments[idx];
    if (s) return { ...s };
    const last = this.profile.segments[this.profile.segments.length - 1];
    return {
      distanceM: 0,
      speedKmh: Math.min(RECOVERY_SPEED_KMH, last?.speedKmh ?? RECOVERY_SPEED_KMH),
      inclinePct: RECOVERY_INCLINE_PCT,
    };
  }

  onDistance(d: number): void {
    if (this.lastDistance !== null) {
      if (d >= this.lastDistance) this.doneM += d - this.lastDistance;
      else this.restart();
    }
    this.lastDistance = d;
  }

  onStopped(): void {
    this.applied = null;
    this.manual = false;
  }

  onManual(): void {
    this.manual = true;
  }

  restart(): void {
    this.doneM = 0;
    this.lastDistance = null;
    this.applied = null;
    this.manual = false;
  }

  /** Consignes à appliquer maintenant (changement de segment), sinon `null`. */
  step(): ProfileSegment | null {
    const idx = this.index();
    if (this.applied === idx) return null;
    this.applied = idx;
    this.manual = false;
    return this.target(idx);
  }

  progress(): ProfileProgress {
    const idx = this.index();
    const count = this.profile.segments.length;
    const end = this.profile.segments.slice(0, idx + 1).reduce((sum, s) => sum + s.distanceM, 0);
    const next = this.profile.segments[idx + 1];
    return {
      profileId: this.profile.id,
      profileName: this.profile.name,
      segmentCount: count,
      segmentIndex: idx,
      doneM: this.doneM,
      totalM: this.totalM(),
      segmentRemainingM: idx < count ? Math.max(0, end - this.doneM) : 0,
      current: this.target(idx),
      next: next ? { ...next } : null,
      finished: idx >= count,
      manual: this.manual,
    };
  }
}

/** Profils de départ : copie de `starter_profiles()` (profile.rs), pour le tapis simulé. */
export function starterProfiles(): RunProfile[] {
  const seg = (distanceM: number, speedKmh: number, inclinePct: number): ProfileSegment => ({
    distanceM,
    speedKmh,
    inclinePct,
  });
  const fractionne = [seg(400, 4, 0)];
  for (let i = 0; i < 4; i++) fractionne.push(seg(200, 6, 0), seg(200, 4, 0));
  fractionne.push(seg(300, 3.5, 0));
  return [
    {
      id: "depart-marche-douce",
      name: "Marche douce",
      builtin: true,
      segments: [seg(300, 3, 0), seg(500, 4, 0), seg(700, 4.5, 1), seg(300, 3.5, 0)],
    },
    {
      id: "depart-collines",
      name: "Collines",
      builtin: true,
      segments: [
        seg(300, 4, 0),
        seg(300, 4.5, 3),
        seg(200, 4.5, 0),
        seg(300, 4.5, 5),
        seg(200, 4.5, 0),
        seg(300, 4, 7),
        seg(400, 3.5, 0),
      ],
    },
    { id: "depart-fractionne", name: "Fractionné", builtin: true, segments: fractionne },
  ];
}
