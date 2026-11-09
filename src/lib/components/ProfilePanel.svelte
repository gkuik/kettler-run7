<script lang="ts">
  // Section « Profil » de la colonne de droite : choix du profil déroulé
  // pendant la séance, et accès à l'éditeur (bande arrêtée seulement).
  // Bande en marche, le profil ne se change pas ; on peut seulement le laisser
  // (« Sans profil ») : les consignes restent alors telles quelles.
  import { canEditProfiles } from "../controls";
  import { profiles } from "../profiles.svelte";
  import { treadmill } from "../treadmill.svelte";

  const snap = $derived(treadmill.snap);
  const selectedId = $derived(snap.profile?.profileId ?? "");
  const editable = $derived(canEditProfiles(snap, treadmill.startPending));
  const starters = $derived(profiles.list.filter((p) => p.builtin));
  const mine = $derived(profiles.list.filter((p) => !p.builtin));

  function onChange(e: Event) {
    const select = e.currentTarget as HTMLSelectElement;
    const id = select.value || null;
    // Les flèches doivent revenir à la vitesse, pas changer de profil.
    select.blur();
    void profiles.select(id);
  }
</script>

<section class="panel" aria-label="Profil de course">
  <div class="head">
    <span class="kr-label">Profil</span>
    {#if !editable && snap.profile}
      <span class="kr-unit">en cours</span>
    {/if}
  </div>

  {#if editable}
    <div class="row">
      <select class="choice" value={selectedId} onchange={onChange} aria-label="Profil de course">
        <option value="">Aucun profil</option>
        {#if starters.length > 0}
          <optgroup label="Profils de départ">
            {#each starters as p (p.id)}
              <option value={p.id}>{p.name}</option>
            {/each}
          </optgroup>
        {/if}
        {#if mine.length > 0}
          <optgroup label="Mes profils">
            {#each mine as p (p.id)}
              <option value={p.id}>{p.name}</option>
            {/each}
          </optgroup>
        {/if}
      </select>
      <button
        type="button"
        class="kr-btn icon"
        disabled={!selectedId}
        title="Voir ou modifier ce profil"
        aria-label="Voir ou modifier le profil choisi"
        onclick={() => profiles.open(selectedId)}
      >
        <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
          <path d="M4 20h4L19 9l-4-4L4 16z"></path>
          <path d="M13.5 6.5l4 4"></path>
        </svg>
      </button>
      <button
        type="button"
        class="kr-btn icon"
        title="Créer un profil"
        aria-label="Créer un profil"
        onclick={() => profiles.open(null)}
      >
        <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.4" stroke-linecap="round" aria-hidden="true">
          <path d="M12 5v14M5 12h14"></path>
        </svg>
      </button>
    </div>
    {#if profiles.suspended}
      <button type="button" class="resume" onclick={() => profiles.resume()}>
        Modifications non enregistrées · reprendre
      </button>
    {/if}
  {:else}
    <div class="row">
      <div class="current">{snap.profile?.profileName ?? "Aucun profil"}</div>
      {#if snap.profile}
        <button
          type="button"
          class="kr-btn leave"
          title="Le profil ne pilote plus le tapis ; les consignes restent telles quelles"
          onclick={() => void profiles.select(null)}
        >
          Sans profil
        </button>
      {/if}
    </div>
  {/if}
</section>

<style>
  .panel {
    flex-shrink: 0;
    padding: var(--kr-sp-3) var(--kr-sp-4);
    background: var(--kr-surface);
    border-radius: var(--kr-r-lg);
    display: flex;
    flex-direction: column;
    gap: var(--kr-sp-2);
  }

  .head {
    display: flex;
    align-items: baseline;
    gap: var(--kr-sp-2);
  }

  .row {
    height: 44px;
    display: flex;
    gap: var(--kr-sp-2);
  }

  .choice {
    flex: 1;
    min-width: 0;
    height: 100%;
    padding: 0 var(--kr-sp-3);
    border-radius: var(--kr-r-sm);
    border: 1px solid var(--kr-line);
    background: var(--kr-surface-2);
    color: var(--kr-text);
    font-family: var(--kr-font-ui);
    font-size: var(--kr-fs-sub);
    font-weight: 600;
  }

  .choice:focus-visible {
    outline: 3px solid var(--kr-accent);
    outline-offset: 2px;
  }

  .icon {
    width: 44px;
    flex-shrink: 0;
    display: grid;
    place-items: center;
    color: var(--kr-text-2);
  }

  .current {
    flex: 1;
    min-width: 0;
    align-self: center;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: var(--kr-fs-sub);
    font-weight: 700;
    color: var(--kr-text);
  }

  .leave {
    flex-shrink: 0;
    padding: 0 var(--kr-sp-3);
    font-family: var(--kr-font-ui);
    font-size: 14px;
    font-weight: 600;
    color: var(--kr-text-2);
  }

  .resume {
    align-self: flex-start;
    padding: 0;
    border: 0;
    background: none;
    font-size: 13px;
    font-weight: 600;
    color: var(--kr-accent);
    text-decoration: underline;
  }
</style>
