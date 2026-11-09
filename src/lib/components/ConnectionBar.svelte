<script lang="ts">
  // En-tête : état de la liaison, identité du tapis, version de l'application,
  // « Déconnecter » (une fois
  // connecté ou en reconnexion) et, sans le vrai tapis (navigateur ou
  // `npm run app:sim`), le menu du tapis simulé. « Connecter » est dans la
  // colonne de droite (ActionPanel).
  import { api } from "../api";
  import { sim } from "../sim.svelte";
  import { treadmill } from "../treadmill.svelte";
  import type { ConnectionState } from "../types";
  import { APP_VERSION } from "../version";
  import SimPanel from "./SimPanel.svelte";
  import Spinner from "./Spinner.svelte";

  const LABELS: Record<ConnectionState, string> = {
    disconnected: "Déconnecté",
    scanning: "Recherche du tapis…",
    connecting: "Connexion…",
    connected: "Connecté",
    reconnecting: "Reconnexion…",
  };

  const snap = $derived(treadmill.snap);
  const connection = $derived(snap.connection);
  const device = $derived(snap.device);
  const spinning = $derived(connection !== "disconnected" && connection !== "connected");
  const canDisconnect = $derived(connection === "connected" || connection === "reconnecting");
  const details = $derived(
    device ? [device.id, device.article].filter(Boolean).map((v) => `· ${v}`).join(" ") : "",
  );
</script>

<header class="bar">
  <div class="status">
    {#if spinning}
      <span class="spinner"><Spinner /></span>
    {:else}
      <span class="dot" class:on={connection === "connected"}></span>
    {/if}
    <span>{LABELS[connection]}</span>
  </div>

  <div class="device">
    {#if device}
      <span class="name">{device.name}</span>
      <span>{details}</span>
    {:else if connection === "disconnected"}
      <span>Aucun tapis connecté</span>
    {/if}
  </div>

  <div class="spacer"></div>

  <span class="version" title="Version de l'application">v{APP_VERSION}</span>

  {#if sim.kind}
    <SimPanel />
  {/if}

  {#if canDisconnect}
    <button type="button" class="disconnect" onclick={() => void treadmill.run(api.disconnect)}>
      <svg
        width="16"
        height="16"
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        stroke-width="2"
        stroke-linecap="round"
        stroke-linejoin="round"
        aria-hidden="true"
      >
        <path d="M7 7l10 10-5 5V2l5 5L7 17"></path>
      </svg>
      Déconnecter
    </button>
  {/if}
</header>

<style>
  .bar {
    height: var(--kr-header-h);
    flex-shrink: 0;
    display: flex;
    align-items: center;
    gap: var(--kr-sp-5);
    padding: 0 var(--kr-pad);
    background: var(--kr-surface);
    border-bottom: 1px solid var(--kr-line);
  }

  .status {
    display: flex;
    align-items: center;
    gap: 10px;
    font-size: var(--kr-fs-sub);
    font-weight: 700;
    color: var(--kr-text);
    white-space: nowrap;
  }

  .spinner {
    display: flex;
    color: var(--kr-accent);
  }

  .dot {
    width: 10px;
    height: 10px;
    border-radius: 50%;
    flex-shrink: 0;
    background: var(--kr-text-3);
  }

  .dot.on {
    background: var(--kr-go);
    box-shadow: 0 0 0 4px var(--kr-go-halo);
  }

  .device {
    display: flex;
    align-items: baseline;
    gap: var(--kr-sp-2);
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: var(--kr-fs-sub);
    color: var(--kr-text-2);
  }

  .name {
    font-weight: 700;
    color: var(--kr-text);
  }

  .spacer {
    flex: 1;
  }

  .version {
    flex-shrink: 0;
    font-family: var(--kr-font-num);
    font-size: var(--kr-fs-hint);
    font-weight: 600;
    font-variant-numeric: tabular-nums;
    letter-spacing: .02em;
    color: var(--kr-text-3);
    user-select: text;
    -webkit-user-select: text;
  }

  .disconnect {
    flex-shrink: 0;
    height: 40px;
    padding: 0 var(--kr-sp-4);
    border: 1px solid var(--kr-line);
    border-radius: var(--kr-r-sm);
    background: transparent;
    color: var(--kr-text-2);
    font-size: var(--kr-fs-sub);
    font-weight: 600;
    display: flex;
    align-items: center;
    gap: var(--kr-sp-2);
  }

  .disconnect:hover {
    background: var(--kr-surface-2);
  }
</style>
