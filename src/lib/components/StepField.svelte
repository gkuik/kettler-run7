<script lang="ts">
  // Champ numérique de l'éditeur de profils : [−] valeur [+]. La saisie au
  // clavier accepte la virgule (« 4,5 ») ; une valeur hors bornes reste saisie
  // et est signalée (le profil ne s'enregistre pas), les boutons ± bornent.
  import { decimal } from "../format";
  import { parseDecimal } from "../profiles";

  interface Props {
    value: number;
    step: number;
    min: number;
    max: number;
    /** Décimales affichées. */
    digits: number;
    label: string;
    unit: string;
    error?: string;
    /** Avertissement (au-dessus du plafond…), sans bloquer l'enregistrement. */
    warning?: string;
    disabled?: boolean;
    onchange: (value: number) => void;
    onfocus?: () => void;
  }

  let { value, step, min, max, digits, label, unit, error, warning, disabled = false, onchange, onfocus }: Props =
    $props();

  const format = (v: number) => (Number.isFinite(v) ? decimal(v, digits) : "");
  let focused = $state(false);
  let text = $state("");
  // Hors saisie, le champ suit la valeur (boutons ±, changement de profil…).
  $effect(() => {
    if (!focused) text = format(value);
  });

  const round = (v: number) => Math.round(v * 10 ** digits) / 10 ** digits;
  const clamp = (v: number) => round(Math.min(max, Math.max(min, v)));

  function onInput(e: Event) {
    text = (e.currentTarget as HTMLInputElement).value;
    const n = parseDecimal(text);
    onchange(n === null ? Number.NaN : round(n));
  }

  function bump(dir: 1 | -1) {
    const base = Number.isFinite(value) ? value : min;
    onchange(clamp(base + dir * step));
  }
</script>

<div class="field" class:error={!!error} class:warning={!error && !!warning} title={error ?? warning ?? ""}>
  <button
    type="button"
    class="kr-btn step"
    tabindex="-1"
    disabled={disabled || (Number.isFinite(value) && value <= min)}
    aria-label={`${label} : −${decimal(step, digits)} ${unit}`}
    onclick={() => bump(-1)}
  >−</button>
  <input
    type="text"
    inputmode="decimal"
    autocomplete="off"
    spellcheck="false"
    value={text}
    readonly={disabled}
    aria-label={`${label} (${unit})`}
    aria-invalid={!!error}
    oninput={onInput}
    onfocus={() => {
      focused = true;
      onfocus?.();
    }}
    onblur={() => {
      focused = false;
      text = format(value);
    }}
    onkeydown={(e) => {
      if (e.key === "Enter") (e.currentTarget as HTMLInputElement).blur();
    }}
  />
  <button
    type="button"
    class="kr-btn step"
    tabindex="-1"
    disabled={disabled || (Number.isFinite(value) && value >= max)}
    aria-label={`${label} : +${decimal(step, digits)} ${unit}`}
    onclick={() => bump(1)}
  >+</button>
</div>

<style>
  .field {
    height: 40px;
    display: grid;
    grid-template-columns: 36px minmax(0, 1fr) 36px;
    border: 1px solid var(--kr-line);
    border-radius: var(--kr-r-sm);
    background: var(--kr-surface-2);
    overflow: hidden;
  }

  .field:focus-within {
    border-color: var(--kr-accent);
  }

  .field.warning {
    border-color: var(--kr-warn);
  }

  .field.error {
    border-color: var(--kr-stop);
    box-shadow: inset 0 0 0 1px var(--kr-stop);
  }

  .step {
    border: 0;
    border-radius: 0;
    background: transparent;
    font-size: 20px;
    color: var(--kr-text-2);
  }

  input {
    min-width: 0;
    border: 0;
    background: transparent;
    text-align: center;
    font-family: var(--kr-font-num);
    font-weight: 600;
    font-size: 19px;
    font-variant-numeric: tabular-nums;
    color: var(--kr-text);
    user-select: text;
    -webkit-user-select: text;
  }

  input:focus {
    outline: none;
  }

  .error input {
    color: var(--kr-stop);
  }
</style>
