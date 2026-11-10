import { describe, expect, it } from "vitest";
import { AnnounceBeeper, SegmentChangeBeeper } from "./beep";

describe("bip d'annonce du segment suivant", () => {
  it("bipe une fois au début de l'annonce, puis à celle du segment suivant", () => {
    const b = new AnnounceBeeper();
    expect(b.update("p#0", false, true)).toBe(false);
    expect(b.update("p#0", true, true)).toBe(true);
    expect(b.update("p#0", true, true)).toBe(false);
    expect(b.update("p#1", false, true)).toBe(false);
    expect(b.update("p#1", true, true)).toBe(true);
  });

  it("ne rebipe pas après une pause dans la zone d'annonce", () => {
    const b = new AnnounceBeeper();
    expect(b.update("p#2", true, true)).toBe(true);
    // Pause : la bande s'arrête, l'annonce disparaît, la progression reste.
    expect(b.update("p#2", false, true)).toBe(false);
    expect(b.update("p#2", true, true)).toBe(false);
  });

  it("rebipe quand le profil est reparti du début", () => {
    const b = new AnnounceBeeper();
    expect(b.update("p#0", true, true)).toBe(true);
    // Fin de séance : progression remise à zéro, profil pas encore démarré.
    expect(b.update("p#0", false, false)).toBe(false);
    expect(b.update("p#0", true, true)).toBe(true);
  });

  it("ne bipe pas sans profil", () => {
    const b = new AnnounceBeeper();
    expect(b.update(null, true, true)).toBe(false);
  });
});

describe("son du changement de segment effectif", () => {
  it("sonne à chaque passage au segment suivant, et à la récupération", () => {
    const b = new SegmentChangeBeeper();
    expect(b.update("p", 0, true)).toBe(false);
    expect(b.update("p", 0, true)).toBe(false);
    expect(b.update("p", 1, true)).toBe(true);
    expect(b.update("p", 1, true)).toBe(false);
    // Fin du dernier segment : segmentIndex = segmentCount (récupération).
    expect(b.update("p", 2, true)).toBe(true);
  });

  it("ne sonne pas au premier état vu, appli ouverte en pleine séance", () => {
    const b = new SegmentChangeBeeper();
    expect(b.update("p", 3, true)).toBe(false);
  });

  it("ne sonne pas quand le profil revient au début", () => {
    const b = new SegmentChangeBeeper();
    b.update("p", 2, true);
    expect(b.update("p", 0, false)).toBe(false);
    expect(b.update("p", 0, true)).toBe(false);
    expect(b.update("p", 1, true)).toBe(true);
  });

  it("ne sonne pas en changeant de profil", () => {
    const b = new SegmentChangeBeeper();
    b.update("p", 0, true);
    expect(b.update("q", 1, true)).toBe(false);
  });
});
