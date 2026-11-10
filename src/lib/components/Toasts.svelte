<script lang="ts">
  // Erreurs de commande et de connexion, en bas de la colonne des mesures,
  // qu'elles recouvrent : elles s'effacent donc toutes d'elles-mêmes (10 s, la
  // barre montre le temps restant). Erreur de l'instantané (Rust) et erreur de
  // la dernière commande : souvent la même chaîne, on ne l'affiche alors qu'une
  // fois. L'échec d'un STOP a son propre bandeau (Alerts.svelte) : pas de
  // doublon. Pendant une reconnexion, l'erreur du Rust n'est pas ici mais dans
  // le bandeau orange (voir `snapError`).
  import { COMMAND_ERROR_MS, treadmill } from "../treadmill.svelte";

  const errors = $derived(
    [treadmill.snapError, treadmill.commandError].filter(
      (e, i, all): e is string => !!e && all.indexOf(e) === i && e !== treadmill.stopError,
    ),
  );
</script>

{#if errors.length > 0}
  <div class="toasts">
    {#each errors as error (error)}
      <div class="toast" role="alert">
        <svg
          width="24"
          height="24"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          stroke-width="2.4"
          stroke-linecap="round"
          stroke-linejoin="round"
          aria-hidden="true"
        >
          <circle cx="12" cy="12" r="9"></circle>
          <path d="M12 7.5v5.5"></path>
          <path d="M12 16.4v.1"></path>
        </svg>
        <div class="message">{error}</div>
        <div class="bar" style:--duree={`${COMMAND_ERROR_MS}ms`}></div>
      </div>
    {/each}
  </div>
{/if}

<style>
  .toasts {
    position: absolute;
    left: 50%;
    bottom: calc(var(--kr-gap) + 8px);
    transform: translateX(-50%);
    width: min(92%, 560px);
    display: flex;
    flex-direction: column;
    gap: var(--kr-sp-2);
  }

  .toast {
    position: relative;
    overflow: hidden;
    display: flex;
    align-items: center;
    gap: var(--kr-sp-3);
    padding: 14px var(--kr-sp-4) var(--kr-sp-4);
    background: var(--kr-text);
    color: var(--kr-warn);
    border-radius: var(--kr-r-md);
    box-shadow: 0 16px 40px rgba(0, 0, 0, .35);
  }

  svg {
    flex-shrink: 0;
  }

  .message {
    flex: 1;
    color: var(--kr-bg);
    font-size: var(--kr-fs-sub);
    font-weight: 600;
    line-height: 1.3;
  }

  .bar {
    position: absolute;
    left: 0;
    bottom: 0;
    height: 3px;
    width: 100%;
    background: var(--kr-warn);
    transform-origin: left;
    animation: kr-toast var(--duree) linear forwards;
  }

  @keyframes kr-toast {
    to {
      transform: scaleX(0);
    }
  }
</style>
