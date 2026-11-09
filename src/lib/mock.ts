// Tapis simulé, utilisé hors Tauri (navigateur, `npx vite`) pour développer et
// tester l'interface sans le RUN 7.
//
// Il imite le Rust ET le tapis : mêmes commandes, mêmes règles que le contrat
// (CLAUDE.md, « Contrat frontend ↔ Rust »), mêmes erreurs (chaînes en français),
// et une physique crédible tirée des observations sur le vrai tapis :
// - scan puis connexion avec délais ;
// - démarrage : compte à rebours ~5 s (écritures ignorées), puis 0,8 km/h ;
// - rampe de vitesse ~0,2 km/h/s, décélération ~0,4 km/h/s à l'arrêt ;
// - inclinaison ~0,3 %/s ;
// - distance en mètres, remontée par à-coups (toutes les 3 s) ;
// - énergie en kcal, temps calculé tant que la bande tourne ;
// - un second arrêt (bande déjà arrêtée) remet les compteurs à zéro ;
// - perte de liaison : la bande CONTINUE de tourner, reconnexion toutes les 2 s ;
//   un `stop` demandé entre-temps est mémorisé et part à la reconnexion ;
// - `stop` impossible (non connecté, écriture ratée) : promesse rejetée avec
//   « Arrêt NON envoyé… » ;
// - profils de course : profils de départ + profils créés (en mémoire), et
//   déroulement du profil choisi (`ProfileRunner`, miroir simplifié du Rust).
//
// La « physique » (la bande) tourne en permanence, connectée ou non : seul
// l'instantané envoyé à l'interface dépend de la liaison.

import { ProfileRunner, cloneProfile, profileError, starterProfiles } from "./profiles";
import {
  emptySnapshot,
  type DeviceInfo,
  type Limits,
  type RunProfile,
  type TreadmillSnapshot,
} from "./types";

export interface MockOptions {
  /** Petites oscillations de la vitesse réelle autour de la consigne (défaut : oui). */
  jitter?: boolean;
  /** Durée du scan, en ms. */
  scanMs?: number;
  /** Durée connexion + handshake + lectures statiques, en ms. */
  connectMs?: number;
  /** Pas de la simulation (et du « polling »), en ms. */
  tickMs?: number;
}

const DEVICE: DeviceInfo = { name: "RUN 7", id: "STB1T0158", article: "07883-600" };
const LIMITS: Limits = { speedMinKmh: 0.8, speedMaxKmh: 20, inclineMinPct: 0, inclineMaxPct: 15 };

const COUNTDOWN_S = 5;
const RAMP_UP_KMH_S = 0.2;
const RAMP_DOWN_KMH_S = 0.4;
const INCLINE_PCT_S = 0.3;
const DISTANCE_REPORT_S = 3;
const RECONNECT_RETRY_MS = 2000;
const MSG_PROFILE_RUNNING =
  "La bande tourne : mettez-la en pause pour changer de profil ou le modifier.";

/** Rapproche `from` de `to` d'au plus `step`. */
function approach(from: number, to: number, step: number): number {
  if (Math.abs(to - from) <= step) return to;
  return from + Math.sign(to - from) * step;
}

const tenth = (x: number) => Math.round(x * 10) / 10;

export class MockTreadmill {
  private snap: TreadmillSnapshot = emptySnapshot();
  private listeners = new Set<(s: TreadmillSnapshot) => void>();
  private readonly opts: Required<MockOptions>;
  private timer: ReturnType<typeof setInterval>;
  /** Incrémenté par `disconnect` : annule une connexion ou une reconnexion en cours. */
  private generation = 0;
  private reconnectTimer: ReturnType<typeof setTimeout> | null = null;

  // ── Physique du tapis (indépendante de la liaison) ──
  private belt = false; // Online : vrai = en marche
  private countdownS = 0;
  private speed = 0;
  private target = 0;
  private incline = 0;
  private targetIncline = 0;
  private distance = 0;
  private distanceReported = 0;
  private sinceDistanceReport = 0;
  private energy = 0;
  private elapsed = 0;
  private pulseOn = false;
  /** Arrêt demandé pendant une reconnexion, envoyé dès la liaison rétablie. */
  private pendingStop = false;
  /** Outil de simulation : le prochain `stop` échoue (écriture BLE ratée). */
  private failNextStop = false;

  // ── Profils de course ──
  private userProfiles: RunProfile[] = [];
  private runner: ProfileRunner | null = null;
  private nextProfileId = 1;

  constructor(options: MockOptions = {}) {
    this.opts = { jitter: true, scanMs: 1500, connectMs: 1200, tickMs: 500, ...options };
    this.timer = setInterval(() => this.tick(this.opts.tickMs / 1000), this.opts.tickMs);
  }

  /** Arrête les minuteries (tests). */
  dispose(): void {
    clearInterval(this.timer);
    if (this.reconnectTimer) clearTimeout(this.reconnectTimer);
    this.listeners.clear();
  }

  // ── Contrat ───────────────────────────────────────────────────────────────

  async getState(): Promise<TreadmillSnapshot> {
    return this.copy();
  }

  onState(cb: (s: TreadmillSnapshot) => void): () => void {
    this.listeners.add(cb);
    return () => this.listeners.delete(cb);
  }

  async connect(): Promise<void> {
    if (this.snap.connection === "connected") return;
    if (this.snap.connection !== "disconnected") {
      throw this.fail("Connexion déjà en cours");
    }
    const gen = ++this.generation;
    this.snap.connection = "scanning";
    this.snap.lastError = null;
    this.emit();
    await this.wait(this.opts.scanMs);
    // Annulée par `disconnect` : rien à signaler, c'était voulu.
    if (gen !== this.generation) return;
    this.snap.connection = "connecting";
    this.emit();
    await this.wait(this.opts.connectMs);
    if (gen !== this.generation) return;
    this.linkUp();
  }

  async disconnect(): Promise<void> {
    this.generation++;
    this.pendingStop = false;
    if (this.reconnectTimer) clearTimeout(this.reconnectTimer);
    this.reconnectTimer = null;
    this.snap = emptySnapshot();
    this.emit();
  }

  async start(): Promise<void> {
    this.requireConnected();
    if (!this.belt) {
      this.belt = true;
      this.countdownS = COUNTDOWN_S;
      this.target = 0;
    }
    this.ok();
  }

  async stop(): Promise<void> {
    if (this.snap.connection === "reconnecting") {
      // Comme le Rust : l'arrêt est mémorisé et partira dès la reconnexion,
      // mais la promesse est rejetée car rien n'est parti pour l'instant.
      this.pendingStop = true;
      throw this.fail(
        "Arrêt NON envoyé : le tapis n'est pas connecté. Il sera envoyé dès la reconnexion si la bande tourne encore. Utilisez le bouton stop de la console ou la clé de sécurité.",
      );
    }
    if (this.snap.connection !== "connected") {
      throw this.fail(
        "Arrêt NON envoyé : le tapis n'est pas connecté — utilisez le bouton stop de la console ou la clé de sécurité",
      );
    }
    if (this.failNextStop) {
      this.failNextStop = false;
      throw this.fail(
        "Arrêt NON envoyé (échec d'écriture Bluetooth) — utilisez le bouton stop de la console ou la clé de sécurité",
      );
    }
    this.applyStop();
    this.ok();
  }

  /** Effet d'un arrêt sur le tapis (Online = 1). */
  private applyStop(): void {
    if (this.belt) {
      this.belt = false;
      this.countdownS = 0;
      this.runner?.onStopped();
    } else {
      this.runner?.restart();
      // Second arrêt, bande déjà arrêtée : le tapis remet ses compteurs à zéro.
      // (Le Rust remet alors son temps à zéro en voyant la distance redescendre.)
      this.distance = 0;
      this.distanceReported = 0;
      this.energy = 0;
      this.elapsed = 0;
    }
    this.target = 0;
  }

  async setTargetSpeed(kmh: number): Promise<void> {
    this.requireRunning();
    if (!Number.isFinite(kmh)) throw this.fail("Vitesse invalide");
    const v = tenth(Math.min(LIMITS.speedMaxKmh, Math.max(LIMITS.speedMinKmh, tenth(kmh))));
    // Pendant le compte à rebours, le tapis ignore les écritures.
    if (this.countdownS <= 0) {
      this.target = v;
      this.runner?.onManual();
    }
    this.ok();
  }

  async setTargetIncline(pct: number): Promise<void> {
    this.requireRunning();
    if (!Number.isFinite(pct)) throw this.fail("Inclinaison invalide");
    const v = tenth(Math.min(LIMITS.inclineMaxPct, Math.max(LIMITS.inclineMinPct, tenth(pct))));
    if (this.countdownS <= 0) {
      this.targetIncline = v;
      this.runner?.onManual();
    }
    this.ok();
  }

  // ── Profils de course ─────────────────────────────────────────────────────

  async listProfiles(): Promise<RunProfile[]> {
    const user = this.userProfiles
      .map(cloneProfile)
      .sort((a, b) => a.name.toLowerCase().localeCompare(b.name.toLowerCase()));
    return [...starterProfiles(), ...user];
  }

  async saveProfile(profile: RunProfile): Promise<RunProfile> {
    const error = profileError(profile);
    if (error) throw error;
    if (starterProfiles().some((p) => p.id === profile.id)) {
      throw "Profil de départ en lecture seule : dupliquez-le pour le modifier.";
    }
    if (profile.id) this.ensureProfileEditable(profile.id);
    const saved: RunProfile = {
      ...cloneProfile(profile),
      name: profile.name.trim(),
      builtin: false,
      segments: profile.segments.map((s) => ({
        distanceM: s.distanceM,
        speedKmh: tenth(s.speedKmh),
        inclinePct: tenth(s.inclinePct),
      })),
    };
    if (!saved.id) {
      saved.id = `p${this.nextProfileId++}`;
      this.userProfiles.push(saved);
    } else {
      const i = this.userProfiles.findIndex((p) => p.id === saved.id);
      if (i < 0) throw "Profil introuvable.";
      this.userProfiles[i] = saved;
    }
    if (this.runner?.profile.id === saved.id) this.runner.replace(cloneProfile(saved));
    this.emit();
    return cloneProfile(saved);
  }

  async deleteProfile(id: string): Promise<void> {
    if (starterProfiles().some((p) => p.id === id)) {
      throw "Les profils de départ ne peuvent pas être supprimés.";
    }
    this.ensureProfileEditable(id);
    const before = this.userProfiles.length;
    this.userProfiles = this.userProfiles.filter((p) => p.id !== id);
    if (this.userProfiles.length === before) throw "Profil introuvable.";
    if (this.runner?.profile.id === id) this.runner = null;
    this.emit();
  }

  async selectProfile(id: string | null): Promise<void> {
    if (id === null) {
      this.runner = null;
    } else {
      if (this.belt && this.runner?.profile.id !== id) throw MSG_PROFILE_RUNNING;
      const p = (await this.listProfiles()).find((x) => x.id === id);
      if (!p) throw "Profil introuvable.";
      if (this.runner?.profile.id === id) this.runner.replace(p);
      else this.runner = new ProfileRunner(p);
    }
    this.emit();
  }

  private ensureProfileEditable(id: string): void {
    if (this.belt && this.runner?.profile.id === id) throw MSG_PROFILE_RUNNING;
  }

  // ── Outils de simulation (exposés en mode navigateur) ─────────────────────

  /** Coupe la liaison pendant `seconds` : la bande, elle, continue. */
  simulateLinkLoss(seconds = 7): void {
    if (this.snap.connection !== "connected") return;
    this.snap.connection = "reconnecting";
    this.snap.lostWhileRunning = this.snap.running === true;
    this.emit();
    const gen = this.generation;
    const until = Date.now() + seconds * 1000;
    const retry = () => {
      if (gen !== this.generation) return;
      if (Date.now() >= until) {
        this.reconnectTimer = null;
        this.linkUp();
      } else {
        this.reconnectTimer = setTimeout(retry, RECONNECT_RETRY_MS);
      }
    };
    this.reconnectTimer = setTimeout(retry, RECONNECT_RETRY_MS);
  }

  /** Retrait de la clé : la bande s'arrête net et le module BLE est coupé. */
  pullSafetyKey(): void {
    this.runner?.onStopped();
    this.belt = false;
    this.countdownS = 0;
    this.target = 0;
    this.speed = 0;
    this.simulateLinkLoss(10);
  }

  /** Fait échouer le prochain `stop` (pour vérifier l'alerte de l'interface). */
  failNextStopWrite(): void {
    this.failNextStop = true;
  }

  /** Mains sur les capteurs de pouls (ou non). */
  togglePulse(): void {
    this.pulseOn = !this.pulseOn;
  }

  // ── Interne ───────────────────────────────────────────────────────────────

  private tick(dt: number): void {
    if (this.belt) {
      this.elapsed += dt;
      if (this.countdownS > 0) {
        this.countdownS -= dt;
        if (this.countdownS <= 1e-6) {
          this.countdownS = 0;
          this.target = LIMITS.speedMinKmh;
          // La bande part directement à la vitesse minimale.
          this.speed = LIMITS.speedMinKmh;
        }
      }
    }
    const desired = this.belt && this.countdownS <= 0 ? this.target : 0;
    const rate = this.belt ? RAMP_UP_KMH_S : RAMP_DOWN_KMH_S;
    this.speed = approach(this.speed, desired, rate * dt);
    this.incline = approach(this.incline, this.targetIncline, INCLINE_PCT_S * dt);

    this.distance += (this.speed / 3.6) * dt;
    // Ordre de grandeur observé : ~10,5 kcal en 3 min 40 à 3 km/h.
    if (this.speed > 0) {
      const kcalPerMin = 0.95 * this.speed * (1 + this.incline / 10);
      this.energy += (kcalPerMin / 60) * dt;
    }
    this.sinceDistanceReport += dt;
    if (this.sinceDistanceReport >= DISTANCE_REPORT_S - 1e-6) {
      this.sinceDistanceReport = 0;
      this.distanceReported = Math.floor(this.distance);
      if (this.snap.connection === "connected") this.runner?.onDistance(this.distanceReported);
    }
    this.driveProfile();

    if (this.snap.connection === "connected") {
      this.refresh();
      this.emit();
    }
  }

  /** Comme le Rust : consignes du segment envoyées à chaque changement de segment. */
  private driveProfile(): void {
    if (!this.runner || this.snap.connection !== "connected") return;
    if (!this.belt || this.countdownS > 0) return;
    const wanted = this.runner.step();
    if (!wanted) return;
    this.target = tenth(Math.min(LIMITS.speedMaxKmh, Math.max(LIMITS.speedMinKmh, wanted.speedKmh)));
    this.targetIncline = tenth(
      Math.min(LIMITS.inclineMaxPct, Math.max(LIMITS.inclineMinPct, wanted.inclinePct)),
    );
  }

  /** Recopie la physique dans l'instantané (ce qu'un polling lirait). */
  private refresh(): void {
    const s = this.snap;
    s.running = this.belt;
    let speed = this.speed;
    if (this.opts.jitter && this.belt && speed > 0 && Math.abs(speed - this.target) < 0.05) {
      if (Math.random() < 0.3) speed -= 0.1;
    }
    s.speedKmh = tenth(Math.max(0, speed));
    s.targetSpeedKmh = tenth(this.target);
    s.inclinePct = tenth(this.incline);
    s.targetInclinePct = tenth(this.targetIncline);
    s.distanceM = this.distanceReported;
    s.energyKcal = tenth(this.energy);
    s.pulseBpm = this.pulseOn
      ? Math.round(88 + this.speed * 6 + this.incline * 2 + (this.opts.jitter ? Math.random() * 4 : 0))
      : null;
    s.elapsedS = Math.floor(this.elapsed);
  }

  private linkUp(): void {
    if (this.pendingStop) {
      this.pendingStop = false;
      this.applyStop();
    }
    this.snap.connection = "connected";
    this.snap.lostWhileRunning = false;
    this.snap.device = { ...DEVICE };
    this.snap.limits = { ...LIMITS };
    this.snap.lastError = null;
    this.refresh();
    this.emit();
  }

  private requireConnected(): void {
    if (this.snap.connection !== "connected") throw this.fail("Tapis non connecté");
  }

  private requireRunning(): void {
    this.requireConnected();
    if (!this.belt) throw this.fail("La bande ne tourne pas : démarrez-la d'abord");
  }

  /** Succès d'une commande : erreur effacée, état relu et diffusé. */
  private ok(): void {
    this.snap.lastError = null;
    if (this.snap.connection === "connected") this.refresh();
    this.emit();
  }

  /** Échec : l'erreur est mémorisée dans l'instantané et renvoyée (chaîne, comme le Rust). */
  private fail(message: string): string {
    this.snap.lastError = message;
    this.emit();
    return message;
  }

  private wait(ms: number): Promise<void> {
    return new Promise((resolve) => setTimeout(resolve, ms));
  }

  private copy(): TreadmillSnapshot {
    const s = this.snap;
    return {
      ...s,
      limits: s.limits ? { ...s.limits } : null,
      device: s.device ? { ...s.device } : null,
      profile: this.runner ? this.runner.progress() : null,
    };
  }

  private emit(): void {
    for (const cb of this.listeners) cb(this.copy());
  }
}
