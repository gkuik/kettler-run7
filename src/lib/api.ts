import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { MockTreadmill } from "./mock";
import { EVENT_STATE, type TreadmillSnapshot } from "./types";

// ── Point d'accès UNIQUE au backend Rust ──────────────────────────────────
// Aucun composant n'appelle `invoke` directement : tout passe par ici.
//
// Hors Tauri (navigateur, `npx vite`), les mêmes appels sont servis par un tapis
// simulé (`mock.ts`), pour développer l'interface sans le RUN 7.

/** Vrai dans l'application native (Tauri injecte `__TAURI_INTERNALS__`). */
export const inTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

/** Vrai quand l'interface pilote le tapis simulé. */
export const simulated = !inTauri;

let mockInstance: MockTreadmill | null = null;

/** Le tapis simulé (créé à la demande, jamais dans Tauri). */
export function mock(): MockTreadmill {
  if (!mockInstance) mockInstance = new MockTreadmill();
  return mockInstance;
}

/** Le Rust renvoie ses erreurs sous forme de `String` ; on normalise le reste. */
export function errorMessage(e: unknown): string {
  if (typeof e === "string") return e;
  if (e instanceof Error) return e.message;
  if (e && typeof e === "object" && "message" in e) return String((e as { message: unknown }).message);
  return "Erreur inconnue";
}

async function call<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(cmd, args);
  } catch (e) {
    throw errorMessage(e);
  }
}

/** Les 8 commandes du contrat (CLAUDE.md, « Contrat frontend ↔ Rust »). */
export const api = {
  getState: (): Promise<TreadmillSnapshot> =>
    inTauri ? call<TreadmillSnapshot>("get_state") : mock().getState(),
  connect: (): Promise<void> => (inTauri ? call<void>("connect") : mock().connect()),
  disconnect: (): Promise<void> => (inTauri ? call<void>("disconnect") : mock().disconnect()),
  start: (): Promise<void> => (inTauri ? call<void>("start") : mock().start()),
  /** Prioritaire côté Rust : part avant toute écriture en attente. */
  stop: (): Promise<void> => (inTauri ? call<void>("stop") : mock().stop()),
  setTargetSpeed: (kmh: number): Promise<void> =>
    inTauri ? call<void>("set_target_speed", { kmh }) : mock().setTargetSpeed(kmh),
  setTargetIncline: (pct: number): Promise<void> =>
    inTauri ? call<void>("set_target_incline", { pct }) : mock().setTargetIncline(pct),
  setSpeedCap: (kmh: number): Promise<void> =>
    inTauri ? call<void>("set_speed_cap", { kmh }) : mock().setSpeedCap(kmh),

  /** Abonnement à `treadmill-state` ; renvoie la fonction de désabonnement. */
  onState: async (cb: (s: TreadmillSnapshot) => void): Promise<() => void> => {
    if (!inTauri) return mock().onState(cb);
    return listen<TreadmillSnapshot>(EVENT_STATE, (event) => cb(event.payload));
  },
};
