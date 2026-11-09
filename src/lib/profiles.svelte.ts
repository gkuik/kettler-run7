import { api, errorMessage } from "./api";
import { cloneProfile, copyName, nextSegment, sameContent } from "./profiles";
import { treadmill } from "./treadmill.svelte";
import type { RunProfile } from "./types";

// État de l'interface autour des profils de course : la liste (lue sur le
// Rust), l'écran affiché (console ou éditeur) et le profil en cours d'édition.
// Le profil CHOISI n'est pas ici : il vient de l'instantané (`snap.profile`),
// le Rust en est seul maître.

export type View = "console" | "editor";

class ProfileStore {
  list = $state<RunProfile[]>([]);
  view = $state<View>("console");
  /** Copie modifiable du profil ouvert dans l'éditeur. */
  draft = $state<RunProfile | null>(null);
  /** Version enregistrée du profil ouvert (`null` : nouveau profil). */
  original = $state<RunProfile | null>(null);
  /** L'éditeur a été fermé d'office (bande démarrée) avec des modifications. */
  suspended = $state(false);
  /** Erreur de la dernière action de l'éditeur (enregistrer, supprimer…). */
  editorError = $state<string | null>(null);
  /** Message bref après un enregistrement réussi. */
  saved = $state(false);

  private savedTimer: ReturnType<typeof setTimeout> | null = null;

  async init(): Promise<void> {
    await this.refresh();
  }

  async refresh(): Promise<void> {
    try {
      this.list = await api.listProfiles();
    } catch (e) {
      this.editorError = errorMessage(e);
    }
  }

  find(id: string | null | undefined): RunProfile | null {
    return this.list.find((p) => p.id === id) ?? null;
  }

  /** Modifications non enregistrées dans l'éditeur ? */
  get dirty(): boolean {
    const d = this.draft;
    if (!d || d.builtin) return false;
    return this.original === null || !sameContent(d, this.original);
  }

  /** Choix du profil déroulé (`null` : aucun) ; erreur affichée comme une commande. */
  select(id: string | null): Promise<void> {
    return treadmill.run(() => api.selectProfile(id));
  }

  /** Ouvre l'éditeur sur un profil existant, ou sur un nouveau profil (`null`). */
  open(id: string | null): void {
    const p = id ? this.find(id) : null;
    this.original = p ? cloneProfile(p) : null;
    this.draft = p
      ? cloneProfile(p)
      : { id: "", name: "", builtin: false, segments: [nextSegment([])] };
    this.resume();
  }

  /** Revient dans l'éditeur (après une fermeture d'office, brouillon gardé). */
  resume(): void {
    this.suspended = false;
    this.editorError = null;
    this.flashSaved(false);
    this.view = "editor";
  }

  /** Retour à la console. `forced` : la bande a démarré, le brouillon est gardé. */
  close(forced = false): void {
    this.suspended = forced && this.dirty;
    if (!this.suspended) {
      this.draft = null;
      this.original = null;
    }
    this.view = "console";
  }

  /** Copie modifiable du profil ouvert (seul moyen de « modifier » un profil de départ). */
  duplicate(): void {
    const d = this.draft;
    if (!d) return;
    const copy = cloneProfile(d);
    copy.id = "";
    copy.builtin = false;
    copy.name = copyName(d.name, this.list.map((p) => p.name));
    this.draft = copy;
    this.original = null;
    this.editorError = null;
    this.flashSaved(false);
  }

  /** Enregistre le brouillon. Un nouveau profil est aussitôt choisi pour la séance. */
  async save(): Promise<boolean> {
    const d = this.draft;
    if (!d) return false;
    const isNew = !d.id;
    try {
      const saved = await api.saveProfile(cloneProfile(d));
      this.draft = cloneProfile(saved);
      this.original = cloneProfile(saved);
      this.editorError = null;
      await this.refresh();
      if (isNew) await api.selectProfile(saved.id).catch(() => undefined);
      this.flashSaved(true);
      return true;
    } catch (e) {
      this.editorError = errorMessage(e);
      return false;
    }
  }

  /** Supprime le profil ouvert puis revient à la console. */
  async remove(): Promise<void> {
    const id = this.draft?.id;
    if (!id) return this.close();
    try {
      await api.deleteProfile(id);
      await this.refresh();
      this.close();
    } catch (e) {
      this.editorError = errorMessage(e);
    }
  }

  private flashSaved(on: boolean): void {
    if (this.savedTimer) clearTimeout(this.savedTimer);
    this.savedTimer = null;
    this.saved = on;
    if (on) this.savedTimer = setTimeout(() => (this.saved = false), 2500);
  }
}

export const profiles = new ProfileStore();
