<script lang="ts">
  // Menu « Simulateur » de l'en-tête, affiché seulement sans le vrai tapis :
  // - mode navigateur : incidents du tapis TypeScript (`mock.ts`) ;
  // - application lancée avec `npm run app:sim` : le tapis Rust (`sim.rs`),
  //   avec sa console (ce que ferait quelqu'un sur le tapis) et ses incidents.
  // Il s'ouvre par-dessus l'écran, sans rien décaler.
  import { mock } from "../api";
  import { decimal, formatTenths } from "../format";
  import { sim } from "../sim.svelte";
  import { treadmill } from "../treadmill.svelte";
  import type { SimAction } from "../types";

  const connected = $derived(treadmill.snap.connection === "connected");
  const st = $derived(sim.status);

  function act(action: SimAction) {
    void treadmill.run(() => sim.act(action));
  }

  /** Ligne d'état de la bande, comme on la lirait sur la console. */
  const belt = $derived.by(() => {
    if (!st) return "";
    if (!st.keyIn) return "Clé retirée : console en erreur";
    if (st.countdownS !== null) return `Compte à rebours : ${st.countdownS}`;
    const speed = `${formatTenths(st.speedKmh)} km/h (consigne ${formatTenths(st.targetSpeedKmh)})`;
    if (st.running) return `En marche · ${speed}`;
    if (st.speedKmh > 0) return `Décélère · ${speed}`;
    return st.distanceM > 0 ? "En pause" : "À l'arrêt";
  });

  /** Ligne d'état de la radio. */
  const radio = $derived.by(() => {
    if (!st) return "";
    if (st.mutedForS !== null) return `Console muette encore ${st.mutedForS} s`;
    if (st.linkBackInS !== null) return `Coupure radio : de retour dans ${st.linkBackInS} s`;
    if (!st.reachable) return "Module Bluetooth éteint";
    return st.linkActive ? "Liaison ouverte" : "Visible, pas de liaison";
  });
</script>

<details class="sim" ontoggle={(e) => sim.watch((e.currentTarget as HTMLDetailsElement).open)}>
  <summary title="L'application ne pilote pas le vrai tapis">Simulateur</summary>

  <div class="panel">
    {#if sim.kind === "native"}
      {#if st}
        <section class="readout" aria-label="Tapis simulé">
          <p class="main">{belt}</p>
          <p>
            Inclinaison {formatTenths(st.inclinePct)} % (consigne {formatTenths(st.targetInclinePct)}) ·
            {st.distanceM} m · {decimal(st.energyKcal, 1)} kcal
          </p>
          <p>{radio}{st.longPause ? " · pause longue : démarrage ignoré" : ""}</p>
        </section>

        <h3>Console du tapis</h3>
        <div class="grid">
          <button type="button" onclick={() => act({ kind: "consoleStart" })}>Démarrer</button>
          <button type="button" onclick={() => act({ kind: "consoleStop" })}>Stop</button>
          <button type="button" onclick={() => act({ kind: "consoleSpeed", delta: -0.5 })}>
            Vitesse −0,5
          </button>
          <button type="button" onclick={() => act({ kind: "consoleSpeed", delta: 0.5 })}>
            Vitesse +0,5
          </button>
          <button type="button" onclick={() => act({ kind: "consoleIncline", delta: -1 })}>
            Inclinaison −1
          </button>
          <button type="button" onclick={() => act({ kind: "consoleIncline", delta: 1 })}>
            Inclinaison +1
          </button>
          <button
            type="button"
            class="wide"
            aria-pressed={st.handsOn}
            onclick={() => act({ kind: "handsOn", on: !st.handsOn })}
          >
            {st.handsOn ? "Lâcher les capteurs de pouls" : "Mains sur les capteurs de pouls"}
          </button>
        </div>

        <h3>Incidents</h3>
        <div class="grid">
          <button type="button" onclick={() => act({ kind: "linkLoss", seconds: 10 })}>
            Couper la liaison 10 s
          </button>
          <button type="button" onclick={() => act({ kind: "linkLoss", seconds: 30 })}>
            Couper la liaison 30 s
          </button>
          {#if st.keyIn}
            <button type="button" onclick={() => act({ kind: "pullKey" })}>Retirer la clé</button>
          {:else}
            <button type="button" onclick={() => act({ kind: "insertKey" })}>Remettre la clé</button>
          {/if}
          <button type="button" onclick={() => act({ kind: "mute", seconds: 8 })}>
            Console muette 8 s
          </button>
          <button type="button" onclick={() => act({ kind: "failNextWrite" })}>
            Faire échouer la prochaine commande
          </button>
          <button type="button" onclick={() => act({ kind: "corruptNextAnswer" })}>
            Réponse au CRC invalide
          </button>
          <button
            type="button"
            class="wide"
            disabled={st.running || st.distanceM === 0 || st.longPause}
            title="Le tapis ignorera le prochain démarrage, comme après une longue pause"
            onclick={() => act({ kind: "agePause" })}
          >
            Faire durer la pause (démarrage ignoré)
          </button>
        </div>
      {/if}
    {:else}
      <div class="list">
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
    {/if}
  </div>
</details>

<style>
  .sim {
    position: relative;
    flex-shrink: 0;
    white-space: nowrap;
  }

  /* Orange = avertissement : ce n'est pas le vrai tapis. */
  summary {
    cursor: pointer;
    list-style: none;
    padding: 3px 10px;
    border: 1px solid var(--kr-warn);
    border-radius: var(--kr-r-pill);
    font-size: var(--kr-fs-hint);
    font-weight: 700;
    letter-spacing: .04em;
    text-transform: uppercase;
    color: var(--kr-warn);
  }

  summary::-webkit-details-marker {
    display: none;
  }

  .panel {
    position: absolute;
    top: calc(100% + 6px);
    right: 0;
    z-index: 10;
    width: 380px;
    max-height: calc(100vh - 80px);
    overflow-y: auto;
    padding: var(--kr-sp-3);
    background: var(--kr-surface);
    border: 1px solid var(--kr-line);
    border-radius: var(--kr-r-md);
    box-shadow: 0 8px 24px rgba(0, 0, 0, .18);
    white-space: normal;
  }

  .readout {
    padding: var(--kr-sp-2) var(--kr-sp-3);
    border-radius: var(--kr-r-sm);
    background: var(--kr-surface-2);
    font-size: var(--kr-fs-hint);
    color: var(--kr-text-2);
  }

  .readout p {
    margin: 2px 0;
  }

  .readout .main {
    font-size: var(--kr-fs-sub);
    font-weight: 700;
    color: var(--kr-text);
  }

  h3 {
    margin: var(--kr-sp-3) 0 var(--kr-sp-2);
    font-size: var(--kr-fs-hint);
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: .04em;
    color: var(--kr-text-3);
  }

  .grid {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: var(--kr-sp-1);
  }

  .wide {
    grid-column: 1 / -1;
  }

  .list {
    display: flex;
    flex-direction: column;
    gap: var(--kr-sp-1);
  }

  button {
    min-height: 36px;
    padding: 4px var(--kr-sp-3);
    border: 0;
    border-radius: var(--kr-r-sm);
    background: var(--kr-surface-2);
    color: var(--kr-text);
    font-size: var(--kr-fs-hint);
    font-weight: 600;
    text-align: left;
  }

  button:hover:not(:disabled) {
    background: var(--kr-surface-3);
  }

  button:disabled {
    opacity: .45;
  }

  button[aria-pressed="true"] {
    box-shadow: inset 0 0 0 2px var(--kr-accent);
  }
</style>
