// Import d'un parcours GPX en profil de course, logique pure (testée dans
// gpx.test.ts). Seul le RELIEF est repris : les vitesses enregistrées (souvent
// à vélo, ou avec des arrêts) n'ont pas de sens sur le tapis, tous les
// segments reçoivent donc la même vitesse, à adapter dans l'éditeur.
//
// Étapes : points du fichier → distance cumulée (formule de haversine) →
// altitude rééchantillonnée tous les 10 m et lissée (l'altitude GPS ou
// barométrique est arrondie au mètre et bruitée : sans lissage, le dénivelé
// est surestimé du double) → pente par tranches de 100 m → inclinaison du
// tapis (les descentes deviennent 0 % : le RUN 7 ne descend pas) → fusion des
// tranches voisines jusqu'à des segments d'au moins 200 m, 60 au plus, et
// différant d'au moins 1 % de leurs voisins.
//
// Le parseur ne dépend pas de DOMParser (absent des tests sous Node) : il
// cherche les balises <trkpt>/<rtept> et leur <ele>, ce qui suffit au format.

import {
  INCLINE_MAX_PCT,
  INCLINE_MIN_PCT,
  MAX_SEGMENTS,
  NAME_MAX_CHARS,
  SEGMENT_MAX_M,
  SEGMENT_MIN_M,
  clampSpeed,
} from "./profiles";
import type { ProfileSegment } from "./types";

/** Point du parcours : position et altitude (`null` si le fichier n'en donne pas). */
export interface GpxPoint {
  lat: number;
  lon: number;
  ele: number | null;
}

export interface GpxTrack {
  /** Nom de la trace (`<trk><name>` ou `<metadata><name>`), `null` sans nom. */
  name: string | null;
  points: GpxPoint[];
}

/** Ce que l'import a trouvé, affiché au-dessus de l'éditeur. */
export interface GpxImportInfo {
  fileName: string;
  /** Distance du parcours (m). */
  totalM: number;
  /** Dénivelés du parcours lissé (m) : `lossM` est perdu sur le tapis. */
  gainM: number;
  lossM: number;
  hasElevation: boolean;
}

export interface GpxImport {
  name: string;
  segments: ProfileSegment[];
  info: GpxImportInfo;
}

/** Pas du rééchantillonnage de l'altitude (m). */
const SAMPLE_M = 10;
/** Fenêtre du lissage (échantillons) : ±40 m autour de chaque point. */
const SMOOTH_SAMPLES = 9;
/** Longueur des tranches sur lesquelles la pente est mesurée (m). */
const BIN_M = 100;
/** Longueur minimale d'un segment importé (m) : en dessous, le tapis n'a pas le temps de monter. */
export const IMPORT_SEGMENT_MIN_M = 200;
/** Pas de l'inclinaison importée (%), celui des boutons ± de l'éditeur. */
const INCLINE_ROUND_PCT = 0.5;
/** Vitesse donnée à tous les segments importés (km/h), comme un nouveau profil. */
export const IMPORT_DEFAULT_SPEED_KMH = 4;

// ── Lecture du fichier ─────────────────────────────────────────────────────

const ENTITIES: Record<string, string> = { amp: "&", lt: "<", gt: ">", quot: '"', apos: "'" };

function decodeXml(text: string): string {
  return text
    .replace(/<!\[CDATA\[([\s\S]*?)\]\]>/g, "$1")
    .replace(/&(#x[0-9a-f]+|#\d+|\w+);/gi, (all, e: string) => {
      if (e[0] === "#") {
        const code = e[1] === "x" || e[1] === "X" ? parseInt(e.slice(2), 16) : parseInt(e.slice(1), 10);
        return Number.isFinite(code) ? String.fromCodePoint(code) : all;
      }
      return ENTITIES[e.toLowerCase()] ?? all;
    });
}

function attr(attrs: string, name: string): number | null {
  const m = new RegExp(`\\b${name}\\s*=\\s*["']([^"']*)["']`).exec(attrs);
  if (!m) return null;
  const n = Number(m[1].trim());
  return Number.isFinite(n) ? n : null;
}

function firstName(xml: string): string | null {
  const m = /<(?:\w+:)?name>([\s\S]*?)<\/(?:\w+:)?name>/.exec(xml);
  const name = m ? decodeXml(m[1]).replace(/\s+/g, " ").trim() : "";
  return name || null;
}

/** Points d'une trace (`trkpt`) ou à défaut d'un itinéraire (`rtept`). */
function readPoints(xml: string, tag: "trkpt" | "rtept"): GpxPoint[] {
  const re = new RegExp(
    `<(?:\\w+:)?${tag}\\b([^>]*?)(?:\\/>|>([\\s\\S]*?)<\\/(?:\\w+:)?${tag}\\s*>)`,
    "g",
  );
  const points: GpxPoint[] = [];
  for (let m = re.exec(xml); m; m = re.exec(xml)) {
    const lat = attr(m[1], "lat");
    const lon = attr(m[1], "lon");
    if (lat === null || lon === null || Math.abs(lat) > 90 || Math.abs(lon) > 180) continue;
    const e = /<(?:\w+:)?ele>\s*([^<]*?)\s*<\/(?:\w+:)?ele>/.exec(m[2] ?? "");
    const ele = e && e[1] !== "" && Number.isFinite(Number(e[1])) ? Number(e[1]) : null;
    points.push({ lat, lon, ele });
  }
  return points;
}

/** Lit un fichier GPX. Erreur (message en français) s'il n'en est pas un. */
export function parseGpx(xml: string): GpxTrack {
  if (!/<(?:\w+:)?gpx\b/.test(xml)) throw new Error("Ce fichier n'est pas un fichier GPX.");
  // Les commentaires peuvent contenir n'importe quoi : on les retire d'abord.
  const clean = xml.replace(/<!--[\s\S]*?-->/g, "");
  let points = readPoints(clean, "trkpt");
  let scope = /<(?:\w+:)?trk\b[\s\S]*/.exec(clean)?.[0] ?? clean;
  if (points.length === 0) {
    points = readPoints(clean, "rtept");
    scope = /<(?:\w+:)?rte\b[\s\S]*/.exec(clean)?.[0] ?? clean;
  }
  // Nom de la trace, sinon celui des métadonnées.
  const name = firstName(scope.split(/<(?:\w+:)?(?:trkseg|rtept)\b/)[0]) ?? firstName(clean);
  return { name, points };
}

// ── Relief ─────────────────────────────────────────────────────────────────

const EARTH_RADIUS_M = 6_371_000;

/** Distance entre deux points (m), formule de haversine. */
export function distanceM(a: GpxPoint, b: GpxPoint): number {
  const rad = Math.PI / 180;
  const dLat = (b.lat - a.lat) * rad;
  const dLon = (b.lon - a.lon) * rad;
  const h =
    Math.sin(dLat / 2) ** 2 + Math.cos(a.lat * rad) * Math.cos(b.lat * rad) * Math.sin(dLon / 2) ** 2;
  return 2 * EARTH_RADIUS_M * Math.asin(Math.min(1, Math.sqrt(h)));
}

/**
 * Altitude tous les `SAMPLE_M` mètres, lissée, et distance totale. Les points
 * sans altitude prennent celle de leurs voisins ; sans aucune altitude, tout
 * est à 0 (parcours plat).
 */
function elevationSamples(points: GpxPoint[]): { totalM: number; samples: number[]; hasElevation: boolean } {
  const dist: number[] = [0];
  for (let i = 1; i < points.length; i++) dist.push(dist[i - 1] + distanceM(points[i - 1], points[i]));
  const totalM = dist[dist.length - 1];

  const known = points.map((p, i) => ({ d: dist[i], e: p.ele })).filter((k): k is { d: number; e: number } => k.e !== null);
  const hasElevation = known.length > 0;
  const count = Math.floor(totalM / SAMPLE_M) + 1;
  const raw: number[] = [];
  let j = 0;
  for (let s = 0; s < count; s++) {
    const x = Math.min(totalM, s * SAMPLE_M);
    if (!hasElevation) {
      raw.push(0);
      continue;
    }
    while (j < known.length - 1 && known[j + 1].d <= x) j++;
    const a = known[j];
    const b = known[j + 1];
    if (!b || x <= a.d) raw.push(a.e);
    else raw.push(b.d === a.d ? b.e : a.e + ((x - a.d) / (b.d - a.d)) * (b.e - a.e));
  }
  // Dernier échantillon exactement à l'arrivée.
  if ((count - 1) * SAMPLE_M < totalM) raw.push(hasElevation ? known[known.length - 1].e : 0);

  // Moyenne glissante centrée ; la fenêtre se resserre aux deux bouts pour
  // garder les altitudes de départ et d'arrivée (sinon le dénivelé fond).
  const samples = raw.map((_, i) => {
    const half = Math.min(Math.floor(SMOOTH_SAMPLES / 2), i, raw.length - 1 - i);
    const from = i - half;
    const to = i + half + 1;
    let sum = 0;
    for (let k = from; k < to; k++) sum += raw[k];
    return sum / (to - from);
  });
  return { totalM, samples, hasElevation };
}

interface Piece {
  lengthM: number;
  /** Inclinaison du tapis, non arrondie (%). */
  incline: number;
}

const roundIncline = (pct: number) =>
  Math.min(INCLINE_MAX_PCT, Math.max(INCLINE_MIN_PCT, Math.round(pct / INCLINE_ROUND_PCT) * INCLINE_ROUND_PCT));

/** Segments nécessaires pour une longueur (un segment fait 10 km au plus). */
const piecesFor = (lengthM: number) => Math.max(1, Math.ceil(lengthM / SEGMENT_MAX_M));

/**
 * Réunit deux morceaux voisins. L'inclinaison est la moyenne pondérée des
 * inclinaisons DÉJÀ ramenées à 0 pour les descentes : la montée gardée sur le
 * tapis (longueur × inclinaison) ne change pas.
 */
function join(a: Piece, b: Piece): Piece {
  const lengthM = a.lengthM + b.lengthM;
  return { lengthM, incline: (a.incline * a.lengthM + b.incline * b.lengthM) / lengthM };
}

/** Fusionne les voisins dont l'inclinaison arrondie est la même. */
function mergeEqual(pieces: Piece[]): Piece[] {
  const out: Piece[] = [];
  for (const p of pieces) {
    const last = out[out.length - 1];
    if (last && roundIncline(last.incline) === roundIncline(p.incline)) out[out.length - 1] = join(last, p);
    else out.push(p);
  }
  return out;
}

/** Profil tiré du relief d'un parcours (voir l'en-tête du fichier). */
export function trackToSegments(
  track: GpxTrack,
  speedKmh = IMPORT_DEFAULT_SPEED_KMH,
): { segments: ProfileSegment[]; totalM: number; gainM: number; lossM: number; hasElevation: boolean } {
  if (track.points.length < 2) throw new Error("Le fichier GPX ne contient aucun parcours.");
  const { totalM, samples, hasElevation } = elevationSamples(track.points);
  if (totalM < SEGMENT_MIN_M) throw new Error(`Parcours trop court (moins de ${SEGMENT_MIN_M} m).`);
  if (totalM > MAX_SEGMENTS * SEGMENT_MAX_M) {
    throw new Error(`Parcours trop long (${MAX_SEGMENTS * SEGMENT_MAX_M / 1000} km au plus).`);
  }

  let gainM = 0;
  let lossM = 0;
  for (let i = 1; i < samples.length; i++) {
    const d = samples[i] - samples[i - 1];
    if (d > 0) gainM += d;
    else lossM -= d;
  }

  // Pente par tranches de BIN_M ; la dernière tranche, plus courte, compte
  // pour sa longueur réelle.
  const perBin = BIN_M / SAMPLE_M;
  let pieces: Piece[] = [];
  for (let i = 0; i < samples.length - 1; i += perBin) {
    const end = Math.min(i + perBin, samples.length - 1);
    const lengthM = Math.min(totalM, end * SAMPLE_M) - i * SAMPLE_M;
    if (lengthM <= 0) break;
    const slope = ((samples[end] - samples[i]) / lengthM) * 100;
    pieces.push({ lengthM, incline: Math.min(INCLINE_MAX_PCT, Math.max(0, slope)) });
  }
  pieces = mergeEqual(pieces);

  // Le plus court morceau rejoint le voisin d'inclinaison la plus proche, tant
  // qu'il en reste un trop court ou qu'il y a trop de segments.
  const count = () => pieces.reduce((n, p) => n + piecesFor(p.lengthM), 0);
  while (pieces.length > 1) {
    let k = 0;
    for (let i = 1; i < pieces.length; i++) if (pieces[i].lengthM < pieces[k].lengthM) k = i;
    if (pieces[k].lengthM >= IMPORT_SEGMENT_MIN_M && count() <= MAX_SEGMENTS) break;
    const near = [k - 1, k + 1].filter((i) => i >= 0 && i < pieces.length);
    const gap = (i: number) => Math.abs(roundIncline(pieces[i].incline) - roundIncline(pieces[k].incline));
    const o = near.reduce((best, i) => (gap(i) < gap(best) || (gap(i) === gap(best) && pieces[i].lengthM < pieces[best].lengthM) ? i : best));
    const [a, b] = k < o ? [k, o] : [o, k];
    pieces.splice(a, 2, join(pieces[a], pieces[b]));
    pieces = mergeEqual(pieces);
  }

  // Enfin, deux voisins à un cran d'écart (0,5 %) ne valent pas un
  // mouvement du moteur : on les réunit aussi (le plus court d'abord).
  for (;;) {
    let k = -1;
    for (let i = 0; i < pieces.length - 1; i++) {
      const gap = Math.abs(roundIncline(pieces[i].incline) - roundIncline(pieces[i + 1].incline));
      const len = pieces[i].lengthM + pieces[i + 1].lengthM;
      if (gap <= INCLINE_ROUND_PCT && (k < 0 || len < pieces[k].lengthM + pieces[k + 1].lengthM)) k = i;
    }
    if (k < 0) break;
    pieces.splice(k, 2, join(pieces[k], pieces[k + 1]));
    pieces = mergeEqual(pieces);
  }

  // Découpe des morceaux de plus de 10 km, puis distances entières : on
  // arrondit les bornes cumulées, pas chaque longueur, pour garder le total.
  const speed = clampSpeed(speedKmh);
  const segments: ProfileSegment[] = [];
  let at = 0;
  let prevEnd = 0;
  for (const p of pieces) {
    const n = piecesFor(p.lengthM);
    for (let i = 0; i < n; i++) {
      at += p.lengthM / n;
      const end = Math.round(at);
      segments.push({ distanceM: end - prevEnd, speedKmh: speed, inclinePct: roundIncline(p.incline) });
      prevEnd = end;
    }
  }
  return { segments, totalM, gainM, lossM, hasElevation };
}

/** Nom du profil : celui du fichier (« trail-la-vaucelle-14-km.gpx » → « Trail la vaucelle 14 km »), sinon celui de la trace. */
export function profileNameFor(fileName: string, trackName: string | null): string {
  const base = fileName
    .replace(/^.*[\\/]/, "")
    .replace(/\.gpx$/i, "")
    .replace(/[_-]+/g, " ")
    .replace(/\s+/g, " ")
    .trim();
  const name = base || trackName || "Parcours importé";
  return [...(name.charAt(0).toUpperCase() + name.slice(1))].slice(0, NAME_MAX_CHARS).join("").trim();
}

/** Fichier GPX → brouillon de profil (pas encore enregistré). */
export function importGpx(xml: string, fileName: string, speedKmh = IMPORT_DEFAULT_SPEED_KMH): GpxImport {
  const track = parseGpx(xml);
  const { segments, totalM, gainM, lossM, hasElevation } = trackToSegments(track, speedKmh);
  return {
    name: profileNameFor(fileName, track.name),
    segments,
    info: { fileName: fileName.replace(/^.*[\\/]/, ""), totalM, gainM, lossM, hasElevation },
  };
}
