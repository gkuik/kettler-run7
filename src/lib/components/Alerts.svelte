<script lang="ts">
  // Bandeaux d'alerte, sous l'en-tête : liaison perdue bande en marche (rouge
  // clignotant, le seul clignotement de l'interface), échec d'un arrêt
  // (« STOP NON ENVOYÉ », persistant, affiché même sans bouton d'arrêt), arrêt
  // mémorisé pendant une reconnexion (visible lui aussi sans bouton), reconnexion
  // en cours. Plusieurs peuvent coexister : ils se resserrent alors pour laisser
  // de la place aux commandes.
  import { treadmill } from "../treadmill.svelte";
  import Spinner from "./Spinner.svelte";

  const snap = $derived(treadmill.snap);
  const lost = $derived(snap.lostWhileRunning);
  const stopError = $derived(treadmill.stopError);
  const memo = $derived(!stopError && treadmill.stopPending);
  // L'arrêt mémorisé annonce déjà la reconnexion : pas de doublon.
  const reconnecting = $derived(snap.connection === "reconnecting" && !lost && !memo);
  const count = $derived([lost, !!stopError, memo, reconnecting].filter(Boolean).length);
</script>

{#snippet warnIcon()}
  <svg
    width="30"
    height="30"
    viewBox="0 0 24 24"
    fill="none"
    stroke="currentColor"
    stroke-width="2.2"
    stroke-linecap="round"
    stroke-linejoin="round"
    aria-hidden="true"
  >
    <circle cx="12" cy="12" r="9"></circle>
    <path d="M12 7v5l3 2"></path>
  </svg>
{/snippet}

{#snippet progress()}
  <div class="progress">
    <Spinner />
    Reconnexion…
  </div>
{/snippet}

{#if count > 0}
  <div class="banners" class:tight={count > 1}>
    {#if lost}
      <div class="banner critical kr-alerte" role="alert">
        <svg
          width="32"
          height="32"
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
        <div class="text">
          <div class="title">Connexion perdue : la bande continue de tourner —</div>
          <div class="sub">utilisez le bouton STOP de la console ou la clé</div>
        </div>
        {@render progress()}
      </div>
    {/if}

    {#if stopError}
      <div class="banner stop" role="alert">
        <svg
          width="30"
          height="30"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          stroke-width="2.2"
          stroke-linecap="round"
          stroke-linejoin="round"
          aria-hidden="true"
        >
          <rect x="3" y="3" width="18" height="18" rx="4"></rect>
          <path d="M8.5 8.5l7 7M15.5 8.5l-7 7"></path>
        </svg>
        <div class="text">
          <div class="title spaced">STOP NON ENVOYÉ</div>
          <div class="sub">{stopError}</div>
        </div>
        <button
          type="button"
          class="dismiss"
          aria-label="Fermer l'alerte"
          onclick={() => treadmill.dismissStopError()}
        >
          <svg
            width="20"
            height="20"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="2.6"
            stroke-linecap="round"
            aria-hidden="true"
          >
            <path d="M6 6l12 12M18 6L6 18"></path>
          </svg>
        </button>
      </div>
    {:else if memo}
      <div class="banner warn" role="status">
        {@render warnIcon()}
        <div class="text">
          <div class="title">Arrêt mémorisé, envoyé dès la reconnexion</div>
          <div class="sub">En attendant, la console et la clé arrêtent la bande immédiatement.</div>
        </div>
        {@render progress()}
      </div>
    {/if}

    {#if reconnecting}
      <div class="banner warn" role="status">
        {@render warnIcon()}
        <div class="text">
          <div class="title">Liaison perdue, reconnexion en cours</div>
          <div class="sub">Les valeurs affichées sont figées.</div>
        </div>
        {@render progress()}
      </div>
    {/if}
  </div>
{/if}

<style>
  .banners {
    flex-shrink: 0;
    display: flex;
    flex-direction: column;
    gap: var(--kr-sp-3);
    padding: var(--kr-pad) var(--kr-pad) 0;
  }

  .banner {
    min-height: 64px;
    display: flex;
    align-items: center;
    gap: var(--kr-sp-4);
    padding: 10px var(--kr-sp-4);
    border-radius: var(--kr-r-md);
  }

  .banner > :global(svg) {
    flex-shrink: 0;
  }

  .critical {
    background: var(--kr-stop);
    color: var(--kr-stop-ink);
    box-shadow: 0 0 0 3px var(--kr-stop-deep);
  }

  .stop {
    padding-right: 10px;
    background: var(--kr-stop);
    color: var(--kr-stop-ink);
  }

  .warn {
    background: var(--kr-warn);
    color: var(--kr-warn-ink);
  }

  .text {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .title {
    font-size: var(--kr-fs-banner);
    font-weight: 700;
    line-height: 1.2;
  }

  .title.spaced {
    letter-spacing: .04em;
  }

  .sub {
    font-size: var(--kr-fs-sub);
    font-weight: 600;
    line-height: 1.25;
  }

  .progress {
    flex-shrink: 0;
    display: flex;
    align-items: center;
    gap: var(--kr-sp-2);
    font-size: var(--kr-fs-sub);
    font-weight: 600;
    white-space: nowrap;
  }

  .dismiss {
    width: 44px;
    height: 44px;
    flex-shrink: 0;
    padding: 0;
    border: 0;
    border-radius: var(--kr-r-sm);
    background: rgba(255, 255, 255, .16);
    color: var(--kr-stop-ink);
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .dismiss:hover {
    background: rgba(255, 255, 255, .26);
  }

  /* Plusieurs bandeaux : hauteur réduite, le rouge critique reste en tête. */
  .tight {
    gap: var(--kr-sp-2);
  }

  .tight .banner {
    min-height: 0;
    gap: var(--kr-sp-3);
    padding-block: 6px;
  }
</style>
