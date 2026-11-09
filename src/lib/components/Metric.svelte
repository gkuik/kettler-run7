<script lang="ts">
  // Tuile de mesure : libellé et unité, chiffre, ligne secondaire. La tuile est
  // un conteneur CSS : le chiffre suit son token, réduit si la tuile rétrécit
  // (bandeaux d'alerte, petite fenêtre). Ses marges sont sur l'élément intérieur,
  // pour que les unités `cq*` se rapportent à la tuile entière, comme dans le
  // design. Un chiffre porte `.kr-num` : il grise quand la liaison est absente.
  import type { Snippet } from "svelte";

  interface Props {
    label: string;
    value: string;
    unit?: string;
    /** Ligne secondaire (consigne, mention…). */
    sub?: string;
    /** Rangée du bas, chiffres plus petits (--kr-fs-metric-s). */
    small?: boolean;
    /** Pas de mesure (pouls sans contact) : chiffre atténué. */
    muted?: boolean;
    /** Signe placé devant le chiffre (≈, cœur). */
    prefix?: Snippet;
  }

  let { label, value, unit = "", sub = "", small = false, muted = false, prefix }: Props = $props();
</script>

<section class="tile" class:small aria-label={label}>
  <div class="inner">
    <div class="head">
      <span class="kr-label">{label}</span>
      {#if unit}<span class="kr-unit">{unit}</span>{/if}
    </div>
    <div class="value kr-num" class:muted>
      {@render prefix?.()}{value}
    </div>
    <span class="sub">{sub}</span>
  </div>
</section>

<style>
  .tile {
    container-type: size;
    min-width: 0;
    min-height: 0;
    background: var(--kr-surface);
    border-radius: var(--kr-r-md);
  }

  .inner {
    height: 100%;
    padding: var(--kr-sp-3) var(--kr-pad);
    display: flex;
    flex-direction: column;
    justify-content: space-between;
  }

  .head {
    display: flex;
    align-items: baseline;
    gap: var(--kr-sp-2);
    white-space: nowrap;
  }

  .value {
    font-size: min(var(--kr-fs-metric), 42cqh, 28cqi);
    color: var(--kr-text);
    white-space: nowrap;
  }

  .small .value {
    display: flex;
    align-items: center;
    gap: .15em;
    font-size: min(var(--kr-fs-metric-s), 38cqh, 30cqi);
  }

  /* Repli sans container queries (voir app.css) : tuiles du rang « duo »
     (½ largeur, 27 % de hauteur) et « trio » (⅓ largeur, 23 %). */
  @supports not (container-type: size) {
    .value {
      font-size: min(var(--kr-fs-metric), calc(var(--kr-col-h) * .113), calc(var(--kr-col-w) * .14));
    }

    .small .value {
      font-size: min(var(--kr-fs-metric-s), calc(var(--kr-col-h) * .087), calc(var(--kr-col-w) * .1));
    }
  }

  .value.muted {
    color: var(--kr-text-3);
  }

  .sub {
    min-height: 1.2em;
    font-size: var(--kr-fs-sub);
    color: var(--kr-text-3);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
</style>
