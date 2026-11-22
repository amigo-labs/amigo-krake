<script lang="ts">
  import type { UpdateMode } from "../lib/api";
  import { UPDATE_MODES } from "../lib/desktopUpdates";
  import { locale, tr } from "../lib/i18n";

  // The three update choices of the desktop app (spec desktop-app, AC-25,
  // AC-30), shared by the first-start question and Settings ▸ About. Native
  // radios: one tab stop with arrow keys, no hand-rolled radiogroup.
  let {
    value,
    name,
    disabled = false,
    showLegend = true,
    onchange,
  }: {
    value: UpdateMode;
    /** Radio group name; unique per place the choices show. */
    name: string;
    disabled?: boolean;
    showLegend?: boolean;
    onchange: (mode: UpdateMode) => void;
  } = $props();
</script>

<fieldset class="modes" {disabled}>
  <legend class="legend" class:sr-only={!showLegend}>{tr($locale, "updates.mode_label")}</legend>
  {#each UPDATE_MODES as mode (mode)}
    <label class="mode">
      <input
        type="radio"
        {name}
        value={mode}
        checked={value === mode}
        onchange={() => onchange(mode)}
      />
      <span class="mode-name">{tr($locale, `updates.mode_${mode}`)}</span>
    </label>
  {/each}
</fieldset>

<style>
  .modes {
    display: grid;
    gap: 0.625rem;
    border: 0;
    margin: 0;
    padding: 0;
    min-width: 0;
  }

  .legend {
    margin-bottom: 0.5rem;
    padding: 0;
    font-size: var(--font-sm);
    font-weight: 600;
    color: var(--text-primary);
  }

  .mode {
    display: flex;
    align-items: flex-start;
    gap: 0.75rem;
    cursor: pointer;
  }

  .modes:disabled .mode {
    cursor: progress;
  }

  .mode input {
    margin-top: 0.2rem;
    accent-color: var(--accent);
  }

  .mode-name {
    font-size: var(--font-sm);
    color: var(--text-primary);
  }
</style>
