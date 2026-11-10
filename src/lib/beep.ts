// Avertisseurs sonores du profil de course : un bip quand le changement de
// segment est annoncé (bandeau orange de ProfileStrip), un autre son quand il
// a lieu (nouvelles consignes envoyées par le Rust). Son synthétisé par Web
// Audio : pas de fichier, marche hors ligne et dans le WebKit de macOS 12.
//
// WebKit crée un AudioContext « suspendu » hors d'un geste de l'utilisateur :
// `installAudioUnlock` le crée et le relance au premier clic ou à la première
// touche (le clic sur « Démarrer » suffit), avant que le profil ne bipe.

/**
 * Décide quand biper : une seule fois par segment, au début de son annonce.
 * Une pause dans la zone d'annonce ne fait pas rebiper ; un profil revenu au
 * début (fin de séance) rebipera.
 */
export class AnnounceBeeper {
  private beeped: string | null = null;

  /**
   * `segmentKey` identifie le segment en cours (profil + n°), `null` sans profil.
   * Rend vrai quand il faut biper maintenant.
   */
  update(segmentKey: string | null, announce: boolean, started: boolean): boolean {
    if (segmentKey === null || !started) {
      this.beeped = null;
      return false;
    }
    if (!announce || this.beeped === segmentKey) return false;
    this.beeped = segmentKey;
    return true;
  }
}

/**
 * Décide du son du changement effectif : quand le Rust passe au segment
 * suivant (consignes envoyées), ou à la récupération après le dernier. Rien au
 * premier état vu (appli ouverte en pleine séance) ni quand le profil revient
 * au début.
 */
export class SegmentChangeBeeper {
  private last: { profileId: string; index: number } | null = null;

  /** Rend vrai quand il faut sonner maintenant. */
  update(profileId: string | null, segmentIndex: number, started: boolean): boolean {
    if (profileId === null || !started) {
      this.last = null;
      return false;
    }
    const prev = this.last;
    this.last = { profileId, index: segmentIndex };
    return prev !== null && prev.profileId === profileId && segmentIndex > prev.index;
  }
}

type AudioCtor = typeof AudioContext;

let ctx: AudioContext | null = null;

function context(): AudioContext | null {
  if (ctx) return ctx;
  const w = window as unknown as { AudioContext?: AudioCtor; webkitAudioContext?: AudioCtor };
  const Ctor = w.AudioContext ?? w.webkitAudioContext;
  if (!Ctor) return null;
  try {
    ctx = new Ctor();
  } catch {
    return null;
  }
  return ctx;
}

function resume(c: AudioContext): void {
  if (c.state === "suspended") c.resume().catch(() => {});
}

/** Crée et relance le contexte audio au premier geste de l'utilisateur. */
export function installAudioUnlock(): void {
  const unlock = () => {
    const c = context();
    if (!c) return;
    resume(c);
    if (c.state === "running") {
      window.removeEventListener("pointerdown", unlock, true);
      window.removeEventListener("keydown", unlock, true);
    }
  };
  window.addEventListener("pointerdown", unlock, true);
  window.addEventListener("keydown", unlock, true);
}

/** Un bip : montée et descente douces (pas de « clic » aux bords). */
function tone(c: AudioContext, at: number, freq: number, length: number): void {
  const osc = c.createOscillator();
  const gain = c.createGain();
  osc.type = "triangle";
  osc.frequency.setValueAtTime(freq, at);
  gain.gain.setValueAtTime(0.0001, at);
  gain.gain.exponentialRampToValueAtTime(0.6, at + 0.012);
  gain.gain.setValueAtTime(0.6, at + length - 0.03);
  gain.gain.exponentialRampToValueAtTime(0.0001, at + length);
  osc.connect(gain).connect(c.destination);
  osc.start(at);
  osc.stop(at + length + 0.02);
}

/** Annonce du segment suivant : deux bips courts puis un plus aigu. */
export function playAnnounce(): void {
  const c = context();
  if (!c) return;
  resume(c);
  const t = c.currentTime + 0.02;
  tone(c, t, 880, 0.12);
  tone(c, t + 0.2, 880, 0.12);
  tone(c, t + 0.4, 1320, 0.22);
}

/** Changement de segment effectif : un « ding-dong » montant, plus long que l'annonce. */
export function playSegmentChange(): void {
  const c = context();
  if (!c) return;
  resume(c);
  const t = c.currentTime + 0.02;
  tone(c, t, 660, 0.25);
  tone(c, t + 0.25, 990, 0.5);
}
