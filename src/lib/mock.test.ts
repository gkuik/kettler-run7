import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { MockTreadmill } from "./mock";
import type { TreadmillSnapshot } from "./types";

let tm: MockTreadmill;
let states: TreadmillSnapshot[];

beforeEach(() => {
  vi.useFakeTimers();
  tm = new MockTreadmill({ jitter: false, scanMs: 1000, connectMs: 500, tickMs: 500 });
  states = [];
  tm.onState((s) => states.push(s));
});

afterEach(() => {
  tm.dispose();
  vi.useRealTimers();
});

/** Avance le temps simulé. */
const advance = (s: number) => vi.advanceTimersByTimeAsync(s * 1000);

async function connect(): Promise<void> {
  const p = tm.connect();
  await advance(2);
  await p;
}

async function startAndWaitBelt(): Promise<void> {
  await tm.start();
  await advance(5.5);
}

describe("connexion", () => {
  it("passe par scanning puis connecting puis connected", async () => {
    await connect();
    const seen = [...new Set(states.map((s) => s.connection))];
    expect(seen).toEqual(["scanning", "connecting", "connected"]);
    const s = await tm.getState();
    expect(s.device).toEqual({ name: "RUN 7", id: "STB1T0158", article: "07883-600" });
    expect(s.limits?.speedMinKmh).toBe(0.8);
    expect(s.running).toBe(false);
  });

  it("disconnect annule une connexion en cours sans erreur", async () => {
    const p = tm.connect();
    await advance(0.5);
    await tm.disconnect();
    await advance(2);
    await expect(p).resolves.toBeUndefined();
    expect((await tm.getState()).connection).toBe("disconnected");
  });
});

describe("règles du contrat", () => {
  it("refuse start et stop hors connexion", async () => {
    await expect(tm.start()).rejects.toBe("Tapis non connecté");
    await expect(tm.stop()).rejects.toMatch(/^Arrêt NON envoyé.*stop de la console/);
    expect((await tm.getState()).lastError).toMatch(/Arrêt NON envoyé/);
  });

  it("refuse vitesse et inclinaison si la bande ne tourne pas", async () => {
    await connect();
    await expect(tm.setTargetSpeed(3)).rejects.toMatch(/ne tourne pas/);
    await expect(tm.setTargetIncline(1)).rejects.toMatch(/ne tourne pas/);
  });

  it("efface lastError au succès suivant", async () => {
    await connect();
    await tm.setTargetSpeed(3).catch(() => {});
    expect((await tm.getState()).lastError).not.toBeNull();
    await tm.start();
    expect((await tm.getState()).lastError).toBeNull();
  });

  it("démarre après un compte à rebours de ~5 s, à 0,8 km/h", async () => {
    await connect();
    await tm.start();
    await advance(3);
    let s = await tm.getState();
    expect(s.running).toBe(true);
    expect(s.speedKmh).toBe(0);
    // Écriture ignorée pendant le compte à rebours.
    await tm.setTargetSpeed(4);
    expect((await tm.getState()).targetSpeedKmh).toBe(0);
    await advance(2.5);
    s = await tm.getState();
    expect(s.targetSpeedKmh).toBe(0.8);
    expect(s.speedKmh).toBeCloseTo(0.8, 5);
  });

  it("borne la consigne à la vitesse max et suit une rampe de ~0,2 km/h/s", async () => {
    await connect();
    await startAndWaitBelt();
    await tm.setTargetSpeed(25);
    expect((await tm.getState()).targetSpeedKmh).toBe(20);
    await tm.setTargetSpeed(9);
    expect((await tm.getState()).targetSpeedKmh).toBe(9);
    await tm.setTargetSpeed(2.84);
    expect((await tm.getState()).targetSpeedKmh).toBe(2.8);
    await advance(5);
    expect((await tm.getState()).speedKmh).toBeCloseTo(1.8, 5);
    await advance(10);
    expect((await tm.getState()).speedKmh).toBeCloseTo(2.8, 5);
  });

  it("incline à ~0,3 %/s dans les limites du tapis", async () => {
    await connect();
    await startAndWaitBelt();
    await tm.setTargetIncline(20);
    expect((await tm.getState()).targetInclinePct).toBe(15);
    await tm.setTargetIncline(1.5);
    await advance(2);
    expect((await tm.getState()).inclinePct).toBeCloseTo(0.6, 5);
    await advance(5);
    expect((await tm.getState()).inclinePct).toBe(1.5);
  });

  it("compte temps, distance et énergie, puis s'arrête en décélérant", async () => {
    await connect();
    await startAndWaitBelt();
    await tm.setTargetSpeed(3.6);
    await advance(60);
    let s = await tm.getState();
    expect(s.elapsedS).toBeGreaterThanOrEqual(65);
    expect(s.distanceM).toBeGreaterThan(40);
    expect(s.energyKcal).toBeGreaterThan(0);

    await tm.stop();
    s = await tm.getState();
    expect(s.running).toBe(false);
    expect(s.targetSpeedKmh).toBe(0);
    const elapsed = s.elapsedS;
    await advance(10);
    s = await tm.getState();
    expect(s.speedKmh).toBe(0);
    expect(s.elapsedS).toBe(elapsed);

    // Second arrêt : compteurs remis à zéro.
    await tm.stop();
    s = await tm.getState();
    expect(s.distanceM).toBe(0);
    expect(s.elapsedS).toBe(0);
  });
});

describe("incidents", () => {
  it("perte de liaison : reconnecting + lostWhileRunning, la bande continue", async () => {
    await connect();
    await startAndWaitBelt();
    await tm.setTargetSpeed(3);
    tm.simulateLinkLoss(5);
    let s = await tm.getState();
    expect(s.connection).toBe("reconnecting");
    expect(s.lostWhileRunning).toBe(true);
    await advance(6.5);
    s = await tm.getState();
    expect(s.connection).toBe("connected");
    expect(s.lostWhileRunning).toBe(false);
    expect(s.running).toBe(true);
  });

  it("un stop demandé pendant la reconnexion part dès la liaison rétablie", async () => {
    await connect();
    await startAndWaitBelt();
    tm.simulateLinkLoss(3);
    // Comme le Rust : rejeté (rien n'est parti), mais mémorisé pour la reconnexion.
    await expect(tm.stop()).rejects.toMatch(/sera envoyé dès la reconnexion/);
    // La bande tourne toujours tant que la liaison manque.
    await advance(1);
    expect((await tm.getState()).running).toBe(true);
    await advance(4);
    const s = await tm.getState();
    expect(s.connection).toBe("connected");
    expect(s.running).toBe(false);
    expect(s.targetSpeedKmh).toBe(0);
  });

  it("un stop dont l'écriture échoue est rejeté avec un message", async () => {
    await connect();
    await startAndWaitBelt();
    tm.failNextStopWrite();
    await expect(tm.stop()).rejects.toMatch(/^Arrêt NON envoyé/);
    expect((await tm.getState()).running).toBe(true);
    await tm.stop();
    expect((await tm.getState()).running).toBe(false);
  });

  it("retrait de la clé : bande arrêtée, liaison coupée", async () => {
    await connect();
    await startAndWaitBelt();
    tm.pullSafetyKey();
    expect((await tm.getState()).connection).toBe("reconnecting");
    await advance(12.5);
    const s = await tm.getState();
    expect(s.connection).toBe("connected");
    expect(s.running).toBe(false);
    expect(s.speedKmh).toBe(0);
  });
});

describe("profils de course", () => {
  it("liste les profils de départ et enregistre un profil", async () => {
    const before = await tm.listProfiles();
    expect(before.filter((p) => p.builtin)).toHaveLength(3);
    const saved = await tm.saveProfile({
      id: "",
      name: " Matin ",
      builtin: false,
      segments: [{ distanceM: 100, speedKmh: 3.04, inclinePct: 0 }],
    });
    expect(saved.id).not.toBe("");
    expect(saved.name).toBe("Matin");
    expect(saved.segments[0].speedKmh).toBe(3);
    expect(await tm.listProfiles()).toHaveLength(4);
    await expect(tm.saveProfile({ ...before[0] })).rejects.toMatch(/lecture seule/);
    await expect(tm.deleteProfile(before[0].id)).rejects.toMatch(/départ/);
  });

  it("déroule le profil choisi pendant la séance", async () => {
    const saved = await tm.saveProfile({
      id: "",
      name: "Essai",
      builtin: false,
      segments: [
        { distanceM: 50, speedKmh: 4, inclinePct: 2 },
        { distanceM: 50, speedKmh: 5, inclinePct: 0 },
      ],
    });
    await tm.selectProfile(saved.id);
    expect((await tm.getState()).profile?.segmentIndex).toBe(0);
    await connect();
    await startAndWaitBelt();
    await advance(1);
    let s = await tm.getState();
    expect(s.targetSpeedKmh).toBe(4);
    expect(s.targetInclinePct).toBe(2);
    await expect(tm.selectProfile(null)).resolves.toBeUndefined();
    await tm.selectProfile(saved.id).catch(() => {});
    await tm.stop();
    await advance(10);
    await tm.selectProfile(saved.id);
    await tm.start();
    await advance(60);
    s = await tm.getState();
    expect(s.profile?.segmentIndex).toBeGreaterThanOrEqual(1);
    expect(s.targetSpeedKmh).toBe(5);
  });

  it("refuse de changer de profil bande en marche", async () => {
    const [a, b] = await tm.listProfiles();
    await tm.selectProfile(a.id);
    await connect();
    await startAndWaitBelt();
    await expect(tm.selectProfile(b.id)).rejects.toMatch(/mettez-la en pause/);
  });
});
