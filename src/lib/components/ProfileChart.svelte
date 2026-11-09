<script lang="ts">
  // Aperçu d'un profil de course : en haut la vitesse (une colonne par segment,
  // largeur = distance), en bas l'inclinaison. Sert dans l'éditeur (grand,
  // segments cliquables) et dans la bande de progression de la console
  // (compact, partie parcourue en plein et curseur).
  //
  // Formes en SVG étiré (preserveAspectRatio="none") ; textes en HTML par-dessus,
  // positionnés en %, pour qu'ils ne soient jamais déformés.
  import { decimal } from "../format";
  import type { ProfileSegment } from "../types";

  interface Props {
    segments: ProfileSegment[];
    /** Distance parcourue dans le profil (console) ; `null` : pas de progression. */
    doneM?: number | null;
    /** Segment mis en évidence (éditeur). */
    selected?: number | null;
    onselect?: (index: number) => void;
    compact?: boolean;
  }

  let { segments, doneM = null, selected = null, onselect, compact = false }: Props =
    $props();

  // Repère : x de 0 à 1000, y de 0 à 100. Vitesse en haut (0–62), inclinaison en bas (70–100).
  const W = 1000;
  const SPEED_BOTTOM = 62;
  const SPEED_H = 58;
  const INC_TOP = 70;
  const INC_BOTTOM = 100;
  const GAP = 3;

  const total = $derived(segments.reduce((sum, s) => sum + Math.max(0, s.distanceM || 0), 0));
  const maxSpeed = $derived(Math.max(0, ...segments.map((s) => s.speedKmh || 0)));
  const speedTop = $derived(Math.max(4, Math.ceil(maxSpeed * 1.15)));
  const inclineTop = $derived(Math.max(5, Math.ceil(Math.max(0, ...segments.map((s) => s.inclinePct || 0)))));

  interface Bar {
    x: number;
    w: number;
    speedY: number;
    incY: number;
    seg: ProfileSegment;
  }

  const bars = $derived.by((): Bar[] => {
    if (total <= 0) return [];
    let at = 0;
    return segments.map((seg) => {
      const d = Math.max(0, seg.distanceM || 0);
      const x = (at / total) * W;
      const w = (d / total) * W;
      at += d;
      const speed = Math.max(0, seg.speedKmh || 0);
      const speedY = SPEED_BOTTOM - (Math.min(speed, speedTop) / speedTop) * SPEED_H;
      const inc = Math.max(0, seg.inclinePct || 0);
      const incY = INC_BOTTOM - (Math.min(inc, inclineTop) / inclineTop) * (INC_BOTTOM - INC_TOP);
      return { x, w, speedY, incY, seg };
    });
  });

  const doneX = $derived(
    doneM === null || total <= 0 ? null : (Math.min(Math.max(0, doneM), total) / total) * W,
  );

  /** Graduations de distance : pas « rond » donnant 3 à 8 repères. */
  const ticks = $derived.by(() => {
    if (compact || total <= 0) return [];
    const steps = [25, 50, 100, 200, 250, 500, 1000, 2000, 5000];
    const step = steps.find((s) => total / s <= 8) ?? 10000;
    const out: { at: number; label: string }[] = [];
    for (let m = 0; m <= total + 1e-6; m += step) {
      out.push({ at: (m / total) * 100, label: m === 0 ? "0" : m < 1000 ? `${m} m` : `${decimal(m / 1000, m % 1000 ? 1 : 0)} km` });
    }
    return out;
  });

  const pct = (v: number) => `${(v / W) * 100}%`;
  const showLabels = $derived(!compact && segments.length <= 16);
  const label = $derived(
    `Profil de ${segments.length} segment${segments.length > 1 ? "s" : ""}, vitesse maximale ${decimal(maxSpeed, 1)} km/h`,
  );
</script>

<div class="chart" class:compact role="img" aria-label={label}>
  <div class="plot">
    <svg viewBox="0 0 {W} 100" preserveAspectRatio="none" aria-hidden="true">
      <defs>
        {#if doneX !== null}
          <clipPath id="kr-done-{compact ? 'c' : 'e'}">
            <rect x="0" y="0" width={doneX} height="100"></rect>
          </clipPath>
        {/if}
      </defs>
      <rect class="lane" x="0" y={INC_TOP} width={W} height={INC_BOTTOM - INC_TOP}></rect>

      <g class:faded={doneX !== null}>
        {#each bars as b, i (i)}
          <rect
            class="speed"
            class:sel={selected === i}
            x={b.x + GAP / 2}
            y={b.speedY}
            width={Math.max(0.5, b.w - GAP)}
            height={SPEED_BOTTOM - b.speedY}
          ></rect>
          <rect
            class="incline"
            class:sel={selected === i}
            x={b.x + GAP / 2}
            y={b.incY}
            width={Math.max(0.5, b.w - GAP)}
            height={INC_BOTTOM - b.incY}
          ></rect>
        {/each}
      </g>

      {#if doneX !== null}
        <g clip-path="url(#kr-done-{compact ? 'c' : 'e'})">
          {#each bars as b, i (i)}
            <rect class="speed" x={b.x + GAP / 2} y={b.speedY} width={Math.max(0.5, b.w - GAP)} height={SPEED_BOTTOM - b.speedY}></rect>
            <rect class="incline" x={b.x + GAP / 2} y={b.incY} width={Math.max(0.5, b.w - GAP)} height={INC_BOTTOM - b.incY}></rect>
          {/each}
        </g>
        <line class="cursor" x1={doneX} x2={doneX} y1="0" y2="100"></line>
      {/if}

    </svg>

    {#if showLabels}
      {#each bars as b, i (i)}
        {#if b.w >= 40}
          <span class="val speed-val" style:left={pct(b.x)} style:width={pct(b.w)} style:top={`${b.speedY}%`}>
            {decimal(b.seg.speedKmh, 1)}
          </span>
          {#if b.seg.inclinePct > 0}
            <span class="val inc-val" style:left={pct(b.x)} style:width={pct(b.w)} style:top={`${b.incY}%`}>
              {decimal(b.seg.inclinePct, 1)}
            </span>
          {/if}
        {/if}
      {/each}
    {/if}

    {#if !compact}
      <span class="lane-title" style:top="0">km/h</span>
      <span class="lane-title" style:top={`${INC_TOP}%`}>%</span>
    {/if}

    {#if onselect}
      {#each bars as b, i (i)}
        <button
          type="button"
          class="hit"
          tabindex="-1"
          style:left={pct(b.x)}
          style:width={pct(b.w)}
          aria-label={`Segment ${i + 1}`}
          onclick={() => onselect?.(i)}
        ></button>
      {/each}
    {/if}
  </div>

  {#if ticks.length > 0}
    <div class="axis" aria-hidden="true">
      {#each ticks as t (t.at)}
        <span style:left={`${t.at}%`} class:first={t.at === 0} class:last={t.at > 97}>{t.label}</span>
      {/each}
    </div>
  {/if}
</div>

<style>
  .chart {
    height: 100%;
    min-height: 0;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .plot {
    position: relative;
    flex: 1;
    min-height: 0;
  }

  svg {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    overflow: visible;
  }

  .lane {
    fill: var(--kr-track);
  }

  .speed {
    fill: var(--kr-accent);
  }

  .incline {
    fill: var(--kr-text-3);
  }

  .faded .speed,
  .faded .incline {
    opacity: .3;
  }

  /* Éditeur : le segment choisi ressort, les autres s'effacent un peu. */
  .speed.sel {
    fill: var(--kr-accent);
    stroke: var(--kr-text);
    stroke-width: 2px;
    vector-effect: non-scaling-stroke;
  }

  .incline.sel {
    fill: var(--kr-text-2);
  }

  .cursor {
    stroke: var(--kr-text);
    stroke-width: 2px;
    vector-effect: non-scaling-stroke;
  }

  .val {
    position: absolute;
    transform: translateY(-100%);
    padding-bottom: 2px;
    text-align: center;
    font-family: var(--kr-font-num);
    font-weight: 600;
    font-size: 13px;
    font-variant-numeric: tabular-nums;
    color: var(--kr-text-2);
    white-space: nowrap;
    overflow: hidden;
    pointer-events: none;
  }

  .inc-val {
    font-size: 12px;
    color: var(--kr-text-3);
  }

  .lane-title {
    position: absolute;
    font-size: 11px;
    font-weight: 700;
    letter-spacing: .06em;
    color: var(--kr-text-3);
    pointer-events: none;
  }

  .lane-title {
    left: -2px;
    transform: translateX(-100%);
    padding-right: 6px;
  }

  .hit {
    position: absolute;
    top: 0;
    bottom: 0;
    padding: 0;
    border: 0;
    background: transparent;
    cursor: pointer;
  }

  .hit:active {
    transform: none;
  }

  .axis {
    position: relative;
    height: 14px;
    flex-shrink: 0;
    font-size: 12px;
    color: var(--kr-text-3);
  }

  .axis span {
    position: absolute;
    top: 0;
    transform: translateX(-50%);
    white-space: nowrap;
    font-variant-numeric: tabular-nums;
  }

  .axis span.first {
    transform: none;
  }

  .axis span.last {
    transform: translateX(-100%);
  }
</style>
