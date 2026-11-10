// Leaflet et sa feuille de style, chargés à la demande (import dynamique) :
// la carte ne s'ouvre que dans l'éditeur, inutile d'alourdir la console.
import "leaflet/dist/leaflet.css";
import * as L from "leaflet";

export { L };

/** Une tuile WMTS de la Géoplateforme IGN (libre, sans clé, France seulement). */
function ign(layer: string, format: "image/png" | "image/jpeg", label: string): L.TileLayer {
  return L.tileLayer(
    "https://data.geopf.fr/wmts?SERVICE=WMTS&REQUEST=GetTile&VERSION=1.0.0" +
      `&LAYER=${layer}&STYLE=normal&TILEMATRIXSET=PM&TILEMATRIX={z}&TILEROW={y}&TILECOL={x}&FORMAT=${format}`,
    // Mentions sans lien : un clic ferait quitter l'application à la WebView.
    { maxZoom: 19, attribution: `${label} © IGN – Géoplateforme` },
  );
}

/** Fonds de carte proposés, le premier par défaut en France. */
export function baseLayers(): Record<string, L.TileLayer> {
  return {
    "Plan IGN": ign("GEOGRAPHICALGRIDSYSTEMS.PLANIGNV2", "image/png", "Plan IGN"),
    "Photos aériennes": ign("ORTHOIMAGERY.ORTHOPHOTOS", "image/jpeg", "Photographies aériennes"),
    OpenStreetMap: L.tileLayer("https://tile.openstreetmap.org/{z}/{x}/{y}.png", {
      maxZoom: 19,
      attribution: "© les contributeurs d'OpenStreetMap",
    }),
  };
}

/** Le parcours est-il en France métropolitaine (couverte par les fonds IGN) ? */
export function inMetropolitanFrance(bounds: L.LatLngBounds): boolean {
  return L.latLngBounds([41.2, -5.6], [51.2, 9.7]).contains(bounds);
}
