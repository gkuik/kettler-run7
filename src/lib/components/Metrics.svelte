<script lang="ts">
  // Colonne de gauche : vitesse 50 %, temps / distance 27 %, inclinaison /
  // calories / pouls 23 %. Toutes les valeurs viennent de l'instantané.
  import { isPaused } from "../controls";
  import { NONE, formatDistance, formatElapsed, formatEnergy, formatPulse, formatTenths } from "../format";
  import { treadmill } from "../treadmill.svelte";
  import Metric from "./Metric.svelte";
  import SpeedCard from "./SpeedCard.svelte";
  import Toasts from "./Toasts.svelte";

  const snap = $derived(treadmill.snap);

  const distance = $derived(formatDistance(treadmill.shownDistanceM ?? snap.distanceM));
  const energy = $derived(formatEnergy(snap.energyKcal));
  const pulse = $derived(formatPulse(snap.pulseBpm));

  const inclineSub = $derived(
    snap.targetInclinePct === null ? "" : `consigne ${formatTenths(snap.targetInclinePct)} %`,
  );

  const pulseSub = $derived.by(() => {
    if (pulse !== NONE) return "capteurs des poignées";
    return snap.connection === "connected" ? "sans contact" : "";
  });
</script>

<section class="metrics" aria-label="Mesures">
  <SpeedCard />

  <div class="row duo">
    <Metric
      label="Temps"
      value={formatElapsed(snap.elapsedS)}
      sub={isPaused(snap) ? "chrono arrêté" : ""}
    />
    <Metric label="Distance" value={distance.value} unit={distance.unit || "m"} />
  </div>

  <div class="row trio">
    <Metric small label="Inclinaison" value={formatTenths(snap.inclinePct)} unit="%" sub={inclineSub} />
    <Metric
      small
      label="Calories"
      value={energy.replace("≈ ", "")}
      unit="kcal"
      sub="unité non confirmée"
    >
      {#snippet prefix()}
        <span class="approx">≈</span>
      {/snippet}
    </Metric>
    <Metric small label="Pouls" value={pulse} unit="bpm" sub={pulseSub} muted={pulse === NONE}>
      {#snippet prefix()}
        <svg
          class="heart"
          viewBox="0 0 24 24"
          fill="currentColor"
          aria-hidden="true"
        >
          <path
            d="M12 21s-7.5-4.6-9.3-9.6C1.5 8 3.6 4.5 7.2 4.5c2 0 3.6 1.1 4.8 2.8 1.2-1.7 2.8-2.8 4.8-2.8 3.6 0 5.7 3.5 4.5 6.9C19.5 16.4 12 21 12 21z"
          ></path>
        </svg>
      {/snippet}
    </Metric>
  </div>

  <Toasts />
</section>

<style>
  .metrics {
    position: relative;
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: var(--kr-gap);
  }

  .row {
    min-height: 0;
    display: grid;
    gap: var(--kr-gap);
  }

  .duo {
    flex: 27 1 0;
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }

  .trio {
    flex: 23 1 0;
    grid-template-columns: repeat(3, minmax(0, 1fr));
  }

  .approx {
    font-size: .6em;
    color: var(--kr-text-3);
  }

  .heart {
    width: .5em;
    height: .5em;
    flex-shrink: 0;
    opacity: .55;
  }
</style>
