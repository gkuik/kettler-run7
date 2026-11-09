<script lang="ts">
  // Menu discret de l'en-tête, mode navigateur uniquement : provoque les incidents
  // du tapis simulé pour vérifier l'interface (perte de liaison, retrait de
  // clé…). Il s'ouvre par-dessus l'écran, sans rien décaler.
  import { mock } from "../api";
  import { treadmill } from "../treadmill.svelte";

  const connected = $derived(treadmill.snap.connection === "connected");
</script>

<details class="sim">
  <summary title="Hors de l'application native : tapis simulé">Simulation</summary>
  <div class="actions">
    <button type="button" disabled={!connected} onclick={() => mock().simulateLinkLoss()}>
      Couper la liaison
    </button>
    <button type="button" disabled={!connected} onclick={() => mock().pullSafetyKey()}>
      Retirer la clé
    </button>
    <button type="button" disabled={!connected} onclick={() => mock().failNextStopWrite()}>
      Faire échouer le prochain arrêt
    </button>
    <button type="button" onclick={() => mock().togglePulse()}>Mains sur les capteurs</button>
  </div>
</details>

<style>
  .sim {
    position: relative;
    flex-shrink: 0;
    white-space: nowrap;
  }

  summary {
    cursor: pointer;
    list-style: none;
    padding: 3px 10px;
    border: 1px solid var(--kr-line);
    border-radius: var(--kr-r-pill);
    font-size: var(--kr-fs-hint);
    font-weight: 600;
    color: var(--kr-text-3);
  }

  summary::-webkit-details-marker {
    display: none;
  }

  .actions {
    position: absolute;
    top: calc(100% + 6px);
    right: 0;
    z-index: 10;
    display: flex;
    flex-direction: column;
    gap: var(--kr-sp-1);
    padding: var(--kr-sp-2);
    background: var(--kr-surface);
    border: 1px solid var(--kr-line);
    border-radius: var(--kr-r-md);
    box-shadow: 0 8px 24px rgba(0, 0, 0, .18);
  }

  button {
    min-height: 40px;
    padding: 0 var(--kr-sp-3);
    border: 0;
    border-radius: var(--kr-r-sm);
    background: var(--kr-surface-2);
    color: var(--kr-text);
    font-size: var(--kr-fs-sub);
    font-weight: 600;
    text-align: left;
  }

  button:hover:not(:disabled) {
    background: var(--kr-surface-3);
  }
</style>
