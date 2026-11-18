<script lang="ts">
  import type { AppConfig } from "../../lib/api";
  import { locale, tr } from "../../lib/i18n";
  import { AUTH_MODES, needsConfirmation, securityView, type AuthMode } from "../../lib/security";
  import Card from "@amigo-labs/krake-ui/components/Card.svelte";
  import Banner from "@amigo-labs/krake-ui/components/Banner.svelte";
  import Button from "@amigo-labs/krake-ui/components/Button.svelte";
  import Dialog from "@amigo-labs/krake-ui/components/Dialog.svelte";

  // Who needs to log in (spec `local-auth-bypass`, AC-13 to AC-15).
  let { config, onsave }: { config: AppConfig; onsave: () => void } = $props();

  let view = $derived(securityView(config.server));
  /** A mode waiting for the user's confirmation. */
  let pending = $state<AuthMode | null>(null);

  function blocked(mode: AuthMode): boolean {
    return view.fromEnv || (mode === "never" && !view.neverAllowed);
  }

  // The radio only changes through `config`, so a cancelled confirmation
  // leaves the current mode checked.
  function choose(e: Event, mode: AuthMode) {
    e.preventDefault();
    if (mode === view.mode || blocked(mode)) return;
    if (needsConfirmation(view.mode, mode)) {
      pending = mode;
      return;
    }
    apply(mode);
  }

  function apply(mode: AuthMode) {
    pending = null;
    config.server.auth_mode = mode;
    onsave();
  }
</script>

<section>
  <h3 class="text-lg font-bold mb-4 section-title">{tr($locale, "security.title")}</h3>
  <Card padding="lg">
    <fieldset class="modes" disabled={view.fromEnv}>
      <legend class="text-sm font-semibold mb-3 legend">{tr($locale, "security.mode_label")}</legend>
      {#each AUTH_MODES as mode (mode)}
        <label class="mode" class:mode-off={blocked(mode)}>
          <input
            type="radio"
            name="auth-mode"
            value={mode}
            checked={view.mode === mode}
            disabled={blocked(mode)}
            onclick={(e) => choose(e, mode)}
          />
          <span class="mode-text">
            <span class="text-sm font-medium mode-name">{tr($locale, `security.${mode}`)}</span>
            <span class="text-xs mode-desc">{tr($locale, `security.${mode}_desc`)}</span>
            {#if mode === "never" && !view.neverAllowed}
              <span class="text-xs mode-desc">
                {tr($locale, "security.never_disabled", { bind: config.server.bind })}
              </span>
            {/if}
          </span>
        </label>
      {/each}
    </fieldset>
    {#if view.fromEnv}
      <p class="text-xs mt-3 mode-desc">{tr($locale, "security.from_env")}</p>
    {/if}
    <Banner tone="warning" class="mt-4">{tr($locale, "security.warning")}</Banner>
  </Card>

  {#if pending}
    {@const mode = pending}
    <Dialog
      title={tr($locale, "security.confirm_title")}
      size="sm"
      closeLabel={tr($locale, "common.close")}
      onclose={() => (pending = null)}
    >
      <p class="text-sm">{tr($locale, `security.${mode}_desc`)}</p>
      <p class="text-sm mt-3 mode-desc">{tr($locale, "security.warning")}</p>
      {#snippet footer()}
        <Button variant="ghost" onclick={() => (pending = null)}>{tr($locale, "common.cancel")}</Button>
        <Button variant="solid" tone="danger" onclick={() => apply(mode)}>
          {tr($locale, "security.confirm")}
        </Button>
      {/snippet}
    </Dialog>
  {/if}
</section>

<style>
  .section-title,
  .legend,
  .mode-name {
    color: var(--text-primary);
  }

  .modes {
    display: grid;
    gap: 0.75rem;
    border: 0;
    margin: 0;
    padding: 0;
    min-width: 0;
  }

  .mode {
    display: flex;
    align-items: flex-start;
    gap: 0.75rem;
    cursor: pointer;
  }

  .mode input {
    margin-top: 0.2rem;
    accent-color: var(--accent);
  }

  .mode-off {
    cursor: not-allowed;
    opacity: 0.6;
  }

  .mode-text {
    display: grid;
    gap: 0.125rem;
  }

  .mode-desc {
    color: var(--text-secondary);
  }
</style>
