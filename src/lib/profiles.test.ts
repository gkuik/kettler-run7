import { describe, expect, it } from "vitest";
import {
  ProfileRunner,
  copyName,
  formatDuration,
  formatProfileDistance,
  nextSegment,
  parseDecimal,
  profileError,
  sameContent,
  segmentErrors,
  starterProfiles,
  summarize,
} from "./profiles";
import type { ProfileSegment, RunProfile } from "./types";

const seg = (distanceM: number, speedKmh: number, inclinePct: number): ProfileSegment => ({
  distanceM,
  speedKmh,
  inclinePct,
});

const profile = (segments: ProfileSegment[], name = "Test"): RunProfile => ({
  id: "t",
  name,
  builtin: false,
  segments,
});

describe("saisie", () => {
  it("accepte la virgule et le point", () => {
    expect(parseDecimal("4,5")).toBe(4.5);
    expect(parseDecimal(" 4.5 ")).toBe(4.5);
    expect(parseDecimal("300")).toBe(300);
    expect(parseDecimal("")).toBeNull();
    expect(parseDecimal("4,5,1")).toBeNull();
    expect(parseDecimal("abc")).toBeNull();
  });
});

describe("validation (mêmes règles que le Rust)", () => {
  it("erreurs par champ", () => {
    expect(segmentErrors(seg(300, 4, 0))).toEqual({});
    expect(segmentErrors(seg(20, 4, 0)).distance).toMatch(/50 et 10000/);
    expect(segmentErrors(seg(300.5, 4, 0)).distance).toBeDefined();
    expect(segmentErrors(seg(300, 0.5, 0)).speed).toMatch(/0,8 et 20,0/);
    expect(segmentErrors(seg(300, Number.NaN, 0)).speed).toBeDefined();
    expect(segmentErrors(seg(300, 4, 16)).incline).toBeDefined();
  });

  it("première erreur du profil", () => {
    expect(profileError(profile([seg(300, 4, 0)]))).toBeNull();
    expect(profileError(profile([seg(300, 4, 0)], "  "))).toMatch(/nom/);
    expect(profileError(profile([]))).toMatch(/au moins un segment/);
    expect(profileError(profile([seg(300, 4, 0), seg(300, 30, 0)]))).toBe(
      "Segment 2 : entre 0,8 et 20,0 km/h.",
    );
  });

  it("les profils de départ sont valides (marche rapide : 6 km/h au plus)", () => {
    const all = starterProfiles();
    expect(all.map((p) => p.name)).toEqual(["Marche douce", "Collines", "Fractionné"]);
    for (const p of all) {
      expect(profileError(p)).toBeNull();
      expect(Math.max(...p.segments.map((s) => s.speedKmh))).toBeLessThanOrEqual(6);
    }
  });
});

describe("édition", () => {
  it("le segment suivant copie le dernier", () => {
    expect(nextSegment([seg(200, 5, 2)])).toEqual(seg(200, 5, 2));
    expect(nextSegment([])).toEqual(seg(300, 4, 0));
  });

  it("noms de copies", () => {
    expect(copyName("Collines", [])).toBe("Collines (copie)");
    expect(copyName("Collines", ["Collines (copie)"])).toBe("Collines (copie 2)");
  });

  it("compare le contenu", () => {
    const a = profile([seg(300, 4, 0)]);
    expect(sameContent(a, { ...a, name: " Test " })).toBe(true);
    expect(sameContent(a, profile([seg(300, 4.1, 0)]))).toBe(false);
  });
});

describe("résumé", () => {
  it("distance, durée, dénivelé", () => {
    const s = summarize([seg(600, 3.6, 0), seg(400, 4.8, 5)]);
    expect(s.totalM).toBe(1000);
    expect(s.durationS).toBeCloseTo(600 + 300);
    expect(s.gainM).toBe(20);
    expect(s.maxSpeedKmh).toBe(4.8);
    expect(formatDuration(s.durationS)).toBe("≈ 15 min");
    expect(formatDuration(3900)).toBe("≈ 1 h 05");
    expect(formatProfileDistance(850)).toBe("850 m");
    expect(formatProfileDistance(2200)).toBe("2,2 km");
  });
});

describe("ProfileRunner (tapis simulé du navigateur)", () => {
  it("segments, récupération, fin de séance", () => {
    const r = new ProfileRunner(profile([seg(100, 3, 1), seg(100, 5, 0)]));
    r.onDistance(40); // point de départ
    expect(r.step()).toEqual(seg(100, 3, 1));
    expect(r.step()).toBeNull();
    r.onDistance(140);
    expect(r.step()).toEqual(seg(100, 5, 0));
    r.onDistance(240);
    const p = r.progress();
    expect(p.finished).toBe(true);
    expect(r.step()).toEqual(seg(0, 3, 0));
    r.onDistance(0); // compteur remis à zéro : retour au début
    expect(r.progress().doneM).toBe(0);
  });

  it("réglage manuel jusqu'au segment suivant, consignes renvoyées après une pause", () => {
    const r = new ProfileRunner(profile([seg(100, 3, 1), seg(100, 5, 0)]));
    r.onDistance(0);
    r.step();
    r.onManual();
    expect(r.progress().manual).toBe(true);
    expect(r.step()).toBeNull();
    r.onStopped();
    expect(r.step()).toEqual(seg(100, 3, 1));
    expect(r.progress().manual).toBe(false);
  });
});
