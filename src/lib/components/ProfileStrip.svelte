<script lang="ts">
  // Progression du profil de course, dans la colonne des mesures (affichée
  // seulement quand un profil est choisi) : segment en cours et ses consignes,
  // mètres restants, aperçu avec la partie parcourue, et le changement suivant,
  // annoncé en orange dans les derniers mètres du segment.
  //
  // La distance du profil (Rust) avance par à-coups comme celle du tapis : on
  // lui ajoute l'avance de la distance lissée affichée (≤ 40 m).
  import { decimal } from "../format";
  import { RECOVERY_SPEED_KMH, formatDuration, formatProfileDistance, summarize } from "../profiles";
  import { profiles } from "../profiles.svelte";
  import { treadmill } from "../treadmill.svelte";
  import type { ProfileSegment } from "../types";
  import ProfileChart from "./ProfileChart.svelte";

  /** En deçà, le changement de segment est annoncé (mètres), sans dépasser 40 % du segment. */
  const ANNOUNCE_M = 60;
  const ANNOUNCE_SHARE = 0.4;

  const snap = $derived(treadmill.snap);
  const p = $derived(snap.profile);
  const segments = $derived(profiles.find(p?.profileId)?.segments ?? []);
  const linked = $derived(snap.connection === "connected");
  const running = $derived(linked && snap.running === true);

  const lead = $derived(
    running && snap.distanceM !== null && treadmill.shownDistanceM !== null
      ? Math.max(0, treadmill.shownDistanceM - snap.distanceM)
      : 0,
  );
  const doneM = $derived(p ? Math.min(p.totalM, p.doneM + lead) : 0);
  const remainingM = $derived(p ? Math.max(0, p.segmentRemainingM - lead) : 0);
  const started = $derived(!!p && (p.doneM > 0 || running));
  const announce = $derived(
    !!p &&
      running &&
      !p.finished &&
      p.next !== null &&
      remainingM <= Math.min(ANNOUNCE_M, p.current.distanceM * ANNOUNCE_SHARE),
  );
  /** Vitesse réellement envoyée : le Rust plafonne (plafond et vitesse max du tapis). */
  const ceiling = $derived(Math.min(snap.speedCapKmh, snap.limits?.speedMaxKmh ?? Infinity));
  const capped = (s: ProfileSegment) => s.speedKmh > ceiling + 1e-6;

  const summary = $derived(summarize(segments));
</script>

{#snippet consigne(s: ProfileSegment)}
  <span class:capped={capped(s)} title={capped(s) ? `${decimal(s.speedKmh, 1)} km/h prévus, ramenés au plafond` : undefined}
    >{decimal(Math.min(s.speedKmh, ceiling), 1)}</span
  ><small>km/h</small>&nbsp; {decimal(s.inclinePct, 1)}<small>%</small>
{/snippet}

{#if p}
  <section class="strip" class:announce aria-label="Profil de course">
    <div class="now">
      <div class="head">
        <span class="kr-label">Profil</span>
        <span class="name">{p.profileName}</span>
        {#if started && !p.finished}
          <span class="seg">{p.segmentIndex + 1}/{p.segmentCount}</span>
        {/if}
      </div>
      {#if p.finished}
        <div class="big">Terminé</div>
        <div class="sub">récupération en cours</div>
      {:else if started}
        <div class="big">{@render consigne(p.current)}</div>
        <div class="sub">
          {#if p.manual}
            <span class="manual">réglage manuel</span> · encore {Math.round(remainingM)} m
          {:else if capped(p.current)}
            <span class="capped">plafonné</span> · encore {Math.round(remainingM)} m
          {:else}
            encore {Math.round(remainingM)} m
          {/if}
        </div>
      {:else}
        <div class="big">{formatProfileDistance(p.totalM)} · {formatDuration(summary.durationS)}</div>
        <div class="sub">{p.segmentCount} segments · pilote après le décompte</div>
      {/if}
    </div>

    <div class="chart">
      <ProfileChart {segments} doneM={started ? doneM : null} capKmh={snap.speedCapKmh} compact />
    </div>

    <div class="next" role="status">
      {#if p.finished}
        <span class="kr-label">Récupération</span>
        <span class="next-val">{@render consigne(p.current)}</span>
        <span class="sub">pas d'arrêt automatique</span>
      {:else if !started}
        <span class="kr-label">Au départ</span>
        <span class="next-val">{@render consigne(p.current)}</span>
        <span class="sub">sur {formatProfileDistance(p.current.distanceM)}</span>
      {:else if p.next}
        <span class="kr-label">{announce ? `Dans ${Math.round(remainingM)} m` : "Ensuite"}</span>
        <span class="next-val">{@render consigne(p.next)}</span>
        <span class="sub">sur {formatProfileDistance(p.next.distanceM)}</span>
      {:else}
        <span class="kr-label">Dernier segment</span>
        <span class="next-val">puis {decimal(Math.min(RECOVERY_SPEED_KMH, p.current.speedKmh), 1)} km/h</span>
        <span class="sub">récupération, 0 %</span>
      {/if}
      {#if p.manual && !p.finished}
        <span class="sub">le profil reprend au segment suivant</span>
      {/if}
    </div>
  </section>
{/if}

<style>
  .strip {
    flex: 0 0 clamp(76px, 12vh, 104px);
    min-height: 0;
    display: grid;
    grid-template-columns: minmax(0, 31%) minmax(0, 1fr) minmax(0, 27%);
    gap: var(--kr-sp-4);
    align-items: stretch;
    padding: var(--kr-sp-3) var(--kr-pad);
    background: var(--kr-surface);
    border-radius: var(--kr-r-md);
    border: 2px solid transparent;
    transition: border-color var(--kr-dur) var(--kr-ease);
  }

  .strip.announce {
    border-color: var(--kr-warn);
  }

  .now,
  .next {
    min-width: 0;
    display: flex;
    flex-direction: column;
    justify-content: space-between;
    gap: 2px;
  }

  .head {
    display: flex;
    align-items: baseline;
    gap: var(--kr-sp-2);
    min-width: 0;
  }

  .name {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: var(--kr-fs-label);
    font-weight: 700;
    color: var(--kr-text);
  }

  .big,
  .next-val {
    font-family: var(--kr-font-num);
    font-weight: 600;
    font-size: clamp(18px, 2.8vh, 24px);
    font-variant-numeric: tabular-nums;
    color: var(--kr-text);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .seg {
    margin-left: auto;
    flex-shrink: 0;
    font-family: var(--kr-font-num);
    font-weight: 600;
    font-size: var(--kr-fs-label);
    font-variant-numeric: tabular-nums;
    color: var(--kr-accent);
  }

  small {
    margin-left: 2px;
    font-family: var(--kr-font-ui);
    font-size: .58em;
    font-weight: 600;
    color: var(--kr-text-3);
  }

  .sub {
    font-size: 13px;
    color: var(--kr-text-3);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .manual {
    font-weight: 700;
    color: var(--kr-text-2);
  }

  .capped {
    color: var(--kr-warn);
  }

  .sub .capped {
    font-weight: 700;
  }

  .chart {
    min-width: 0;
    min-height: 0;
    padding: 2px 0;
  }

  .announce .next .kr-label,
  .announce .next-val {
    color: var(--kr-warn);
  }
</style>
