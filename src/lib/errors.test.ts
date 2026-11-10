import { describe, expect, it } from "vitest";
import { reconnectDetail } from "./errors";

describe("reconnectDetail", () => {
  it("rien à ajouter sans erreur ou pour la seule perte de liaison", () => {
    expect(reconnectDetail(null)).toBeNull();
    expect(reconnectDetail("")).toBeNull();
    expect(reconnectDetail("Liaison Bluetooth perdue")).toBeNull();
  });

  it("garde seulement la cause d'un essai raté", () => {
    const cause =
      "Tapis RUN 7 introuvable après 5 s de recherche : vérifiez qu'il est allumé, " +
      "à portée, et qu'aucune autre application n'y est connectée.";
    expect(
      reconnectDetail(`Liaison Bluetooth perdue, reconnexion en cours… (${cause})`),
    ).toBe(cause);
  });

  it("une cause entre parenthèses garde les siennes", () => {
    expect(
      reconnectDetail("Liaison Bluetooth perdue, reconnexion en cours… (erreur (code 5))"),
    ).toBe("erreur (code 5)");
  });

  it("toute autre erreur est montrée telle quelle", () => {
    expect(reconnectDetail("Le tapis ne répond plus")).toBe("Le tapis ne répond plus");
  });
});
