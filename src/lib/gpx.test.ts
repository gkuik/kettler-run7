import { describe, expect, it } from "vitest";
import {
  IMPORT_SEGMENT_MIN_M,
  distanceM,
  importGpx,
  parseGpx,
  profileNameFor,
  trackToSegments,
  type GpxPoint,
} from "./gpx";
import { MAX_SEGMENTS, SEGMENT_MAX_M, profileError, summarize } from "./profiles";

/** Mètres par degré de latitude (le long d'un méridien). */
const M_PER_DEG = distanceM({ lat: 0, lon: 0, ele: null }, { lat: 1, lon: 0, ele: null });

/** Parcours rectiligne vers le nord, un point tous les `stepM` mètres, altitude `ele(d)`. */
function line(totalM: number, ele: ((d: number) => number) | null, stepM = 5): GpxPoint[] {
  const pts: GpxPoint[] = [];
  for (let d = 0; d <= totalM + 1e-9; d += stepM) {
    pts.push({ lat: 50 + d / M_PER_DEG, lon: 3, ele: ele ? ele(d) : null });
  }
  return pts;
}

function gpx(points: GpxPoint[], name = "Sortie"): string {
  const pts = points
    .map((p) => `<trkpt lat="${p.lat}" lon="${p.lon}">${p.ele === null ? "" : `<ele>${p.ele}</ele>`}<time>2026-05-30T11:57:23Z</time></trkpt>`)
    .join("\n");
  return `<?xml version="1.0"?><gpx version="1.1" creator="test"><trk><name>${name}</name><trkseg>${pts}</trkseg></trk></gpx>`;
}

const total = (segs: { distanceM: number }[]) => segs.reduce((s, x) => s + x.distanceM, 0);

describe("lecture du fichier", () => {
  it("lit les points, leur altitude et le nom de la trace", () => {
    const t = parseGpx(`<?xml version="1.0"?>
      <gpx xmlns="http://www.topografix.com/GPX/1/1">
        <metadata><name>Métadonnées</name></metadata>
        <!-- <trkpt lat="9" lon="9"></trkpt> -->
        <trk><name>Boucle &amp; côte &#233;</name><trkseg>
          <trkpt lat="50.1" lon="3.2"><ele>74</ele></trkpt>
          <trkpt lon='3.3' lat='50.2'><ele> 75.5 </ele><extensions><x:ele>1</x:ele></extensions></trkpt>
          <trkpt lat="50.3" lon="3.4"/>
          <trkpt lat="abc" lon="3.4"></trkpt>
        </trkseg></trk>
      </gpx>`);
    expect(t.name).toBe("Boucle & côte é");
    expect(t.points).toEqual([
      { lat: 50.1, lon: 3.2, ele: 74 },
      { lat: 50.2, lon: 3.3, ele: 75.5 },
      { lat: 50.3, lon: 3.4, ele: null },
    ]);
  });

  it("prend un itinéraire (rtept) faute de trace, et les balises préfixées", () => {
    const t = parseGpx(`<gpx:gpx><gpx:rte><gpx:name>Itinéraire</gpx:name>
      <gpx:rtept lat="1" lon="2"><gpx:ele>10</gpx:ele></gpx:rtept>
      <gpx:rtept lat="1.01" lon="2"></gpx:rtept></gpx:rte></gpx:gpx>`);
    expect(t.name).toBe("Itinéraire");
    expect(t.points).toEqual([
      { lat: 1, lon: 2, ele: 10 },
      { lat: 1.01, lon: 2, ele: null },
    ]);
  });

  it("refuse ce qui n'est pas un GPX, ou un GPX sans parcours", () => {
    expect(() => importGpx("<kml></kml>", "a.kml")).toThrow(/pas un fichier GPX/);
    expect(() => importGpx("<gpx><wpt lat='1' lon='2'/></gpx>", "a.gpx")).toThrow(/aucun parcours/);
    expect(() => importGpx(gpx(line(30, () => 0)), "a.gpx")).toThrow(/trop court/);
  });

  it("distance de haversine", () => {
    expect(M_PER_DEG).toBeCloseTo(111_195, 0);
  });
});

describe("relief → segments", () => {
  it("parcours plat : un seul segment à 0 %, à la vitesse demandée", () => {
    const r = trackToSegments({ name: null, points: line(3000, () => 80) }, 5.5);
    expect(r.segments).toEqual([{ distanceM: 3000, speedKmh: 5.5, inclinePct: 0 }]);
    expect(r.gainM).toBeCloseTo(0, 6);
  });

  it("une côte de 1 km à 5 % au milieu", () => {
    const ele = (d: number) => 100 + 0.05 * Math.min(1000, Math.max(0, d - 1000));
    const r = trackToSegments({ name: null, points: line(3000, ele) });
    expect(r.segments.map((s) => s.inclinePct)).toEqual([0, 5, 0]);
    // Le lissage (±40 m) étale les bords de la côte : ±100 m.
    for (const [i, s] of r.segments.entries()) expect(Math.abs(s.distanceM - 1000)).toBeLessThanOrEqual(100 * (i === 1 ? 2 : 1));
    expect(total(r.segments)).toBe(3000);
    expect(r.gainM).toBeCloseTo(50, 0);
    expect(summarize(r.segments).gainM).toBeCloseTo(50, -1);
  });

  it("les descentes passent à 0 % ; le dénivelé négatif est signalé", () => {
    const ele = (d: number) => (d < 1000 ? 100 + 0.04 * d : 140 - 0.04 * (d - 1000));
    const r = trackToSegments({ name: null, points: line(2000, ele) });
    expect(r.segments.map((s) => s.inclinePct)).toEqual([4, 0]);
    // Le lissage arrondit le sommet : un peu moins que 40 m.
    expect(r.lossM).toBeGreaterThan(38);
    expect(r.lossM).toBeLessThanOrEqual(40);
  });

  it("altitude arrondie au mètre (bruit) : la pente reste juste", () => {
    const r = trackToSegments({ name: null, points: line(2000, (d) => Math.round(70 + 0.03 * d)) });
    expect(r.segments).toHaveLength(1);
    expect(r.segments[0].inclinePct).toBe(3);
    expect(r.gainM).toBeCloseTo(60, 0);
  });

  it("sans altitude : parcours plat", () => {
    const r = trackToSegments({ name: null, points: line(1500, null) });
    expect(r.hasElevation).toBe(false);
    expect(r.segments).toEqual([{ distanceM: 1500, speedKmh: 4, inclinePct: 0 }]);
  });

  it("plus de 10 km d'un seul tenant : découpé en segments de 10 km au plus", () => {
    const r = trackToSegments({ name: null, points: line(25_000, () => 0, 25) });
    expect(r.segments.map((s) => s.distanceM)).toEqual([8333, 8334, 8333]);
  });

  it("relief très découpé : 60 segments au plus, tous valides, total gardé", () => {
    // 40 km de bosses de 300 m à 0, 2, 4, 6 % : bien plus de 60 changements.
    const slope = (d: number) => [0, 0.02, 0.04, 0.06][Math.floor(d / 300) % 4];
    let h = 50;
    const pts = line(40_000, null, 10).map((p, i) => {
      if (i > 0) h += slope((i - 1) * 10) * 10;
      return { ...p, ele: h };
    });
    const r = trackToSegments({ name: null, points: pts });
    expect(r.segments.length).toBeLessThanOrEqual(MAX_SEGMENTS);
    expect(total(r.segments)).toBe(40_000);
    for (const s of r.segments) {
      expect(s.distanceM).toBeGreaterThanOrEqual(IMPORT_SEGMENT_MIN_M);
      expect(s.distanceM).toBeLessThanOrEqual(SEGMENT_MAX_M);
    }
    expect(profileError({ name: "Bosses", segments: r.segments })).toBeNull();
    // La montée gardée sur le tapis reste proche de celle du parcours.
    expect(summarize(r.segments).gainM / r.gainM).toBeGreaterThan(0.9);
    expect(summarize(r.segments).gainM / r.gainM).toBeLessThan(1.1);
  });

  it("courte bosse : rattachée à un voisin, pas de segment de moins de 200 m", () => {
    const ele = (d: number) => 100 + 0.08 * Math.min(100, Math.max(0, d - 1000));
    const r = trackToSegments({ name: null, points: line(2000, ele) });
    for (const s of r.segments) expect(s.distanceM).toBeGreaterThanOrEqual(IMPORT_SEGMENT_MIN_M);
    expect(Math.max(...r.segments.map((s) => s.inclinePct))).toBeGreaterThan(0);
  });

  it("voisins à un cran d'écart (0,5 %) réunis", () => {
    const ele = (d: number) => 100 + (d < 1000 ? 0.02 * d : 20 + 0.025 * (d - 1000));
    const r = trackToSegments({ name: null, points: line(2000, ele) });
    expect(r.segments).toHaveLength(1);
    expect(r.segments[0].inclinePct).toBe(2);
  });

  it("inclinaison plafonnée à 15 %", () => {
    const r = trackToSegments({ name: null, points: line(1000, (d) => 0.3 * d) });
    expect(r.segments[0].inclinePct).toBe(15);
  });
});

describe("nom et brouillon", () => {
  it("nom tiré du fichier, sinon de la trace", () => {
    expect(profileNameFor("trail-la-vaucelle-14-km.gpx", "Vélo")).toBe("Trail la vaucelle 14 km");
    expect(profileNameFor("/Users/x/Downloads/mon_parcours.GPX", null)).toBe("Mon parcours");
    expect(profileNameFor(".gpx", "Boucle du lac")).toBe("Boucle du lac");
    expect(profileNameFor("", null)).toBe("Parcours importé");
    expect([...profileNameFor("x".repeat(80) + ".gpx", null)]).toHaveLength(40);
  });

  it("brouillon valide, prêt pour l'éditeur", () => {
    const ele = (d: number) => 70 + 0.05 * Math.min(800, Math.max(0, d - 400));
    const r = importGpx(gpx(line(2400, ele), "Vélo tout terrain"), "côte.gpx");
    expect(r.name).toBe("Côte");
    expect(profileError(r)).toBeNull();
    expect(r.info).toMatchObject({ fileName: "côte.gpx", hasElevation: true });
    expect(r.info.totalM).toBeCloseTo(2400, 0);
    expect(r.info.gainM).toBeCloseTo(40, 0);
  });
});
