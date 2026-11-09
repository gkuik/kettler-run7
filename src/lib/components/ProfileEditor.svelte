<script lang="ts">
  // Écran de création / modification d'un profil de course, à la place de la
  // console (accessible bande arrêtée seulement : App.svelte y ramène d'office
  // si la bande démarre). Un profil = une suite de segments : sur telle
  // distance, telle vitesse et telle inclinaison, puis le segment suivant.
  //
  // Les profils de départ sont en lecture seule : « Dupliquer » en fait une
  // copie modifiable. Rien n'est envoyé au tapis depuis cet écran.
  import { decimal } from "../format";
  import {
    DISTANCE_STEP_M,
    INCLINE_MAX_PCT,
    INCLINE_MIN_PCT,
    INCLINE_STEP_PCT,
    MAX_SEGMENTS,
    NAME_MAX_CHARS,
    SEGMENT_MAX_M,
    SEGMENT_MIN_M,
    SPEED_MAX_KMH,
    SPEED_MIN_KMH,
    SPEED_STEP_KMH,
    formatDuration,
    formatProfileDistance,
    nextSegment,
    profileError,
    segmentErrors,
    summarize,
  } from "../profiles";
  import { profiles } from "../profiles.svelte";
  import type { ProfileSegment } from "../types";
  import ProfileChart from "./ProfileChart.svelte";
  import StepField from "./StepField.svelte";

  const draft = $derived(profiles.draft!);
  const readOnly = $derived(draft.builtin);
  const summary = $derived(summarize(draft.segments));
  const error = $derived(readOnly ? null : profileError(draft));

  let selected = $state<number | null>(null);
  let confirm = $state<"leave" | "delete" | null>(null);
  let saving = $state(false);
  let rowsEl = $state<HTMLElement | null>(null);

  const title = $derived(
    readOnly ? "Profil de départ" : draft.id ? "Modifier le profil" : "Nouveau profil",
  );

  /** Début de chaque segment (m), pour la colonne « de… à… ». */
  const starts = $derived.by(() => {
    let at = 0;
    return draft.segments.map((s) => {
      const from = at;
      at += Number.isFinite(s.distanceM) ? s.distanceM : 0;
      return from;
    });
  });

  function update(i: number, patch: Partial<ProfileSegment>) {
    draft.segments[i] = { ...draft.segments[i], ...patch };
  }

  function add() {
    draft.segments.push(nextSegment(draft.segments));
    selected = draft.segments.length - 1;
    // Le nouveau segment est en bas de la liste : on le montre.
    queueMicrotask(() => rowsEl?.scrollTo({ top: rowsEl.scrollHeight, behavior: "smooth" }));
  }

  function duplicateSegment(i: number) {
    draft.segments.splice(i + 1, 0, { ...draft.segments[i] });
    selected = i + 1;
  }

  function removeSegment(i: number) {
    draft.segments.splice(i, 1);
    selected = null;
  }

  function move(i: number, dir: -1 | 1) {
    const j = i + dir;
    const segs = draft.segments;
    [segs[i], segs[j]] = [segs[j], segs[i]];
    selected = j;
  }

  function selectFromChart(i: number) {
    selected = i;
    rowsEl?.querySelector(`[data-row="${i}"]`)?.scrollIntoView({ block: "nearest", behavior: "smooth" });
  }

  function back() {
    if (profiles.dirty) confirm = "leave";
    else profiles.close();
  }

  async function save(thenClose = false) {
    saving = true;
    const ok = await profiles.save();
    saving = false;
    if (ok && thenClose) profiles.close();
    if (ok) confirm = null;
  }

  async function remove() {
    confirm = null;
    await profiles.remove();
  }
</script>

<section class="editor" aria-label="Éditeur de profil">
  <header class="top">
    <button type="button" class="ghost back" onclick={back}>
      <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.4" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
        <path d="M15 5l-7 7 7 7"></path>
      </svg>
      Console
    </button>

    <div class="title">
      <span class="kr-label">{title}</span>
      <input
        class="name"
        type="text"
        maxlength={NAME_MAX_CHARS}
        placeholder="Nom du profil"
        aria-label="Nom du profil"
        readonly={readOnly}
        bind:value={draft.name}
      />
    </div>

    <div class="actions">
      <button type="button" class="ghost" onclick={() => profiles.duplicate()}>Dupliquer</button>
      {#if !readOnly && draft.id}
        <button type="button" class="ghost danger" onclick={() => (confirm = "delete")}>Supprimer</button>
      {/if}
      {#if !readOnly}
        <button
          type="button"
          class="save"
          disabled={!!error || saving || (!profiles.dirty && !!draft.id)}
          onclick={() => void save()}
        >
          {profiles.saved && !profiles.dirty ? "Enregistré" : "Enregistrer"}
        </button>
      {/if}
    </div>
  </header>

  {#if confirm === "leave"}
    <div class="confirm" role="alertdialog" aria-label="Modifications non enregistrées">
      <span>Modifications non enregistrées.</span>
      <button type="button" class="save small" disabled={!!error} onclick={() => void save(true)}>Enregistrer et revenir</button>
      <button type="button" class="ghost small danger" onclick={() => profiles.close()}>Revenir sans enregistrer</button>
      <button type="button" class="ghost small" onclick={() => (confirm = null)}>Rester</button>
    </div>
  {:else if confirm === "delete"}
    <div class="confirm" role="alertdialog" aria-label="Supprimer le profil">
      <span>Supprimer « {draft.name.trim() || "sans nom"} » ? C'est définitif.</span>
      <button type="button" class="ghost small danger" onclick={() => void remove()}>Supprimer</button>
      <button type="button" class="ghost small" onclick={() => (confirm = null)}>Annuler</button>
    </div>
  {:else if readOnly}
    <div class="notice">
      Profil fourni avec l'application, en lecture seule. <strong>Dupliquer</strong> en fait une copie que vous pouvez adapter.
    </div>
  {/if}

  <div class="chart-card">
    <div class="chart">
      <ProfileChart segments={draft.segments} {selected} onselect={selectFromChart} />
    </div>
    <dl class="summary">
      <div><dt>Distance</dt><dd>{formatProfileDistance(summary.totalM)}</dd></div>
      <div><dt>Durée estimée</dt><dd>{formatDuration(summary.durationS)}</dd></div>
      <div><dt>Dénivelé</dt><dd>+{Math.round(summary.gainM)} m</dd></div>
      <div>
        <dt>Vitesse max</dt>
        <dd>{decimal(summary.maxSpeedKmh, 1)} km/h</dd>
      </div>
    </dl>
  </div>

  <div class="table-card">
    <div class="row head" aria-hidden="true">
      <span>Segment</span>
      <span>Distance <em>m</em></span>
      <span>Vitesse <em>km/h</em></span>
      <span>Inclinaison <em>%</em></span>
      <span></span>
    </div>
    <div class="rows" bind:this={rowsEl}>
      {#each draft.segments as seg, i (i)}
        {@const errs = readOnly ? {} : segmentErrors(seg)}
        <div class="row" class:sel={selected === i} data-row={i}>
          <div class="index">
            <span class="n">{i + 1}</span>
            <span class="span">{formatProfileDistance(starts[i])} → {formatProfileDistance(starts[i] + (Number.isFinite(seg.distanceM) ? seg.distanceM : 0))}</span>
          </div>
          <StepField
            label={`Segment ${i + 1}, distance`}
            unit="m"
            value={seg.distanceM}
            step={DISTANCE_STEP_M}
            min={SEGMENT_MIN_M}
            max={SEGMENT_MAX_M}
            digits={0}
            error={errs.distance}
            disabled={readOnly}
            onfocus={() => (selected = i)}
            onchange={(v) => update(i, { distanceM: v })}
          />
          <StepField
            label={`Segment ${i + 1}, vitesse`}
            unit="km/h"
            value={seg.speedKmh}
            step={SPEED_STEP_KMH}
            min={SPEED_MIN_KMH}
            max={SPEED_MAX_KMH}
            digits={1}
            error={errs.speed}
            disabled={readOnly}
            onfocus={() => (selected = i)}
            onchange={(v) => update(i, { speedKmh: v })}
          />
          <StepField
            label={`Segment ${i + 1}, inclinaison`}
            unit="%"
            value={seg.inclinePct}
            step={INCLINE_STEP_PCT}
            min={INCLINE_MIN_PCT}
            max={INCLINE_MAX_PCT}
            digits={1}
            error={errs.incline}
            disabled={readOnly}
            onfocus={() => (selected = i)}
            onchange={(v) => update(i, { inclinePct: v })}
          />
          <div class="tools">
            {#if !readOnly}
              <button type="button" class="tool" title="Monter" aria-label={`Monter le segment ${i + 1}`} disabled={i === 0} onclick={() => move(i, -1)}>
                <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.4" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M6 15l6-6 6 6"></path></svg>
              </button>
              <button type="button" class="tool" title="Descendre" aria-label={`Descendre le segment ${i + 1}`} disabled={i === draft.segments.length - 1} onclick={() => move(i, 1)}>
                <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.4" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M6 9l6 6 6-6"></path></svg>
              </button>
              <button type="button" class="tool" title="Dupliquer le segment" aria-label={`Dupliquer le segment ${i + 1}`} disabled={draft.segments.length >= MAX_SEGMENTS} onclick={() => duplicateSegment(i)}>
                <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linejoin="round" aria-hidden="true"><rect x="8" y="8" width="12" height="12" rx="2"></rect><path d="M16 8V5a1 1 0 0 0-1-1H5a1 1 0 0 0-1 1v10a1 1 0 0 0 1 1h3"></path></svg>
              </button>
              <button type="button" class="tool danger" title="Supprimer le segment" aria-label={`Supprimer le segment ${i + 1}`} disabled={draft.segments.length <= 1} onclick={() => removeSegment(i)}>
                <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.4" stroke-linecap="round" aria-hidden="true"><path d="M6 6l12 12M18 6L6 18"></path></svg>
              </button>
            {/if}
          </div>
        </div>
      {/each}

      {#if !readOnly}
        <button type="button" class="add" disabled={draft.segments.length >= MAX_SEGMENTS} onclick={add}>
          <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.4" stroke-linecap="round" aria-hidden="true"><path d="M12 5v14M5 12h14"></path></svg>
          Ajouter le segment suivant
        </button>
      {/if}
    </div>
  </div>

  <footer class="foot">
    {#if profiles.editorError}
      <span class="err" role="alert">{profiles.editorError}</span>
    {:else if error}
      <!-- Nouveau profil sans nom : une consigne, pas une faute. -->
      <span class:err={!!draft.name.trim() || draft.id !== ""}>{error}</span>
    {:else}
      <span>Après le dernier segment : récupération lente, sans arrêt automatique.</span>
    {/if}
    <span class="keys"><kbd>Échap</kbd> envoie toujours l'arrêt au tapis</span>
  </footer>
</section>

<style>
  .editor {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    gap: var(--kr-gap);
    padding: var(--kr-pad);
  }

  .top {
    flex-shrink: 0;
    display: flex;
    align-items: center;
    gap: var(--kr-sp-4);
  }

  .title {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .name {
    width: 100%;
    max-width: 420px;
    height: 40px;
    padding: 0 var(--kr-sp-3);
    border: 1px solid var(--kr-line);
    border-radius: var(--kr-r-sm);
    background: var(--kr-surface);
    color: var(--kr-text);
    font-family: var(--kr-font-ui);
    font-size: 20px;
    font-weight: 700;
    user-select: text;
    -webkit-user-select: text;
  }

  .name:focus {
    outline: none;
    border-color: var(--kr-accent);
  }

  .name[readonly] {
    border-color: transparent;
    background: transparent;
    padding-left: 0;
  }

  .actions {
    flex-shrink: 0;
    display: flex;
    gap: var(--kr-sp-2);
  }

  .ghost,
  .save {
    height: 44px;
    padding: 0 var(--kr-sp-4);
    border-radius: var(--kr-r-sm);
    font-size: var(--kr-fs-sub);
    font-weight: 700;
    display: inline-flex;
    align-items: center;
    gap: 6px;
    white-space: nowrap;
  }

  .ghost {
    border: 1px solid var(--kr-line);
    background: var(--kr-surface);
    color: var(--kr-text-2);
  }

  .ghost:hover:not(:disabled) {
    background: var(--kr-surface-2);
  }

  .ghost.danger {
    color: var(--kr-stop);
  }

  .back {
    padding-left: var(--kr-sp-3);
  }

  .save {
    border: 0;
    background: var(--kr-accent);
    color: var(--kr-accent-ink);
    min-width: 132px;
    justify-content: center;
  }

  .save:hover:not(:disabled) {
    filter: brightness(1.08);
  }

  .small {
    height: 36px;
    min-width: 0;
    font-size: 14px;
    padding: 0 var(--kr-sp-3);
  }

  .confirm,
  .notice {
    flex-shrink: 0;
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: var(--kr-sp-2) var(--kr-sp-3);
    padding: var(--kr-sp-2) var(--kr-sp-4);
    border-radius: var(--kr-r-sm);
    font-size: 15px;
  }

  .confirm {
    background: var(--kr-surface);
    border: 2px solid var(--kr-warn);
    color: var(--kr-text);
    font-weight: 600;
  }

  .confirm span {
    margin-right: auto;
  }

  .notice {
    background: var(--kr-surface-2);
    color: var(--kr-text-2);
  }

  .chart-card,
  .table-card {
    background: var(--kr-surface);
    border-radius: var(--kr-r-md);
  }

  .chart-card {
    flex: 0 0 auto;
    padding: var(--kr-sp-4) var(--kr-pad) var(--kr-sp-3) calc(var(--kr-pad) + 34px);
    display: flex;
    flex-direction: column;
    gap: var(--kr-sp-3);
  }

  .chart {
    height: clamp(120px, 24vh, 220px);
  }

  .summary {
    margin: 0;
    display: flex;
    gap: var(--kr-sp-6);
  }

  .summary div {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .summary dt {
    font-size: 12px;
    font-weight: 700;
    letter-spacing: .08em;
    text-transform: uppercase;
    color: var(--kr-text-3);
  }

  .summary dd {
    margin: 0;
    font-family: var(--kr-font-num);
    font-weight: 600;
    font-size: 22px;
    font-variant-numeric: tabular-nums;
    color: var(--kr-text);
  }

  .table-card {
    flex: 1;
    min-height: 120px;
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }

  .row {
    display: grid;
    grid-template-columns: minmax(96px, .8fr) repeat(3, minmax(120px, 1fr)) 152px;
    align-items: center;
    gap: var(--kr-sp-3);
    padding: 6px var(--kr-pad);
    border-left: 3px solid transparent;
  }

  .row.head {
    flex-shrink: 0;
    padding-top: var(--kr-sp-3);
    padding-bottom: var(--kr-sp-2);
    border-bottom: 1px solid var(--kr-line);
    font-size: 12px;
    font-weight: 700;
    letter-spacing: .08em;
    text-transform: uppercase;
    color: var(--kr-text-2);
  }

  .row.head em {
    font-style: normal;
    font-weight: 400;
    letter-spacing: 0;
    text-transform: none;
    color: var(--kr-text-3);
  }

  .rows {
    flex: 1;
    min-height: 0;
    overflow: hidden auto;
    padding-bottom: var(--kr-sp-3);
  }

  .row.sel {
    background: var(--kr-surface-2);
    border-left-color: var(--kr-accent);
  }

  .index {
    display: flex;
    flex-direction: column;
    gap: 1px;
    min-width: 0;
  }

  .n {
    font-family: var(--kr-font-num);
    font-weight: 600;
    font-size: 20px;
    color: var(--kr-text);
  }

  .span {
    font-size: 12px;
    color: var(--kr-text-3);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .tools {
    display: flex;
    justify-content: flex-end;
    gap: 2px;
  }

  .tool {
    width: 36px;
    height: 36px;
    padding: 0;
    border: 0;
    border-radius: var(--kr-r-sm);
    background: transparent;
    color: var(--kr-text-3);
    display: grid;
    place-items: center;
  }

  .tool:hover:not(:disabled) {
    background: var(--kr-surface-3);
    color: var(--kr-text);
  }

  .tool.danger:hover:not(:disabled) {
    color: var(--kr-stop);
  }

  .add {
    margin: var(--kr-sp-3) var(--kr-pad) 0;
    height: 44px;
    padding: 0 var(--kr-sp-4);
    border: 1px dashed var(--kr-line);
    border-radius: var(--kr-r-sm);
    background: transparent;
    color: var(--kr-accent);
    font-size: var(--kr-fs-sub);
    font-weight: 700;
    display: inline-flex;
    align-items: center;
    gap: var(--kr-sp-2);
  }

  .add:hover:not(:disabled) {
    background: var(--kr-surface-2);
  }

  .foot {
    flex-shrink: 0;
    display: flex;
    justify-content: space-between;
    gap: var(--kr-sp-4);
    font-size: 13px;
    color: var(--kr-text-3);
  }

  .err {
    font-weight: 700;
    color: var(--kr-stop);
  }

  .keys {
    white-space: nowrap;
  }

  kbd {
    font-family: var(--kr-font-ui);
    font-size: 12px;
    font-weight: 700;
    padding: 1px 6px;
    border-radius: 5px;
    border: 1px solid var(--kr-line);
    background: var(--kr-surface-2);
    color: var(--kr-text-2);
  }
</style>
