<script lang="ts">
  // Carte de la vitesse : puce d'état de la bande, vitesse réelle, consigne et
  // jauge, ou écran de démarrage pendant le compte à rebours.
  //
  // Le tapis ne donne pas le temps restant : l'anneau tourne sans chiffre, un
  // décompte estimé pourrait annoncer le départ de la bande trop tard.
  import { isPaused, isStarting, speedScaleMax, speedTrend } from "../controls";
  import { NONE, formatTenths } from "../format";
  import { treadmill } from "../treadmill.svelte";

  const snap = $derived(treadmill.snap);
  const linked = $derived(snap.connection === "connected");
  const reconnecting = $derived(snap.connection === "reconnecting");
  const paused = $derived(isPaused(snap));

  const starting = $derived(
    linked && (isStarting(snap) || (treadmill.startPending && snap.running === false)),
  );

  const chip = $derived.by(() => {
    if (reconnecting) {
      return {
        tone: snap.lostWhileRunning ? "stop" : "warn",
        label: "Dernières valeurs reçues",
      };
    }
    if (!linked) return { tone: "off", label: "Tapis non connecté" };
    if (snap.running === null) return { tone: "off", label: "Lecture de l'état…" };
    if (snap.running) return { tone: "go", label: "En marche" };
    if (paused) return { tone: "pause", label: "En pause" };
    return { tone: "off", label: "Bande arrêtée" };
  });

  // À l'arrêt, le tapis remet sa consigne à 0 : il n'y a pas de consigne à montrer.
  const target = $derived(
    snap.running === true && (snap.targetSpeedKmh ?? 0) > 0 ? snap.targetSpeedKmh : null,
  );

  const rampe = $derived.by(() => {
    if (!linked || snap.running === null) return "";
    if (!snap.running) return paused ? "définie à la reprise" : "définie au démarrage";
    if (target === null) return "";
    const trend = speedTrend(snap);
    if (trend === "up") return `accélère vers ${formatTenths(target)}`;
    if (trend === "down") return `ralentit vers ${formatTenths(target)}`;
    return "consigne atteinte";
  });

  const scaleMax = $derived(speedScaleMax(snap));
  const pct = (kmh: number) => `${Math.max(0, Math.min(100, (kmh / scaleMax) * 100)).toFixed(2)}%`;
</script>

<div class="kr-hero">
  <div class="inner">
    {#if starting}
      <div class="countdown">
        <div class="ring">
          <svg class="kr-spin" viewBox="0 0 120 120" aria-hidden="true">
            <circle class="ring-track" cx="60" cy="60" r="54"></circle>
            <circle class="ring-arc" cx="60" cy="60" r="54"></circle>
          </svg>
          <svg
            class="ring-icon"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="2.2"
            stroke-linecap="round"
            stroke-linejoin="round"
            aria-hidden="true"
          >
            <path d="M12 3l10 18H2z"></path>
            <path d="M12 10v4.5"></path>
            <path d="M12 17.6v.1"></path>
          </svg>
        </div>
        <div class="countdown-text" role="status">
          <div class="kr-label countdown-label">Démarrage…</div>
          <div class="countdown-title">Restez sur les rebords</div>
          <div class="countdown-sub">Les réglages sont ignorés pendant le décompte.</div>
        </div>
      </div>
    {:else}
      <div class="top">
        <div class="head">
          <span class="kr-label">Vitesse</span>
          <span class="kr-unit">km/h</span>
        </div>
        <div class="chip">
          <span class="dot {chip.tone}"></span>{chip.label}
        </div>
      </div>

      <div class="speeds">
        <div class="kr-num speed">{formatTenths(snap.speedKmh)}</div>
        <div class="target">
          <span class="kr-label target-label">Consigne</span>
          <div class="target-value" class:live={linked && target !== null}>
            {target === null ? NONE : formatTenths(target)}
          </div>
          <span class="rampe">{rampe}</span>
        </div>
      </div>

      <div class="gauge">
        <div class="bar">
          <div class="track">
            <div class="over" style:left={pct(snap.speedCapKmh)}></div>
            <div class="fill" class:live={linked} style:width={pct(snap.speedKmh ?? 0)}></div>
          </div>
          {#if target !== null}
            <div class="tick" style:left={pct(target)}></div>
          {/if}
        </div>
        <div class="scale">
          <span>0</span>
          <span class="cap" style:left={`clamp(60px, ${pct(snap.speedCapKmh)}, calc(100% - 60px))`}>
            plafond {formatTenths(snap.speedCapKmh)}
          </span>
          <span class="scale-max">{scaleMax}</span>
        </div>
      </div>
    {/if}
  </div>
</div>

<style>
  .kr-hero {
    flex: 50 1 0;
    min-height: 0;
    background: var(--kr-surface);
    border-radius: var(--kr-r-lg);
    overflow: hidden;
    --ring: min(74cqh, 42cqi);
  }

  /* Repli sans container queries (voir app.css) : carte = toute la largeur
     de la colonne, 50 % de sa hauteur. */
  @supports not (container-type: size) {
    .kr-hero {
      --ring: min(calc(var(--kr-col-h) * .37), calc(var(--kr-col-w) * .42));
    }

    .target-value {
      font-size: min(var(--kr-fs-metric), calc(var(--kr-col-h) * .15));
    }
  }

  /* Les marges sont ici : `cqi`/`cqh` se rapportent à la carte entière. */
  .inner {
    height: 100%;
    padding: var(--kr-pad) calc(var(--kr-pad) + 6px);
    display: flex;
    flex-direction: column;
    gap: var(--kr-sp-3);
  }

  /* Plusieurs bandeaux empilés : la carte devient basse, on resserre. */
  @container (max-height: 220px) {
    .inner {
      padding-block: var(--kr-sp-2);
      gap: var(--kr-sp-1);
    }

    .kr-hero .speed {
      font-size: min(var(--kr-fs-hero), 30cqi, 52cqh);
    }
  }

  .top {
    flex-shrink: 0;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--kr-sp-3);
  }

  .head {
    display: flex;
    align-items: baseline;
    gap: var(--kr-sp-2);
  }

  .chip {
    display: inline-flex;
    align-items: center;
    gap: var(--kr-sp-2);
    height: 30px;
    padding: 0 var(--kr-sp-3);
    border-radius: var(--kr-r-pill);
    background: var(--kr-surface-2);
    font-size: var(--kr-fs-label);
    font-weight: 600;
    color: var(--kr-text-2);
    white-space: nowrap;
  }

  .dot {
    width: 10px;
    height: 10px;
    border-radius: 50%;
    flex-shrink: 0;
    background: var(--kr-text-3);
    transition: background-color var(--kr-dur) var(--kr-ease);
  }

  .dot.go {
    background: var(--kr-go);
  }
  .dot.pause {
    background: var(--kr-pause);
  }
  .dot.warn {
    background: var(--kr-warn);
  }
  .dot.stop {
    background: var(--kr-stop);
  }

  .speeds {
    flex: 1;
    min-height: 0;
    display: flex;
    align-items: flex-end;
    justify-content: space-between;
    gap: var(--kr-sp-5);
  }

  .speed {
    color: var(--kr-text);
    line-height: .8;
    letter-spacing: -.02em;
    white-space: nowrap;
    /* La virgule descend sous la ligne : elle ne doit pas toucher la jauge. */
    padding-bottom: .06em;
  }

  .target {
    display: flex;
    flex-direction: column;
    align-items: flex-end;
    gap: 6px;
    min-width: 0;
    padding-bottom: var(--kr-sp-1);
  }

  .target-label {
    color: var(--kr-text-3);
  }

  .target-value {
    font-family: var(--kr-font-num);
    font-weight: 600;
    font-size: min(var(--kr-fs-metric), 30cqh);
    font-variant-numeric: tabular-nums;
    line-height: .85;
    color: var(--kr-digit-off);
  }

  .target-value.live {
    color: var(--kr-accent);
  }

  .rampe {
    min-height: 1.2em;
    font-size: var(--kr-fs-sub);
    color: var(--kr-text-3);
    white-space: nowrap;
  }

  .gauge {
    flex-shrink: 0;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .bar {
    position: relative;
    height: 12px;
  }

  .track {
    position: absolute;
    inset: 0;
    border-radius: var(--kr-r-pill);
    background: var(--kr-track);
    overflow: hidden;
  }

  /* Zone interdite, au-delà du plafond. */
  .over {
    position: absolute;
    top: 0;
    bottom: 0;
    right: 0;
    background: repeating-linear-gradient(
      135deg,
      transparent 0 5px,
      var(--kr-line) 5px 8px
    );
  }

  .fill {
    position: absolute;
    left: 0;
    top: 0;
    bottom: 0;
    border-radius: var(--kr-r-pill);
    background: var(--kr-digit-off);
    transition: width 300ms var(--kr-ease);
  }

  .fill.live {
    background: var(--kr-accent);
  }

  .tick {
    position: absolute;
    top: -5px;
    height: 22px;
    width: 4px;
    margin-left: -2px;
    border-radius: 2px;
    background: var(--kr-text);
    transition: left var(--kr-dur) var(--kr-ease);
  }

  .scale {
    position: relative;
    height: 16px;
    font-size: var(--kr-fs-hint);
    color: var(--kr-text-3);
  }

  .cap {
    position: absolute;
    top: 0;
    white-space: nowrap;
    transform: translateX(-50%);
  }

  .scale-max {
    position: absolute;
    right: 0;
    top: 0;
  }

  /* Compte à rebours */
  .countdown {
    flex: 1;
    min-height: 0;
    display: flex;
    align-items: center;
    gap: calc(var(--kr-pad) * 1.5);
  }

  .ring {
    position: relative;
    width: var(--ring);
    height: var(--ring);
    flex-shrink: 0;
  }

  .ring .kr-spin {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    transform-origin: center;
  }

  .ring-track,
  .ring-arc {
    fill: none;
    stroke-width: 8;
  }

  .ring-track {
    stroke: var(--kr-track);
  }

  .ring-arc {
    stroke: var(--kr-pause);
    stroke-linecap: round;
    stroke-dasharray: 85 339.3;
  }

  .ring-icon {
    position: absolute;
    inset: 27%;
    color: var(--kr-pause);
  }

  .countdown-text {
    display: flex;
    flex-direction: column;
    gap: var(--kr-sp-2);
    min-width: 0;
  }

  .countdown-label {
    color: var(--kr-pause);
  }

  .countdown-title {
    font-family: var(--kr-font-num);
    font-size: calc(var(--kr-fs-metric) * .78);
    font-weight: 600;
    line-height: 1.02;
    color: var(--kr-text);
    text-wrap: balance;
  }

  .countdown-sub {
    font-size: var(--kr-fs-sub);
    line-height: 1.4;
    color: var(--kr-text-3);
    text-wrap: pretty;
  }
</style>
