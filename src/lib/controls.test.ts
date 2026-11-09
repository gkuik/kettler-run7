import { describe, expect, it } from "vitest";
import {
  MAX_LEAD_M,
  canAdjust,
  canStart,
  clampCap,
  clampIncline,
  clampSpeed,
  inclineStepUseful,
  isPaused,
  isStarting,
  keyAction,
  nextIncline,
  nextSpeed,
  presetAllowed,
  round1,
  smoothDistance,
  speedBounds,
  speedScaleMax,
  speedStepUseful,
  speedTrend,
  stopButtonMode,
} from "./controls";
import { emptySnapshot, type TreadmillSnapshot } from "./types";

/** Tapis connecté, bande en marche à 3,0 km/h, plafond 6,0. */
function running(over: Partial<TreadmillSnapshot> = {}): TreadmillSnapshot {
  return {
    ...emptySnapshot(),
    connection: "connected",
    running: true,
    speedKmh: 3,
    targetSpeedKmh: 3,
    inclinePct: 0,
    targetInclinePct: 0,
    distanceM: 0,
    energyKcal: 0,
    limits: { speedMinKmh: 0.8, speedMaxKmh: 20, inclineMinPct: 0, inclineMaxPct: 15 },
    device: { name: "RUN 7", id: "STB1T0158", article: "07883-600" },
    ...over,
  };
}

describe("round1", () => {
  it("arrondit au dixième sans artefact flottant", () => {
    expect(round1(0.1 + 0.2)).toBe(0.3);
    expect(round1(3.05)).toBe(3.1);
    expect(round1(2.94)).toBe(2.9);
  });
});

describe("permissions", () => {
  it("démarrer seulement connecté et bande arrêtée", () => {
    expect(canStart(running({ running: false }))).toBe(true);
    expect(canStart(running())).toBe(false);
    expect(canStart(running({ running: false, connection: "reconnecting" }))).toBe(false);
    expect(canStart(emptySnapshot())).toBe(false);
  });

  it("régler seulement connecté et bande en marche", () => {
    expect(canAdjust(running())).toBe(true);
    expect(canAdjust(running({ running: false }))).toBe(false);
    expect(canAdjust(running({ connection: "reconnecting" }))).toBe(false);
  });

  it("détecte le compte à rebours de démarrage", () => {
    expect(isStarting(running({ targetSpeedKmh: 0, speedKmh: 0 }))).toBe(true);
    expect(isStarting(running({ targetSpeedKmh: 0.8 }))).toBe(false);
    expect(isStarting(running({ running: false, targetSpeedKmh: 0 }))).toBe(false);
    expect(speedStepUseful(running({ targetSpeedKmh: 0 }), 0.1)).toBe(false);
  });

  it("en pause : bande arrêtée avec des compteurs gardés", () => {
    expect(isPaused(running({ running: false, distanceM: 120 }))).toBe(true);
    expect(isPaused(running({ running: false, elapsedS: 30 }))).toBe(true);
    // Compteurs à zéro : séance terminée (ou jamais commencée), pas une pause.
    expect(isPaused(running({ running: false }))).toBe(false);
    expect(isPaused(running({ distanceM: 120 }))).toBe(false);
    expect(isPaused(running({ running: false, distanceM: 120, connection: "reconnecting" }))).toBe(
      false,
    );
  });
});

describe("stopButtonMode", () => {
  const paused = () => running({ running: false, speedKmh: 0, targetSpeedKmh: 0, distanceM: 120, elapsedS: 95 });

  it("bande en marche : PAUSE", () => {
    expect(stopButtonMode(running())).toBe("pause");
  });

  it("compte à rebours de démarrage : PAUSE", () => {
    expect(stopButtonMode(running({ targetSpeedKmh: 0, speedKmh: 0 }))).toBe("pause");
  });

  it("pause envoyée, bande encore vue en marche : STOP tout de suite", () => {
    expect(stopButtonMode(running(), true)).toBe("stop");
  });

  it("séance en pause : STOP (terminer la séance)", () => {
    expect(stopButtonMode(paused())).toBe("stop");
    expect(stopButtonMode(paused(), true)).toBe("stop");
  });

  it("bande arrêtée sans séance : pas de bouton", () => {
    expect(stopButtonMode(running({ running: false, speedKmh: 0, targetSpeedKmh: 0 }))).toBeNull();
    expect(stopButtonMode(running({ running: false }), true)).toBeNull();
  });

  it("non connecté : pas de bouton", () => {
    expect(stopButtonMode(emptySnapshot())).toBeNull();
    expect(stopButtonMode({ ...emptySnapshot(), connection: "scanning" })).toBeNull();
    expect(stopButtonMode({ ...emptySnapshot(), connection: "connecting" })).toBeNull();
  });

  it("connecté mais Online pas encore relu : bouton affiché", () => {
    expect(stopButtonMode(running({ running: null }))).toBe("pause");
  });

  it("reconnexion bande en marche (lostWhileRunning) : PAUSE", () => {
    expect(stopButtonMode(running({ connection: "reconnecting", lostWhileRunning: true }))).toBe(
      "pause",
    );
    // Même si le dernier état relu dit « arrêtée » : la perte prime.
    expect(
      stopButtonMode(
        running({ connection: "reconnecting", lostWhileRunning: true, running: false }),
      ),
    ).toBe("pause");
  });

  it("reconnexion sans lostWhileRunning : bouton toujours affiché", () => {
    expect(stopButtonMode({ ...paused(), connection: "reconnecting" })).toBe("stop");
    expect(
      stopButtonMode(running({ connection: "reconnecting", running: false, distanceM: 0 })),
    ).toBe("stop");
    expect(stopButtonMode({ ...emptySnapshot(), connection: "reconnecting" })).toBe("pause");
  });
});

describe("bornage de la vitesse", () => {
  it("borne à [min, min(plafond, max)]", () => {
    const s = running();
    expect(speedBounds(s)).toEqual({ min: 0.8, max: 6 });
    expect(clampSpeed(s, 9)).toBe(6);
    expect(clampSpeed(s, 0.2)).toBe(0.8);
    expect(clampSpeed(s, 3.14)).toBe(3.1);
    expect(clampSpeed(running({ speedCapKmh: 25 }), 30)).toBe(20);
  });

  it("calcule la consigne suivante depuis la consigne relue", () => {
    expect(nextSpeed(running({ targetSpeedKmh: 3.5, speedKmh: 3.1 }), 0.1)).toBe(3.6);
    expect(nextSpeed(running({ targetSpeedKmh: 5.8 }), 0.5)).toBe(6);
    expect(nextSpeed(running({ targetSpeedKmh: 1 }), -0.5)).toBe(0.8);
    // Pas de consigne connue : on part de la vitesse réelle, sinon du minimum.
    expect(nextSpeed(running({ targetSpeedKmh: null, speedKmh: 2 }), 0.1)).toBe(2.1);
    expect(nextSpeed(running({ targetSpeedKmh: null, speedKmh: null }), 0.1)).toBe(0.9);
  });

  it("désactive un pas qui ne changerait rien", () => {
    expect(speedStepUseful(running({ targetSpeedKmh: 6 }), 0.1)).toBe(false);
    expect(speedStepUseful(running({ targetSpeedKmh: 6 }), -0.1)).toBe(true);
    expect(speedStepUseful(running({ targetSpeedKmh: 0.8 }), -0.5)).toBe(false);
    expect(speedStepUseful(running({ running: false }), 0.1)).toBe(false);
  });

  it("ne propose pas un préréglage au-dessus du plafond", () => {
    expect(presetAllowed(running(), 6)).toBe(true);
    expect(presetAllowed(running({ speedCapKmh: 5 }), 6)).toBe(false);
    expect(presetAllowed(running({ running: false }), 3)).toBe(false);
  });

  it("borne le plafond à 1–20 km/h", () => {
    expect(clampCap(0.5)).toBe(1);
    expect(clampCap(25)).toBe(20);
    expect(clampCap(6.04)).toBe(6);
  });
});

describe("bornage de l'inclinaison", () => {
  it("borne aux limites du tapis", () => {
    const s = running();
    expect(clampIncline(s, -1)).toBe(0);
    expect(clampIncline(s, 16)).toBe(15);
    expect(nextIncline(running({ targetInclinePct: 1 }), 0.5)).toBe(1.5);
    expect(inclineStepUseful(running({ targetInclinePct: 0 }), -0.5)).toBe(false);
    expect(inclineStepUseful(running({ targetInclinePct: 0 }), 0.5)).toBe(true);
  });
});

describe("keyAction", () => {
  it("Échap et Espace arrêtent toujours, même dans un champ", () => {
    expect(keyAction("Escape", false)).toBe("stop");
    expect(keyAction(" ", false)).toBe("stop");
    expect(keyAction("Escape", true)).toBe("stop");
    expect(keyAction(" ", true)).toBe("stop");
  });

  it("les flèches règlent la vitesse hors des champs", () => {
    expect(keyAction("ArrowUp", false)).toBe("speedUp");
    expect(keyAction("ArrowDown", false)).toBe("speedDown");
    expect(keyAction("ArrowUp", true)).toBeNull();
    expect(keyAction("a", false)).toBeNull();
  });
});

describe("smoothDistance", () => {
  it("part de la première mesure", () => {
    expect(smoothDistance(null, 100, 3.6, 1)).toEqual({ shown: 100, reported: 100 });
    expect(smoothDistance(null, null, 3.6, 1)).toBeNull();
  });

  it("extrapole avec la vitesse entre deux à-coups", () => {
    const s = smoothDistance({ shown: 100, reported: 100 }, 100, 3.6, 2);
    expect(s).toEqual({ shown: 102, reported: 100 });
  });

  it("ne s'écarte pas de plus de MAX_LEAD_M de la mesure", () => {
    const s = smoothDistance({ shown: 100, reported: 100 }, 100, 36, 60);
    expect(s?.shown).toBe(100 + MAX_LEAD_M);
  });

  it("rattrape un bond de la mesure sans jamais reculer", () => {
    const a = smoothDistance({ shown: 110, reported: 100 }, 140, 3.6, 0.5);
    expect(a?.shown).toBe(140);
    const b = smoothDistance({ shown: 130, reported: 100 }, 100, 0, 1);
    expect(b?.shown).toBe(130);
  });

  it("repart de la mesure quand le tapis remet le compteur à zéro", () => {
    expect(smoothDistance({ shown: 520, reported: 500 }, 0, 3.6, 0.5)).toEqual({
      shown: 0,
      reported: 0,
    });
  });
});

describe("speedScaleMax", () => {
  it("suit la vitesse maximale annoncée, sans tenir compte du plafond", () => {
    expect(speedScaleMax(running({ speedCapKmh: 6 }))).toBe(20);
    expect(
      speedScaleMax(
        running({ limits: { speedMinKmh: 0.8, speedMaxKmh: 16, inclineMinPct: 0, inclineMaxPct: 15 } }),
      ),
    ).toBe(16);
  });

  it("garde 20 km/h tant que le tapis n'a pas annoncé ses bornes", () => {
    expect(speedScaleMax(emptySnapshot())).toBe(20);
  });
});

describe("speedTrend", () => {
  it("ignore l'oscillation de 0,1 km/h autour de la consigne", () => {
    expect(speedTrend(running({ speedKmh: 2.9, targetSpeedKmh: 3 }))).toBe("steady");
    expect(speedTrend(running({ speedKmh: 3, targetSpeedKmh: 3 }))).toBe("steady");
  });

  it("voit la rampe dans les deux sens", () => {
    expect(speedTrend(running({ speedKmh: 2, targetSpeedKmh: 3 }))).toBe("up");
    expect(speedTrend(running({ speedKmh: 4, targetSpeedKmh: 3 }))).toBe("down");
  });

  it("ne dit rien si une vitesse manque", () => {
    expect(speedTrend(running({ targetSpeedKmh: null }))).toBeNull();
    expect(speedTrend(running({ speedKmh: null }))).toBeNull();
  });
});
