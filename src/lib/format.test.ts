import { describe, expect, it } from "vitest";
import {
  NONE,
  decimal,
  formatDistance,
  formatElapsed,
  formatEnergy,
  formatPulse,
  formatTenths,
} from "./format";

describe("decimal", () => {
  it("utilise la virgule et arrondit correctement", () => {
    expect(decimal(3.5, 1)).toBe("3,5");
    expect(decimal(1.005, 2)).toBe("1,01");
    expect(decimal(0.1 + 0.2, 1)).toBe("0,3");
    expect(decimal(6, 1)).toBe("6,0");
  });
});

describe("formatElapsed", () => {
  it("affiche mm:ss sous une heure", () => {
    expect(formatElapsed(0)).toBe("00:00");
    expect(formatElapsed(220)).toBe("03:40");
    expect(formatElapsed(3599)).toBe("59:59");
  });

  it("affiche h:mm:ss à partir d'une heure", () => {
    expect(formatElapsed(3600)).toBe("1:00:00");
    expect(formatElapsed(3600 + 5 * 60 + 7)).toBe("1:05:07");
  });

  it("ignore les fractions et les valeurs négatives", () => {
    expect(formatElapsed(59.9)).toBe("00:59");
    expect(formatElapsed(-4)).toBe("00:00");
  });
});

describe("formatDistance", () => {
  it("affiche les mètres sous 1 km", () => {
    expect(formatDistance(0)).toEqual({ value: "0", unit: "m" });
    expect(formatDistance(182)).toEqual({ value: "182", unit: "m" });
    expect(formatDistance(999.7)).toEqual({ value: "999", unit: "m" });
  });

  it("affiche les km à 2 décimales, tronqués", () => {
    expect(formatDistance(1000)).toEqual({ value: "1,00", unit: "km" });
    expect(formatDistance(1234)).toEqual({ value: "1,23", unit: "km" });
    expect(formatDistance(1996)).toEqual({ value: "1,99", unit: "km" });
    expect(formatDistance(12050)).toEqual({ value: "12,05", unit: "km" });
  });

  it("affiche un tiret si inconnue", () => {
    expect(formatDistance(null)).toEqual({ value: NONE, unit: "" });
  });
});

describe("formatTenths / formatEnergy / formatPulse", () => {
  it("formate vitesse et inclinaison au dixième", () => {
    expect(formatTenths(2.9)).toBe("2,9");
    expect(formatTenths(0)).toBe("0,0");
    expect(formatTenths(null)).toBe(NONE);
  });

  it("formate l'énergie en kcal entières, avec ≈", () => {
    expect(formatEnergy(10.5)).toBe("≈ 11");
    expect(formatEnergy(0)).toBe("≈ 0");
    expect(formatEnergy(null)).toBe(NONE);
  });

  it("masque le pouls sans contact", () => {
    expect(formatPulse(89)).toBe("89");
    expect(formatPulse(null)).toBe(NONE);
    expect(formatPulse(0)).toBe(NONE);
  });
});
