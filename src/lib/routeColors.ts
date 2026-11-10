// Couleur des segments sur la carte du parcours, selon la vitesse ou
// l'inclinaison choisie par l'utilisateur. Logique pure (routeColors.test.ts).
//
// Échelle ordinale à une seule teinte (bleu = donnée, clair → foncé), cinq
// classes au plus : au-delà, l'œil ne les distingue plus sur une carte. Si le
// profil a cinq valeurs différentes ou moins, chaque valeur a sa classe (la
// légende montre les valeurs exactes) ; sinon, cinq intervalles égaux entre
// le minimum et le maximum.

import { decimal } from "./format";
import type { ProfileSegment } from "./types";

export type ColorBy = "speed" | "incline";

/** Nombre de classes (et de teintes de l'échelle `--kr-map-seq-1..5`). */
export const COLOR_CLASSES = 5;

export interface LegendEntry {
  /** Rang de la teinte dans l'échelle, 1 (clair) à 5 (foncé). */
  step: number;
  label: string;
}

export interface SegmentColors {
  /** Teinte (1 à 5) de chaque segment ; `null` si sa valeur est en cours de saisie. */
  steps: (number | null)[];
  legend: LegendEntry[];
}

const UNIT: Record<ColorBy, string> = { speed: "km/h", incline: "%" };

const value = (s: ProfileSegment, by: ColorBy) => (by === "speed" ? s.speedKmh : s.inclinePct);
const round1 = (x: number) => Math.round(x * 10) / 10;

/** Teintes réparties sur toute l'échelle : 1 classe → milieu, 2 → bouts, 5 → toutes. */
function spread(count: number): number[] {
  if (count <= 1) return [Math.ceil(COLOR_CLASSES / 2)];
  return Array.from({ length: count }, (_, i) => 1 + Math.round((i * (COLOR_CLASSES - 1)) / (count - 1)));
}

export function segmentColors(segments: ProfileSegment[], by: ColorBy): SegmentColors {
  const unit = UNIT[by];
  const vals = segments.map((s) => {
    const v = value(s, by);
    return Number.isFinite(v) ? round1(v) : null;
  });
  const known = vals.filter((v): v is number => v !== null);
  if (known.length === 0) return { steps: vals.map(() => null), legend: [] };

  const distinct = [...new Set(known)].sort((a, b) => a - b);
  if (distinct.length <= COLOR_CLASSES) {
    const steps = spread(distinct.length);
    return {
      steps: vals.map((v) => (v === null ? null : steps[distinct.indexOf(v)])),
      legend: distinct.map((v, i) => ({ step: steps[i], label: `${decimal(v, 1)} ${unit}` })),
    };
  }

  // Intervalles égaux ; la légende montre les valeurs réellement présentes
  // dans chaque classe (une classe vide n'y figure pas).
  const min = distinct[0];
  const width = (distinct[distinct.length - 1] - min) / COLOR_CLASSES;
  const classOf = (v: number) => Math.min(COLOR_CLASSES - 1, Math.floor((v - min) / width + 1e-9));
  const steps = vals.map((v) => (v === null ? null : classOf(v) + 1));
  const legend: LegendEntry[] = [];
  for (let c = 0; c < COLOR_CLASSES; c++) {
    const inClass = distinct.filter((v) => classOf(v) === c);
    if (inClass.length === 0) continue;
    const lo = inClass[0];
    const hi = inClass[inClass.length - 1];
    legend.push({
      step: c + 1,
      label: lo === hi ? `${decimal(lo, 1)} ${unit}` : `${decimal(lo, 1)} – ${decimal(hi, 1)} ${unit}`,
    });
  }
  return { steps, legend };
}
