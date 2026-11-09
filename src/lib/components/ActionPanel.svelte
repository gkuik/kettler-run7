<script lang="ts">
  // Action principale, en haut de la colonne de droite, selon l'état :
  // - déconnecté → « Connecter » ; recherche, connexion → indicateur d'attente ;
  // - connecté, bande arrêtée sans séance → « Démarrer » ;
  // - bande en marche (compte à rebours compris) → PAUSE ;
  // - séance en pause, pause envoyée, reconnexion → « Reprendre » et STOP.
  // Pendant une reconnexion, le bouton d'arrêt reste affiché (PAUSE ou STOP selon
  // `stopButtonMode`) ; « Reprendre » n'est actif que si `canStart` l'autorise.
  import { api } from "../api";
  import { canStart, isPaused, stopButtonMode } from "../controls";
  import { treadmill } from "../treadmill.svelte";
  import Spinner from "./Spinner.svelte";
  import StopButton from "./StopButton.svelte";

  const snap = $derived(treadmill.snap);
  const connection = $derived(snap.connection);
  const stopMode = $derived(stopButtonMode(snap, treadmill.pauseRequested));
  const startDisabled = $derived(!canStart(snap) || treadmill.startPending);

  const resumeHint = $derived.by(() => {
    if (connection === "reconnecting") return "STOP reste actif pendant la reconnexion.";
    if (isPaused(snap)) return "Reprendre : clé en place, pieds sur les rebords.";
    return "Pause envoyée · un 2e appui termine la séance.";
  });

  function start() {
    void treadmill.start();
  }
</script>

{#snippet playIcon(size: number)}
  <svg width={size} height={size} viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">
    <path d="M7 4.5v15l13-7.5z"></path>
  </svg>
{/snippet}

<div class="panel">
  {#if stopMode === "pause"}
    <StopButton />
  {:else if stopMode === "stop"}
    <div class="pair">
      <button type="button" class="kr-primary go resume" disabled={startDisabled} onclick={start}>
        {@render playIcon(24)}
        {treadmill.startPending ? "Démarrage…" : "Reprendre"}
      </button>
      <StopButton />
    </div>
    <p class="hint strong">{resumeHint}</p>
  {:else if connection === "disconnected"}
    <button
      type="button"
      class="kr-primary connect"
      onclick={() => void treadmill.run(api.connect)}
    >
      <svg
        width="28"
        height="28"
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        stroke-width="2.2"
        stroke-linecap="round"
        stroke-linejoin="round"
        aria-hidden="true"
      >
        <path d="M7 7l10 10-5 5V2l5 5L7 17"></path>
      </svg>
      Connecter
    </button>
    <p class="hint">Allumez le tapis, puis connectez-le en Bluetooth.</p>
  {:else if connection !== "connected"}
    <button type="button" class="kr-primary busy" disabled>
      <span class="busy-icon"><Spinner size={24} /></span>
      {connection === "scanning" ? "Recherche du tapis…" : "Connexion…"}
    </button>
    <p class="hint">
      {connection === "scanning"
        ? "Le tapis doit être allumé et à portée."
        : "Tapis trouvé, établissement de la liaison."}
    </p>
  {:else}
    <button type="button" class="kr-primary go" disabled={startDisabled} onclick={start}>
      {@render playIcon(26)}
      {treadmill.startPending ? "Démarrage…" : "Démarrer"}
    </button>
    <p class="hint strong">Clé en place, pieds sur les rebords.</p>
  {/if}
</div>

<style>
  .panel {
    flex-shrink: 0;
    display: flex;
    flex-direction: column;
    gap: var(--kr-sp-2);
    padding: var(--kr-sp-3);
    background: var(--kr-surface);
    border-radius: var(--kr-r-lg);
  }

  .pair {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: var(--kr-sp-2);
  }

  .connect {
    background: var(--kr-accent);
    color: var(--kr-accent-ink);
  }

  .go {
    background: var(--kr-go);
    color: var(--kr-go-ink);
  }

  .resume {
    flex-direction: column;
    gap: 6px;
    font-size: var(--kr-fs-btn);
  }

  .busy {
    border: 1px solid var(--kr-line);
    background: var(--kr-surface-2);
    color: var(--kr-text-2);
    font-size: var(--kr-fs-btn);
  }

  .busy:disabled {
    opacity: 1;
    cursor: progress;
  }

  .busy-icon {
    display: flex;
    color: var(--kr-accent);
  }

  .hint {
    margin: 0;
    padding: 0 var(--kr-sp-1);
    font-size: var(--kr-fs-sub);
    line-height: 1.35;
    color: var(--kr-text-3);
  }

  .hint.strong {
    color: var(--kr-text-2);
  }
</style>
