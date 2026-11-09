<script lang="ts">
  // Écran de pilotage unique, pour ordinateur (pas de version mobile) : mise en
  // page fixe, tout visible sans défiler de 900×680 (fenêtre par défaut) au
  // plein écran. En-tête et alertes en haut, puis grands chiffres à gauche
  // (lisibles à ~2 m) et commandes à droite, action principale en tête.
  //
  // Raccourcis : Échap ou Espace = arrêt (TOUJOURS envoyé, quel que soit le
  // bouton affiché : l'état peut avoir ~1 s de retard) ; flèches = ±0,1 km/h.
  // Seule exception : Espace tapé dans un champ texte de l'éditeur de profils.
  //
  // Deux écrans : la console, et l'éditeur de profils, accessible bande
  // arrêtée seulement. Si la bande démarre (console du tapis), on revient
  // d'office à la console, où PAUSE/STOP est affiché.
  import { api } from "./lib/api";
  import { canEditProfiles, keyAction, nextSpeed, speedStepUseful } from "./lib/controls";
  import ActionPanel from "./lib/components/ActionPanel.svelte";
  import Alerts from "./lib/components/Alerts.svelte";
  import ConnectionBar from "./lib/components/ConnectionBar.svelte";
  import Controls from "./lib/components/Controls.svelte";
  import Metrics from "./lib/components/Metrics.svelte";
  import ProfileEditor from "./lib/components/ProfileEditor.svelte";
  import ProfilePanel from "./lib/components/ProfilePanel.svelte";
  import { profiles } from "./lib/profiles.svelte";
  import { treadmill } from "./lib/treadmill.svelte";

  // Liaison absente : chiffres grisés (règle `.kr-app[data-liaison="off"]`).
  const liaison = $derived(treadmill.snap.connection === "connected" ? "on" : "off");

  const editing = $derived(profiles.view === "editor" && profiles.draft !== null);

  $effect(() => {
    if (profiles.view === "editor" && !canEditProfiles(treadmill.snap, treadmill.startPending)) {
      profiles.close(true);
    }
  });

  /** Champ texte (nom d'un profil, valeurs de l'éditeur) : Espace y tape une espace. */
  function isTextEntry(target: EventTarget | null): boolean {
    if (target instanceof HTMLTextAreaElement) return true;
    return target instanceof HTMLInputElement && (target.type === "text" || target.type === "search");
  }

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
    const action = keyAction(e.key, isEditable(e.target), editing && isTextEntry(e.target));
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
    if ((e.key === " " || e.key === "Spacebar") && !(editing && isTextEntry(e.target))) {
      e.preventDefault();
    }
  }
</script>

<svelte:window onkeydown={onKeyDown} onkeyup={onKeyUp} />

<div class="kr-app" class:alarm={treadmill.snap.lostWhileRunning} data-liaison={liaison}>
  <ConnectionBar />
  <Alerts />
  {#if editing}
    <ProfileEditor />
  {:else}
    <main>
      <Metrics />
      <aside aria-label="Commandes">
        <ActionPanel />
        <ProfilePanel />
        <Controls />
      </aside>
    </main>
  {/if}
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
