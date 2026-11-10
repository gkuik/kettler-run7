import { describe, expect, it } from "vitest";
import { segmentColors } from "./routeColors";
import type { ProfileSegment } from "./types";

const seg = (speedKmh: number, inclinePct: number): ProfileSegment => ({ distanceM: 300, speedKmh, inclinePct });

describe("couleurs des segments sur la carte", () => {
  it("une seule valeur : teinte du milieu", () => {
    const c = segmentColors([seg(4, 0), seg(4, 2)], "speed");
    expect(c.steps).toEqual([3, 3]);
    expect(c.legend).toEqual([{ step: 3, label: "4,0 km/h" }]);
  });

  it("cinq valeurs ou moins : une teinte par valeur, de la plus claire à la plus foncée", () => {
    const c = segmentColors([seg(4, 3), seg(4, 0), seg(4, 6.5), seg(4, 0)], "incline");
    expect(c.steps).toEqual([3, 1, 5, 1]);
    expect(c.legend.map((l) => l.label)).toEqual(["0,0 %", "3,0 %", "6,5 %"]);
    expect(segmentColors([seg(3, 0), seg(6, 0)], "speed").steps).toEqual([1, 5]);
  });

  it("plus de cinq valeurs : cinq intervalles égaux, légende des valeurs présentes", () => {
    const inclines = [0, 0.5, 1, 2, 3, 4, 5, 10];
    const c = segmentColors(inclines.map((i) => seg(4, i)), "incline");
    // Intervalles de 2 % : [0,2) [2,4) [4,6) [6,8) [8,10].
    expect(c.steps).toEqual([1, 1, 1, 2, 2, 3, 3, 5]);
    expect(c.legend).toEqual([
      { step: 1, label: "0,0 – 1,0 %" },
      { step: 2, label: "2,0 – 3,0 %" },
      { step: 3, label: "4,0 – 5,0 %" },
      { step: 5, label: "10,0 %" },
    ]);
  });

  it("valeur en cours de saisie : pas de teinte, ignorée par la légende", () => {
    const c = segmentColors([seg(Number.NaN, 0), seg(5, 0)], "speed");
    expect(c.steps).toEqual([null, 3]);
    expect(c.legend).toHaveLength(1);
    expect(segmentColors([seg(Number.NaN, 0)], "speed")).toEqual({ steps: [null], legend: [] });
  });

  it("valeurs arrondies au dixième avant le classement", () => {
    expect(segmentColors([seg(4.04, 0), seg(4, 0)], "speed").legend).toHaveLength(1);
  });
});
