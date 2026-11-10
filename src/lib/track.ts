// Tracé d'un parcours (points [lat, lon]) : distance, allègement à l'import,
// et découpage selon les segments d'un profil pour la carte. Logique pure
// (testée dans track.test.ts).

import type { ProfileSegment, TrackPoint } from "./types";

const EARTH_RADIUS_M = 6_371_000;
const RAD = Math.PI / 180;

/** Distance entre deux points (m), formule de haversine. */
export function haversineM(lat1: number, lon1: number, lat2: number, lon2: number): number {
  const dLat = (lat2 - lat1) * RAD;
  const dLon = (lon2 - lon1) * RAD;
  const h = Math.sin(dLat / 2) ** 2 + Math.cos(lat1 * RAD) * Math.cos(lat2 * RAD) * Math.sin(dLon / 2) ** 2;
  return 2 * EARTH_RADIUS_M * Math.asin(Math.min(1, Math.sqrt(h)));
}

/** Distances cumulées le long du tracé (m), une par point. */
export function cumulativeM(track: TrackPoint[]): number[] {
  const out = [0];
  for (let i = 1; i < track.length; i++) {
    out.push(out[i - 1] + haversineM(track[i - 1][0], track[i - 1][1], track[i][0], track[i][1]));
  }
  return out;
}

// ── Allègement (Douglas-Peucker) ───────────────────────────────────────────

/** Tolérance de départ de l'allègement (m) : invisible sur la carte. */
const SIMPLIFY_TOLERANCE_M = 3;
/** Points au plus gardés à l'import (le Rust en accepte `TRACK_MAX_POINTS`). */
export const IMPORT_TRACK_MAX_POINTS = 1000;

/**
 * Supprime les points qui s'écartent de moins de `toleranceM` de la ligne
 * entre leurs voisins gardés (Douglas-Peucker, sans récursion : un tracé peut
 * avoir des dizaines de milliers de points). Calcul en mètres sur une
 * projection locale, largement assez précise à l'échelle d'un parcours.
 */
function douglasPeucker(track: TrackPoint[], toleranceM: number): TrackPoint[] {
  if (track.length <= 2) return track.slice();
  const lat0 = track[0][0] * RAD;
  const kx = EARTH_RADIUS_M * RAD * Math.cos(lat0);
  const ky = EARTH_RADIUS_M * RAD;
  const xy = track.map(([lat, lon]) => [lon * kx, lat * ky]);
  const keep = new Uint8Array(track.length);
  keep[0] = keep[track.length - 1] = 1;
  const stack: [number, number][] = [[0, track.length - 1]];
  while (stack.length > 0) {
    const [a, b] = stack.pop()!;
    const [ax, ay] = xy[a];
    const [bx, by] = xy[b];
    const dx = bx - ax;
    const dy = by - ay;
    const len2 = dx * dx + dy * dy;
    let worst = -1;
    let worstD = toleranceM;
    for (let i = a + 1; i < b; i++) {
      const [px, py] = xy[i];
      // Distance au segment [a, b] (au point a si a et b sont confondus).
      const t = len2 === 0 ? 0 : Math.max(0, Math.min(1, ((px - ax) * dx + (py - ay) * dy) / len2));
      const d = Math.hypot(px - (ax + t * dx), py - (ay + t * dy));
      if (d > worstD) {
        worstD = d;
        worst = i;
      }
    }
    if (worst >= 0) {
      keep[worst] = 1;
      stack.push([a, worst], [worst, b]);
    }
  }
  return track.filter((_, i) => keep[i] === 1);
}

/** Tracé allégé à `maxPoints` points au plus (tolérance doublée tant qu'il le faut), arrondi au 1e-6 degré (~0,1 m). */
export function simplifyTrack(track: TrackPoint[], maxPoints = IMPORT_TRACK_MAX_POINTS): TrackPoint[] {
  let tolerance = SIMPLIFY_TOLERANCE_M;
  let out = douglasPeucker(track, tolerance);
  while (out.length > maxPoints) {
    tolerance *= 2;
    out = douglasPeucker(out, tolerance);
  }
  const r = (x: number) => Math.round(x * 1e6) / 1e6;
  return out.map(([lat, lon]) => [r(lat), r(lon)]);
}

// ── Carte ──────────────────────────────────────────────────────────────────

/** Point du tracé à `atM` mètres du départ (interpolé). */
function pointAt(track: TrackPoint[], cum: number[], atM: number): TrackPoint {
  if (atM <= 0) return track[0];
  const last = track.length - 1;
  if (atM >= cum[last]) return track[last];
  let lo = 0;
  let hi = last;
  while (hi - lo > 1) {
    const mid = (lo + hi) >> 1;
    if (cum[mid] <= atM) lo = mid;
    else hi = mid;
  }
  const span = cum[hi] - cum[lo];
  const f = span === 0 ? 0 : (atM - cum[lo]) / span;
  return [track[lo][0] + f * (track[hi][0] - track[lo][0]), track[lo][1] + f * (track[hi][1] - track[lo][1])];
}

/**
 * Morceau du tracé couvert par chaque segment du profil. Les distances du
 * profil sont rapportées à la longueur du tracé (proportion) : elles restent
 * justes après un import, approchées si on les a modifiées dans l'éditeur.
 */
export function splitBySegments(track: TrackPoint[], segments: ProfileSegment[]): TrackPoint[][] {
  if (track.length < 2 || segments.length === 0) return segments.map(() => []);
  const cum = cumulativeM(track);
  const trackM = cum[cum.length - 1];
  const lengths = segments.map((s) => (Number.isFinite(s.distanceM) && s.distanceM > 0 ? s.distanceM : 0));
  const profileM = lengths.reduce((sum, d) => sum + d, 0);
  if (profileM === 0) return segments.map(() => []);
  const scale = trackM / profileM;
  const parts: TrackPoint[][] = [];
  let from = 0;
  let i = 0;
  for (const d of lengths) {
    const to = from + d * scale;
    const part: TrackPoint[] = [pointAt(track, cum, from)];
    while (i < track.length && cum[i] <= from) i++;
    for (let k = i; k < track.length && cum[k] < to; k++) part.push(track[k]);
    part.push(pointAt(track, cum, to));
    parts.push(d > 0 ? part : []);
    from = to;
  }
  return parts;
}
