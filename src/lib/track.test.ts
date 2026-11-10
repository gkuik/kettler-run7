import { describe, expect, it } from "vitest";
import { IMPORT_TRACK_MAX_POINTS, cumulativeM, haversineM, simplifyTrack, splitBySegments } from "./track";
import type { ProfileSegment, TrackPoint } from "./types";

const M_PER_DEG = haversineM(0, 0, 1, 0);

/** Ligne droite vers le nord, un point tous les `stepM` mètres. */
function north(totalM: number, stepM = 10, lon = 3): TrackPoint[] {
  const pts: TrackPoint[] = [];
  for (let d = 0; d <= totalM + 1e-9; d += stepM) pts.push([50 + d / M_PER_DEG, lon]);
  return pts;
}

const seg = (distanceM: number): ProfileSegment => ({ distanceM, speedKmh: 4, inclinePct: 0 });
const last = <T>(a: T[]) => a[a.length - 1];

describe("allègement du tracé", () => {
  it("une ligne droite se réduit à ses deux bouts", () => {
    const t = north(2000);
    expect(simplifyTrack(t)).toEqual([
      [50, 3],
      [Math.round(last(t)[0] * 1e6) / 1e6, 3],
    ]);
  });

  it("garde les virages, ignore les écarts de moins de 3 m", () => {
    // Aller vers le nord 1 km, puis vers l'est 1 km ; bruit de ±1 m.
    const t: TrackPoint[] = [];
    for (let i = 0; i <= 100; i++) t.push([50 + (i * 10) / M_PER_DEG, 3 + ((i % 2) * 1) / (M_PER_DEG * Math.cos(50 * Math.PI / 180))]);
    const corner = last(t);
    const kx = M_PER_DEG * Math.cos(corner[0] * Math.PI / 180);
    for (let i = 1; i <= 100; i++) t.push([corner[0] + ((i % 2) * 1) / M_PER_DEG, corner[1] + (i * 10) / kx]);
    const s = simplifyTrack(t);
    expect(s.length).toBeLessThanOrEqual(4);
    expect(s.length).toBeGreaterThanOrEqual(3);
    const c = s.find((p) => Math.abs(p[0] - corner[0]) * M_PER_DEG < 5 && Math.abs(p[1] - corner[1]) * kx < 15);
    expect(c).toBeDefined();
  });

  it("jamais plus de 1000 points, même pour un tracé très sinueux", () => {
    const t: TrackPoint[] = [];
    for (let i = 0; i < 20_000; i++) t.push([50 + i * 1e-5, 3 + Math.sin(i / 3) * 1e-3]);
    const s = simplifyTrack(t);
    expect(s.length).toBeLessThanOrEqual(IMPORT_TRACK_MAX_POINTS);
    expect(s[0]).toEqual([50, 3]);
    // Les décimales sont limitées à 6.
    for (const [lat, lon] of s) {
      expect(Math.round(lat * 1e6) / 1e6).toBe(lat);
      expect(Math.round(lon * 1e6) / 1e6).toBe(lon);
    }
  });

  it("deux points ou moins : inchangé", () => {
    expect(simplifyTrack([[1, 2]])).toEqual([[1, 2]]);
  });
});

describe("découpage selon les segments", () => {
  it("chaque segment couvre sa part du tracé, bouts à bouts", () => {
    const t = north(1000);
    const parts = splitBySegments(t, [seg(200), seg(500), seg(300)]);
    expect(parts).toHaveLength(3);
    const lengths = parts.map((p) => last(cumulativeM(p)));
    expect(lengths[0]).toBeCloseTo(200, 0);
    expect(lengths[1]).toBeCloseTo(500, 0);
    expect(lengths[2]).toBeCloseTo(300, 0);
    // Continuité : chaque morceau commence où le précédent finit.
    expect(parts[1][0]).toEqual(last(parts[0]));
    expect(parts[2][0]).toEqual(last(parts[1]));
    expect(parts[0][0]).toEqual(t[0]);
    expect(last(parts[2])).toEqual(last(t));
  });

  it("distances modifiées dans l'éditeur : réparties à la proportion", () => {
    const parts = splitBySegments(north(1000), [seg(100), seg(100)]);
    expect(last(cumulativeM(parts[0]))).toBeCloseTo(500, 0);
    expect(last(cumulativeM(parts[1]))).toBeCloseTo(500, 0);
  });

  it("segment de distance invalide (saisie en cours) : morceau vide", () => {
    const parts = splitBySegments(north(1000), [seg(400), seg(Number.NaN), seg(600)]);
    expect(parts[1]).toEqual([]);
    expect(last(cumulativeM(parts[2]))).toBeCloseTo(600, 0);
  });

  it("tracé trop court : aucun morceau", () => {
    expect(splitBySegments([[50, 3]], [seg(300)])).toEqual([[]]);
  });
});
