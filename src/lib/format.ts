// Formatage des valeurs affichées (logique pure, testée dans format.test.ts).
// Conventions françaises : virgule décimale, « — » pour une valeur inconnue.

/** Valeur affichée quand une mesure n'est pas (encore) disponible. */
export const NONE = "—";

/** Nombre à `digits` décimales avec une virgule (sans séparateur de milliers). */
export function decimal(value: number, digits: number): string {
  // `toFixed` arrondit mal certains cas (1.005) ; on corrige par un petit epsilon.
  const factor = 10 ** digits;
  const rounded = Math.round((value + Math.sign(value) * 1e-9) * factor) / factor;
  return rounded.toFixed(digits).replace(".", ",");
}

/** Temps écoulé : « mm:ss », ou « h:mm:ss » à partir d'une heure. */
export function formatElapsed(totalSeconds: number): string {
  const s = Math.max(0, Math.floor(totalSeconds));
  const h = Math.floor(s / 3600);
  const m = Math.floor((s % 3600) / 60);
  const sec = s % 60;
  const pad = (n: number) => String(n).padStart(2, "0");
  return h > 0 ? `${h}:${pad(m)}:${pad(sec)}` : `${pad(m)}:${pad(sec)}`;
}

/**
 * Distance : en mètres sous 1 km, sinon en km à 2 décimales.
 * Les km sont tronqués (jamais arrondis vers le haut) : 999 m ne doit pas
 * s'afficher « 1,00 km », ni 1 996 m « 2,00 km ».
 */
export function formatDistance(meters: number | null): { value: string; unit: string } {
  if (meters === null || !Number.isFinite(meters)) return { value: NONE, unit: "" };
  const m = Math.max(0, Math.floor(meters));
  if (m < 1000) return { value: String(m), unit: "m" };
  return { value: decimal(Math.floor(m / 10) / 100, 2), unit: "km" };
}

/** Vitesse ou inclinaison à une décimale (« 3,5 »), « — » si inconnue. */
export function formatTenths(value: number | null): string {
  return value === null || !Number.isFinite(value) ? NONE : decimal(value, 1);
}

/** Énergie : « ≈ 12 » (kcal entières, l'unité du tapis n'étant pas confirmée). */
export function formatEnergy(kcal: number | null): string {
  return kcal === null || !Number.isFinite(kcal) ? NONE : `≈ ${Math.round(kcal)}`;
}

/** Pouls en bpm, « — » sans contact. */
export function formatPulse(bpm: number | null): string {
  return bpm === null || bpm <= 0 ? NONE : String(Math.round(bpm));
}
