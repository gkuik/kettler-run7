import { api, browserMock } from "./api";
import type { SimAction, SimStatus } from "./types";

// Quel tapis simulé l'interface pilote-t-elle ?
// - "browser" : hors Tauri, le tapis TypeScript (`mock.ts`) ;
// - "native"  : application lancée avec KETTLER_SIM=1, le tapis Rust (`sim.rs`) ;
// - null      : le vrai RUN 7.

const POLL_MS = 500;

class SimStore {
  kind = $state<"browser" | "native" | null>(browserMock ? "browser" : null);
  /** Dernier état du tapis Rust (mode "native" seulement). */
  status = $state<SimStatus | null>(null);

  private timer: ReturnType<typeof setInterval> | null = null;

  /** Demande au Rust s'il simule le tapis. */
  async init(): Promise<void> {
    if (browserMock) return;
    try {
      const s = await api.sim.status();
      if (s) {
        this.kind = "native";
        this.status = s;
      }
    } catch {
      // Ancienne version du Rust ou erreur : pas de menu simulateur.
    }
  }

  /** Relit l'état du tapis Rust tant que le menu est ouvert. */
  watch(on: boolean): void {
    if (this.timer) clearInterval(this.timer);
    this.timer = null;
    if (on && this.kind === "native") {
      void this.refresh();
      this.timer = setInterval(() => void this.refresh(), POLL_MS);
    }
  }

  async refresh(): Promise<void> {
    try {
      this.status = await api.sim.status();
    } catch {
      // Le prochain tour réessaiera.
    }
  }

  async act(action: SimAction): Promise<void> {
    try {
      await api.sim.act(action);
    } finally {
      await this.refresh();
    }
  }
}

export const sim = new SimStore();
