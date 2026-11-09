import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { MockTreadmill } from "./mock";
import {
  EVENT_STATE,
  type RunProfile,
  type SimAction,
  type SimStatus,
  type TreadmillSnapshot,
} from "./types";

// ── Point d'accès UNIQUE au backend Rust ──────────────────────────────────
// Aucun composant n'appelle `invoke` directement : tout passe par ici.
//
// Deux façons de travailler sans le RUN 7 :
// - hors Tauri (navigateur, `npx vite`), les appels sont servis par un tapis
//   simulé en TypeScript (`mock.ts`) ;
// - dans l'application lancée avec `npm run app:sim` (KETTLER_SIM=1), le Rust
//   pilote un tapis simulé au niveau du protocole (`src-tauri/src/treadmill/sim.rs`) :
//   tout le code Rust tourne comme avec le vrai tapis.

/** Vrai dans l'application native (Tauri injecte `__TAURI_INTERNALS__`). */
export const inTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

/** Vrai hors de l'application native : tapis simulé en TypeScript. */
export const browserMock = !inTauri;

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

/** Les commandes du contrat (CLAUDE.md, « Contrat frontend ↔ Rust »). */
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

  /** Profils de départ puis profils de l'utilisateur. */
  listProfiles: (): Promise<RunProfile[]> =>
    inTauri ? call<RunProfile[]>("list_profiles") : mock().listProfiles(),
  /** Enregistre (crée si `id` est vide) ; rend le profil tel qu'enregistré. */
  saveProfile: (profile: RunProfile): Promise<RunProfile> =>
    inTauri ? call<RunProfile>("save_profile", { profile }) : mock().saveProfile(profile),
  deleteProfile: (id: string): Promise<void> =>
    inTauri ? call<void>("delete_profile", { id }) : mock().deleteProfile(id),
  /** Profil déroulé pendant la séance ; `null` : aucun. */
  selectProfile: (id: string | null): Promise<void> =>
    inTauri ? call<void>("select_profile", { id }) : mock().selectProfile(id),

  /** Tapis simulé côté Rust (application lancée avec KETTLER_SIM=1). */
  sim: {
    /** `null` quand l'application pilote le vrai tapis (ou hors Tauri). */
    status: (): Promise<SimStatus | null> =>
      inTauri ? call<SimStatus | null>("sim_status") : Promise.resolve(null),
    act: (action: SimAction): Promise<void> => call<void>("sim_action", { action }),
  },

  /** Abonnement à `treadmill-state` ; renvoie la fonction de désabonnement. */
  onState: async (cb: (s: TreadmillSnapshot) => void): Promise<() => void> => {
    if (!inTauri) return mock().onState(cb);
    return listen<TreadmillSnapshot>(EVENT_STATE, (event) => cb(event.payload));
  },
};
