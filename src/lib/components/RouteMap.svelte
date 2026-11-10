<script lang="ts">
  // Carte du parcours d'un profil importé d'un GPX : le tracé, découpé selon
  // les segments du profil (cliquer un morceau sélectionne le segment, comme
  // le graphique), le départ et l'arrivée. Leaflet est chargé à l'ouverture.
  //
  // Fonds de carte en ligne (IGN, OpenStreetMap) : sans connexion, le tracé
  // reste dessiné sur fond gris et un message le dit. Couleurs du design :
  // bleu = donnée (le tracé), vert = départ ; tokens `--kr-map-*` fixes (les
  // fonds de carte restent clairs en thème sombre).
  //
  // Chaque segment prend une teinte de bleu (clair → foncé) selon sa vitesse ou
  // son inclinaison, au choix de l'utilisateur (`routeColors.ts`), avec une
  // légende ; un liseré foncé garde les teintes claires lisibles sur la carte,
  // et un rond blanc marque chaque changement de segment.
  import { onMount } from "svelte";
  import type * as Leaflet from "leaflet";
  import { decimal } from "../format";
  import { COLOR_CLASSES, segmentColors, type ColorBy } from "../routeColors";
  import { splitBySegments } from "../track";
  import type { ProfileSegment, TrackPoint } from "../types";

  interface Props {
    track: TrackPoint[];
    segments: ProfileSegment[];
    selected: number | null;
    onselect: (index: number) => void;
  }

  let { track, segments, selected, onselect }: Props = $props();

  let el = $state<HTMLDivElement | null>(null);
  let ready = $state(false);
  let failed = $state<string | null>(null);
  /** Aucune tuile n'arrive (pas de connexion, serveur indisponible). */
  let tilesDown = $state(false);

  let L: typeof Leaflet | null = null;
  let map: Leaflet.Map | null = null;
  let parts: Leaflet.LayerGroup | null = null;
  /** Dernier segment choisi déjà montré (la carte ne se déplace qu'au changement de choix). */
  let lastShown: number | null = null;
  /** Morceaux du tracé du dernier dessin, un par segment (pour zoomer dessus). */
  let currentPieces: TrackPoint[][] = [];

  /** Marges du cadrage : à droite, la place de l'encart du segment choisi (éditeur). */
  const FIT_ALL = {
    paddingTopLeft: [32, 32] as [number, number],
    paddingBottomRight: [290, 32] as [number, number],
  };
  const FIT_SEGMENT = {
    paddingTopLeft: [56, 56] as [number, number],
    paddingBottomRight: [300, 56] as [number, number],
    // Au-delà, le Plan IGN n'a plus guère de détails pour se repérer.
    maxZoom: 17,
  };

  /** Vue d'ensemble : tout le tracé. */
  export function showAll(): void {
    if (map && L) map.fitBounds(L.latLngBounds(track), FIT_ALL);
  }

  /** Cadre la carte sur le segment `index`. */
  export function zoomToSegment(index: number): void {
    const piece = currentPieces[index];
    if (map && L && piece && piece.length >= 2) map.fitBounds(L.latLngBounds(piece), FIT_SEGMENT);
  }

  const LAYER_KEY = "kr-carte-fond";
  const COLOR_BY_KEY = "kr-carte-couleur";

  /** Mesure qui colore les segments (inclinaison par défaut : un profil importé a d'abord une seule vitesse). */
  let colorBy = $state<ColorBy>(readColorBy());
  const colors = $derived(segmentColors(segments, colorBy));

  function readColorBy(): ColorBy {
    try {
      return localStorage.getItem(COLOR_BY_KEY) === "speed" ? "speed" : "incline";
    } catch {
      return "incline";
    }
  }

  function chooseColorBy(by: ColorBy): void {
    colorBy = by;
    try {
      localStorage.setItem(COLOR_BY_KEY, by);
    } catch {
      // Stockage indisponible : le choix ne sera simplement pas retenu.
    }
  }

  function token(name: string): string {
    return getComputedStyle(document.documentElement).getPropertyValue(name).trim() || "#0A6FBF";
  }

  function readLayer(): string | null {
    try {
      return localStorage.getItem(LAYER_KEY);
    } catch {
      return null;
    }
  }

  function saveLayer(name: string): void {
    try {
      localStorage.setItem(LAYER_KEY, name);
    } catch {
      // Stockage indisponible : le choix ne sera simplement pas retenu.
    }
  }

  onMount(() => {
    let cancelled = false;
    import("../map")
      .then((mod) => {
        if (cancelled || !el) return;
        L = mod.L;
        const bounds = L.latLngBounds(track);
        // Zoom par niveaux entiers : un demi-niveau étire les tuiles et laisse
        // des lignes claires entre elles.
        map = L.map(el, { attributionControl: false, keyboard: false });
        // Préfixe sans lien (un clic ferait quitter l'application).
        L.control.attribution({ prefix: "Leaflet" }).addTo(map);
        L.control.scale({ imperial: false }).addTo(map);

        const layers = mod.baseLayers();
        const saved = readLayer();
        const first =
          saved && layers[saved] ? saved : mod.inMetropolitanFrance(bounds) ? "Plan IGN" : "OpenStreetMap";
        let loaded = 0;
        let errors = 0;
        for (const layer of Object.values(layers)) {
          layer.on("tileload", () => {
            loaded++;
            tilesDown = false;
          });
          layer.on("tileerror", () => {
            errors++;
            if (loaded === 0 && errors >= 2) tilesDown = true;
          });
        }
        layers[first].addTo(map);
        L.control.layers(layers, undefined, { position: "topright" }).addTo(map);

        // « Tout le tracé », sous les boutons de zoom.
        const FitAll = L.Control.extend({
          onAdd(): HTMLElement {
            const bar = document.createElement("div");
            bar.className = "leaflet-bar leaflet-control";
            const btn = document.createElement("button");
            btn.type = "button";
            btn.className = "kr-fit";
            btn.title = "Voir tout le tracé";
            btn.setAttribute("aria-label", "Voir tout le tracé");
            btn.innerHTML =
              '<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.4" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">' +
              '<path d="M4 9V4h5M20 9V4h-5M4 15v5h5M20 15v5h-5"></path></svg>';
            btn.addEventListener("click", () => showAll());
            bar.appendChild(btn);
            mod.L.DomEvent.disableClickPropagation(bar);
            return bar;
          },
        });
        new FitAll({ position: "topleft" }).addTo(map);
        map.on("baselayerchange", (e: Leaflet.LayersControlEvent) => {
          loaded = 0;
          errors = 0;
          tilesDown = false;
          saveLayer(e.name);
        });

        parts = L.layerGroup().addTo(map);
        const start = track[0];
        const end = track[track.length - 1];
        L.circleMarker(end, {
          radius: 7,
          color: "#FFFFFF",
          weight: 3,
          fillColor: token("--kr-map-casing"),
          fillOpacity: 1,
        })
          .bindTooltip("Arrivée")
          .addTo(map);
        L.circleMarker(start, {
          radius: 8,
          color: "#FFFFFF",
          weight: 3,
          fillColor: token("--kr-go"),
          fillOpacity: 1,
        })
          .bindTooltip("Départ")
          .addTo(map);

        map.fitBounds(bounds, FIT_ALL);
        ready = true;
      })
      .catch((e: unknown) => {
        failed = `Carte indisponible : ${e instanceof Error ? e.message : String(e)}`;
      });
    return () => {
      cancelled = true;
      map?.remove();
      map = null;
      parts = null;
    };
  });

  // Tracé découpé selon les segments, redessiné quand le profil, la mesure
  // choisie ou la sélection changent.
  $effect(() => {
    const pieces = splitBySegments(
      track,
      segments.map((s) => ({ ...s })),
    );
    const steps = colors.steps;
    const sel = selected;
    currentPieces = pieces;
    if (!ready || !L || !parts) return;
    const leaflet = L;
    const group = parts;
    group.clearLayers();
    const casing = token("--kr-map-casing");
    const none = token("--kr-map-none");
    const ramp = Array.from({ length: COLOR_CLASSES }, (_, i) => token(`--kr-map-seq-${i + 1}`));

    const draw = (piece: typeof pieces[number], i: number) => {
      const isSel = sel === i;
      const s = segments[i];
      const step = steps[i];
      leaflet
        .polyline(piece, {
          color: casing,
          weight: isSel ? 14 : 10,
          opacity: isSel ? 1 : 0.7,
          lineCap: "round",
          lineJoin: "round",
          interactive: false,
        })
        .addTo(group);
      const speed = Number.isFinite(s?.speedKmh) ? `${decimal(s.speedKmh, 1)} km/h` : "? km/h";
      const incline = Number.isFinite(s?.inclinePct) ? `${decimal(s.inclinePct, 1)} %` : "? %";
      leaflet
        .polyline(piece, {
          color: step === null ? none : ramp[step - 1],
          weight: isSel ? 9 : 7,
          lineCap: "round",
          lineJoin: "round",
        })
        .bindTooltip(`Segment ${i + 1} · ${incline} · ${speed}`, { sticky: true })
        .on("click", () => onselect(i))
        .addTo(group);
    };

    // Le segment choisi par-dessus les autres.
    pieces.forEach((piece, i) => {
      if (piece.length >= 2 && i !== sel) draw(piece, i);
    });
    if (sel !== null && pieces[sel]?.length >= 2) draw(pieces[sel], sel);
    // Changements de segment.
    pieces.forEach((piece, i) => {
      if (i === 0 || piece.length === 0) return;
      leaflet
        .circleMarker(piece[0], {
          radius: 3.5,
          color: casing,
          weight: 2,
          fillColor: "#FFFFFF",
          fillOpacity: 1,
          interactive: false,
        })
        .addTo(group);
    });

    // Nouveau segment choisi (‹ › du panneau) hors de la vue : la carte s'y déplace.
    if (sel !== lastShown) {
      lastShown = sel;
      const piece = sel !== null ? pieces[sel] : null;
      if (map && piece && piece.length >= 2) {
        const b = leaflet.latLngBounds(piece);
        if (!map.getBounds().contains(b)) map.panTo(b.getCenter());
      }
    }
  });
</script>

<div class="route">
  <div class="bar">
    <div class="by" role="group" aria-label="Couleur des segments">
      <span class="by-label">Couleur selon</span>
      <button type="button" class="by-btn" aria-pressed={colorBy === "incline"} onclick={() => chooseColorBy("incline")}>
        Inclinaison
      </button>
      <button type="button" class="by-btn" aria-pressed={colorBy === "speed"} onclick={() => chooseColorBy("speed")}>
        Vitesse
      </button>
    </div>
    <ul class="legend" aria-label={colorBy === "speed" ? "Légende : vitesse" : "Légende : inclinaison"}>
      {#each colors.legend as entry (entry.step)}
        <li>
          <span class="swatch" style:background={`var(--kr-map-seq-${entry.step})`}></span>
          {entry.label}
        </li>
      {/each}
    </ul>
  </div>
<div class="wrap">
  <div class="map" bind:this={el}></div>
  {#if failed}
    <div class="msg err" role="alert">{failed}</div>
  {:else if !ready}
    <div class="msg">Chargement de la carte…</div>
  {:else if tilesDown}
    <div class="msg" role="status">Fond de carte indisponible (pas de connexion Internet ?) : seul le tracé est affiché.</div>
  {/if}
</div>
</div>

<style>
  .route {
    width: 100%;
    height: 100%;
    display: flex;
    flex-direction: column;
    gap: var(--kr-sp-3);
  }

  .bar {
    flex-shrink: 0;
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: var(--kr-sp-2) var(--kr-sp-5);
  }

  .by {
    display: flex;
    align-items: center;
    gap: var(--kr-sp-2);
  }

  .by-label {
    font-size: 13px;
    font-weight: 700;
    color: var(--kr-text-2);
  }

  .by-btn {
    height: 36px;
    padding: 0 var(--kr-sp-3);
    border: 1px solid var(--kr-line);
    border-radius: var(--kr-r-sm);
    background: var(--kr-surface);
    color: var(--kr-text-2);
    font-family: var(--kr-font-ui);
    font-size: 14px;
    font-weight: 700;
  }

  .by-btn:hover {
    background: var(--kr-surface-2);
  }

  .by-btn[aria-pressed="true"] {
    border-color: var(--kr-accent);
    background: var(--kr-accent);
    color: var(--kr-accent-ink);
  }

  .legend {
    margin: 0;
    padding: 0;
    list-style: none;
    display: flex;
    flex-wrap: wrap;
    gap: var(--kr-sp-1) var(--kr-sp-4);
    font-size: 13px;
    font-weight: 600;
    font-variant-numeric: tabular-nums;
    color: var(--kr-text-2);
  }

  .legend li {
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .swatch {
    width: 22px;
    height: 8px;
    border-radius: 4px;
    box-shadow: 0 0 0 1.5px var(--kr-map-casing);
  }

  .wrap {
    position: relative;
    width: 100%;
    flex: 1;
    min-height: 0;
    border-radius: var(--kr-r-sm);
    overflow: hidden;
    background: var(--kr-surface-2);
  }

  .map {
    width: 100%;
    height: 100%;
    background: var(--kr-surface-2);
    font-family: var(--kr-font-ui);
  }

  .msg {
    position: absolute;
    left: 50%;
    /* En haut : le bas à droite porte l'encart du segment choisi (éditeur). */
    top: var(--kr-sp-3);
    transform: translateX(-50%);
    z-index: 1000;
    max-width: calc(100% - 32px);
    padding: var(--kr-sp-2) var(--kr-sp-4);
    border-radius: var(--kr-r-sm);
    background: var(--kr-surface);
    color: var(--kr-text-2);
    font-size: 14px;
    font-weight: 600;
    box-shadow: 0 2px 10px rgba(0, 0, 0, 0.18);
  }

  .msg.err {
    color: var(--kr-stop);
  }

  /* Leaflet : contrôles aux couleurs de l'application (thème clair et sombre). */
  .map :global(.leaflet-control-layers),
  .map :global(.leaflet-bar a),
  .map :global(.leaflet-bar a.leaflet-disabled) {
    font-family: var(--kr-font-ui);
    color: var(--kr-text);
    background-color: var(--kr-surface);
    border-color: var(--kr-line);
  }

  .map :global(.leaflet-bar button.kr-fit) {
    width: 30px;
    height: 30px;
    padding: 0;
    border: 0;
    border-radius: 4px;
    display: grid;
    place-items: center;
    background-color: var(--kr-surface);
    color: var(--kr-text);
    cursor: pointer;
  }

  .map :global(.leaflet-bar button.kr-fit:hover) {
    background-color: var(--kr-surface-2);
  }

  .map :global(.leaflet-bar a:hover) {
    background-color: var(--kr-surface-2);
  }

  .map :global(.leaflet-bar a.leaflet-disabled) {
    color: var(--kr-text-3);
  }

  .map :global(.leaflet-tooltip) {
    font-family: var(--kr-font-ui);
    font-size: 13px;
    font-weight: 600;
  }

  .map :global(.leaflet-control-layers-expanded) {
    padding: var(--kr-sp-2) var(--kr-sp-3);
    font-size: 14px;
  }
</style>
