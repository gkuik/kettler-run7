<script lang="ts">
  // Réglages de la colonne de droite : vitesse (pas et préréglages), inclinaison,
  // plafond, rappel des raccourcis. Boutons désactivés quand la commande n'aurait
  // pas de sens (non connecté, bande arrêtée, compte à rebours, consigne déjà en
  // butée…) ; le Rust reste seul juge : il borne et refuse lui-même.
  //
  // Les rangées de boutons gardent --kr-btn-h et ne se tassent que si la place
  // manque (bandeaux d'alerte) : rien ne défile à 900×680.
  import { api } from "../api";
  import {
    clampCap,
    inclineStepUseful,
    isConnected,
    nextIncline,
    nextSpeed,
    presetAllowed,
    round1,
    speedStepUseful,
  } from "../controls";
  import { decimal, formatTenths } from "../format";
  import { treadmill } from "../treadmill.svelte";

  const SPEED_STEPS = [-0.5, -0.1, 0.1, 0.5];
  const PRESETS = [3, 4, 5, 6];
  const INCLINE_STEPS = [-0.5, 0.5];
  const CAP_STEP = 0.5;

  const snap = $derived(treadmill.snap);
  const cap = $derived(snap.speedCapKmh);
  const linked = $derived(isConnected(snap));

  const abs = (d: number) => decimal(Math.abs(d), 1);
  const signed = (d: number) => (d > 0 ? "+" : "−") + abs(d);

  function speedStep(d: number) {
    void treadmill.run(() => api.setTargetSpeed(nextSpeed(snap, d)));
  }
  function preset(kmh: number) {
    void treadmill.run(() => api.setTargetSpeed(kmh));
  }
  function inclineStep(d: number) {
    void treadmill.run(() => api.setTargetIncline(nextIncline(snap, d)));
  }
  function capStep(d: number) {
    void treadmill.run(() => api.setSpeedCap(clampCap(cap + d)));
  }
</script>

<div class="settings">
  <div class="group">
    <div class="head">
      <span class="kr-label">Vitesse</span>
      <span class="kr-unit">km/h</span>
    </div>
    <div class="row four">
      {#each SPEED_STEPS as d (d)}
        <button
          type="button"
          class="kr-btn"
          disabled={!speedStepUseful(snap, d)}
          aria-label={`${d < 0 ? "Ralentir" : "Accélérer"} de ${abs(d)} km/h`}
          onclick={() => speedStep(d)}
        >
          {signed(d)}
        </button>
      {/each}
    </div>
    <div class="row four">
      {#each PRESETS as kmh (kmh)}
        {@const active = snap.running === true && round1(snap.targetSpeedKmh ?? -1) === kmh}
        <button
          type="button"
          class="kr-btn preset"
          class:active
          disabled={!presetAllowed(snap, kmh)}
          aria-label={`Préréglage ${kmh} km/h`}
          aria-pressed={active}
          onclick={() => preset(kmh)}
        >
          {kmh}
        </button>
      {/each}
    </div>
  </div>

  <div class="group">
    <div class="head">
      <span class="kr-label">Inclinaison</span>
      <span class="kr-unit">%</span>
    </div>
    <div class="row two">
      {#each INCLINE_STEPS as d (d)}
        <button
          type="button"
          class="kr-btn"
          disabled={!inclineStepUseful(snap, d)}
          aria-label={`${d < 0 ? "Baisser" : "Monter"} l'inclinaison de ${abs(d)} %`}
          onclick={() => inclineStep(d)}
        >
          {signed(d)}
        </button>
      {/each}
    </div>
  </div>

  <div class="group">
    <div class="head">
      <svg
        class="lock"
        width="14"
        height="14"
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        stroke-width="2.4"
        stroke-linecap="round"
        aria-hidden="true"
      >
        <rect x="5" y="11" width="14" height="9" rx="2"></rect>
        <path d="M8 11V8a4 4 0 0 1 8 0v3"></path>
      </svg>
      <span class="kr-label">Plafond</span>
      <span class="kr-unit">km/h</span>
    </div>
    <div class="row cap">
      <button
        type="button"
        class="kr-btn sign"
        disabled={!linked || clampCap(cap - CAP_STEP) === cap}
        aria-label={`Baisser le plafond de ${abs(CAP_STEP)} km/h`}
        onclick={() => capStep(-CAP_STEP)}
      >
        −
      </button>
      <output class="cap-value" aria-label="Plafond de vitesse en km/h">{formatTenths(cap)}</output>
      <button
        type="button"
        class="kr-btn sign"
        disabled={!linked || clampCap(cap + CAP_STEP) === cap}
        aria-label={`Monter le plafond de ${abs(CAP_STEP)} km/h`}
        onclick={() => capStep(CAP_STEP)}
      >
        +
      </button>
    </div>
  </div>

  <div class="keys">
    <kbd>Échap</kbd>
    <span>ou</span>
    <kbd>Espace</kbd>
    <span>arrêt immédiat ·</span>
    <kbd>↑</kbd>
    <kbd>↓</kbd>
    <span>vitesse ±0,1</span>
  </div>
</div>

<style>
  .settings {
    container-type: size;
    flex: 1;
    min-height: 0;
    padding: var(--kr-sp-4);
    background: var(--kr-surface);
    border-radius: var(--kr-r-lg);
    display: flex;
    flex-direction: column;
    justify-content: space-between;
    gap: var(--kr-sp-3);
    /* Filet de sécurité sous 900×680 (fenêtre minimale 600×520). */
    overflow: hidden auto;
  }

  .group {
    flex: 0 1 auto;
    min-height: 0;
    display: flex;
    flex-direction: column;
    gap: var(--kr-sp-2);
  }

  .head {
    flex-shrink: 0;
    display: flex;
    align-items: baseline;
    gap: var(--kr-sp-2);
  }

  .lock {
    align-self: center;
    color: var(--kr-text-3);
  }

  .row {
    flex: 0 1 var(--kr-btn-h);
    min-height: 40px;
    display: grid;
    gap: var(--kr-sp-2);
  }

  .four {
    grid-template-columns: repeat(4, minmax(0, 1fr));
  }

  .two {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }

  .cap {
    grid-template-columns: minmax(0, 1fr) minmax(0, 1.1fr) minmax(0, 1fr);
    align-items: stretch;
  }

  .preset {
    font-size: calc(var(--kr-fs-btn) * 1.15);
  }

  .kr-btn.active,
  .kr-btn.active:hover:not(:disabled) {
    background: var(--kr-accent);
    color: var(--kr-accent-ink);
    border-color: var(--kr-accent);
  }

  .sign {
    font-size: calc(var(--kr-fs-btn) * 1.3);
  }

  .cap-value {
    align-self: center;
    text-align: center;
    font-family: var(--kr-font-num);
    font-weight: 600;
    font-size: calc(var(--kr-fs-btn) * 1.45);
    font-variant-numeric: tabular-nums;
    color: var(--kr-text);
  }

  .keys {
    flex-shrink: 0;
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--kr-sp-1) 6px;
    font-size: var(--kr-fs-hint);
    line-height: 1.5;
    color: var(--kr-text-3);
  }

  /* Bandeaux empilés : les commandes passent avant le rappel des raccourcis. */
  @container (max-height: 326px) {
    .keys {
      display: none;
    }
  }

  /* Plus bas encore (sous 900×680) : hauteur minimale, la carte défile. */
  @container (max-height: 270px) {
    .group,
    .row {
      flex: none;
    }

    .row {
      height: 40px;
    }
  }

  kbd {
    font-family: var(--kr-font-ui);
    font-size: 12px;
    font-weight: 700;
    padding: 1px 6px;
    border-radius: 5px;
    border: 1px solid var(--kr-line);
    background: var(--kr-surface-2);
    color: var(--kr-text-2);
  }
</style>
