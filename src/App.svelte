<script lang="ts">
  // Écran de pilotage unique, pour ordinateur (pas de version mobile) : mise en
  // page fixe, tout visible sans défiler de 900×680 (fenêtre par défaut) au
  // plein écran. En-tête et alertes en haut, puis grands chiffres à gauche
  // (lisibles à ~2 m) et commandes à droite, action principale en tête.
  //
  // Raccourcis : Échap ou Espace = arrêt (TOUJOURS envoyé, quel que soit le
  // bouton affiché : l'état peut avoir ~1 s de retard) ; flèches = ±0,1 km/h.
  import { api } from "./lib/api";
  import { keyAction, nextSpeed, speedStepUseful } from "./lib/controls";
  import ActionPanel from "./lib/components/ActionPanel.svelte";
  import Alerts from "./lib/components/Alerts.svelte";
  import ConnectionBar from "./lib/components/ConnectionBar.svelte";
  import Controls from "./lib/components/Controls.svelte";
  import Metrics from "./lib/components/Metrics.svelte";
  import { treadmill } from "./lib/treadmill.svelte";

  // Liaison absente : chiffres grisés (règle `.kr-app[data-liaison="off"]`).
  const liaison = $derived(treadmill.snap.connection === "connected" ? "on" : "off");

  /** Champ où les flèches ont leur sens habituel. */
  function isEditable(target: EventTarget | null): boolean {
    if (!(target instanceof HTMLElement)) return false;
    return (
      target.isContentEditable ||
      target instanceof HTMLInputElement ||
      target instanceof HTMLTextAreaElement ||
      target instanceof HTMLSelectElement
    );
  }

  function onKeyDown(e: KeyboardEvent) {
    if (e.metaKey || e.ctrlKey || e.altKey) return;
    const action = keyAction(e.key, isEditable(e.target));
    if (!action) return;
    // Empêche aussi Espace d'activer le bouton qui a le focus (« Démarrer » !).
    e.preventDefault();
    const snap = treadmill.snap;
    if (action === "stop") {
      if (!e.repeat) void treadmill.stop();
    } else {
      const delta = action === "speedUp" ? 0.1 : -0.1;
      if (speedStepUseful(snap, delta)) {
        void treadmill.run(() => api.setTargetSpeed(nextSpeed(snap, delta)));
      }
    }
  }

  // Un bouton s'active au relâchement d'Espace : on bloque aussi le keyup.
  function onKeyUp(e: KeyboardEvent) {
    if (e.key === " " || e.key === "Spacebar") e.preventDefault();
  }
</script>

<svelte:window onkeydown={onKeyDown} onkeyup={onKeyUp} />

<div class="kr-app" class:alarm={treadmill.snap.lostWhileRunning} data-liaison={liaison}>
  <ConnectionBar />
  <Alerts />
  <main>
    <Metrics />
    <aside aria-label="Commandes">
      <ActionPanel />
      <Controls />
    </aside>
  </main>
</div>

<style>
  .kr-app {
    position: relative;
    height: 100%;
    display: flex;
    flex-direction: column;
    overflow: hidden;
    background: var(--kr-bg);
    color: var(--kr-text);
  }

  /* Liseré de l'alerte critique, autour de la fenêtre, par-dessus tout le reste. */
  .alarm::after {
    content: "";
    position: absolute;
    inset: 0;
    z-index: 20;
    border: 4px solid var(--kr-stop);
    pointer-events: none;
  }

  main {
    flex: 1;
    min-height: 0;
    display: flex;
    gap: var(--kr-gap);
    padding: var(--kr-pad);
  }

  aside {
    width: var(--kr-rail-w);
    flex-shrink: 0;
    min-height: 0;
    display: flex;
    flex-direction: column;
    gap: var(--kr-gap);
  }
</style>
