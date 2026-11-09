<script lang="ts">
  // Le bouton d'arrêt : PAUSE (ambre, pleine largeur) quand la bande tourne, STOP
  // (rouge, à côté de « Reprendre ») sinon, rien quand il n'y a rien à arrêter
  // (voir `stopButtonMode`). Les deux envoient le même arrêt (Online = 1) : c'est
  // le tapis qui fait d'un premier arrêt une pause et d'un second la fin de séance.
  // - Affiché, il n'est JAMAIS désactivé ; pendant une reconnexion (état réel
  //   inconnu), il est toujours affiché, et le Rust mémorise l'arrêt pour
  //   l'envoyer dès la liaison rétablie.
  // - Échec de l'arrêt (« STOP NON ENVOYÉ ») et arrêt mémorisé : bandeaux en
  //   haut de l'écran (Alerts.svelte), affichés même sans bouton.
  // - Échap et Espace envoient l'arrêt quel que soit le libellé (App.svelte).
  import { stopButtonMode } from "../controls";
  import { treadmill } from "../treadmill.svelte";

  const mode = $derived(stopButtonMode(treadmill.snap, treadmill.pauseRequested));

  function stop() {
    void treadmill.stop();
  }
</script>

{#if mode === "pause"}
  <button
    type="button"
    class="kr-primary pause"
    onclick={stop}
    aria-keyshortcuts="Escape Space"
    aria-label="Pause : arrêter la bande"
  >
    <svg width="26" height="26" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">
      <rect x="5" y="4" width="5" height="16" rx="1.5"></rect>
      <rect x="14" y="4" width="5" height="16" rx="1.5"></rect>
    </svg>
    PAUSE
  </button>
{:else if mode === "stop"}
  <button
    type="button"
    class="kr-primary stop"
    onclick={stop}
    aria-keyshortcuts="Escape Space"
    aria-label="Stop : terminer la séance"
  >
    <span class="main">
      <svg width="20" height="20" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">
        <rect x="5" y="5" width="14" height="14" rx="2"></rect>
      </svg>
      STOP
    </span>
    <span class="sub">Terminer la séance</span>
  </button>
{/if}

<style>
  .pause {
    gap: 14px;
    background: var(--kr-pause);
    color: var(--kr-pause-ink);
    letter-spacing: .06em;
  }

  .stop {
    flex-direction: column;
    gap: var(--kr-sp-1);
    padding: 0 var(--kr-sp-1);
    background: var(--kr-stop);
    color: var(--kr-stop-ink);
  }

  .main {
    display: flex;
    align-items: center;
    gap: var(--kr-sp-2);
    font-size: var(--kr-fs-btn);
    letter-spacing: .08em;
  }

  .sub {
    font-size: 14px;
    font-weight: 600;
    line-height: 1.15;
  }
</style>
