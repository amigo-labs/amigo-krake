<script lang="ts">
  import { onMount } from "svelte";
  import { applyCoreUpdate, checkUpdates, getStatus, setUpdateMode, type UpdateMode } from "../../lib/api";
  import { desktopMode, refreshDesktopMode } from "../../lib/desktopMode";
  import { aboutView, checkAndApply, type CheckOutcome } from "../../lib/desktopUpdates";
  import { locale, tr } from "../../lib/i18n";
  import Card from "@amigo-labs/krake-ui/components/Card.svelte";
  import Button from "@amigo-labs/krake-ui/components/Button.svelte";
  import Banner from "@amigo-labs/krake-ui/components/Banner.svelte";
  import UpdateModeChoices from "../UpdateModeChoices.svelte";

  // Settings ▸ About (spec desktop-app, AC-30): the running version
  // everywhere; in the desktop app also the version waiting for the next
  // start, the update choice and Check for updates.

  let version = $state<string | null>(null);
  let view = $derived(aboutView($desktopMode));

  let savingMode = $state(false);
  /** The last change applied for this run but could not be stored. */
  let notSaved = $state(false);
  let modeError = $state("");

  let checking = $state(false);
  let outcome = $state<CheckOutcome | null>(null);
  let checkError = $state("");

  onMount(async () => {
    void refreshDesktopMode();
    try {
      version = (await getStatus()).version;
    } catch {
      // The version is informational; the section shows without it.
    }
  });

  async function chooseMode(mode: UpdateMode) {
    if (savingMode) return;
    savingMode = true;
    modeError = "";
    try {
      const state = await setUpdateMode(mode);
      desktopMode.set(state);
      notSaved = state.saved === false;
    } catch (e) {
      modeError = e instanceof Error ? e.message : String(e);
      // Put the radios back on what the server holds.
      await refreshDesktopMode();
    } finally {
      savingMode = false;
    }
  }

  async function check() {
    if (checking) return;
    checking = true;
    outcome = null;
    checkError = "";
    try {
      outcome = await checkAndApply(checkUpdates, applyCoreUpdate);
      if (outcome.kind === "ready") void refreshDesktopMode();
    } catch (e) {
      checkError = e instanceof Error ? e.message : String(e);
    } finally {
      checking = false;
    }
  }
</script>

<section>
  <h3 class="text-lg font-bold mb-4 title">{tr($locale, "settings.about")}</h3>
  <Card padding="lg">
    <div class="flex items-center gap-4">
      <img src="/icon.svg" alt="Krake" width="40" height="40" />
      <div>
        <p class="font-bold title">Krake</p>
        <p class="text-sm muted">
          {#if version}<span class="mono" data-testid="about-version">v{version}</span> — {/if}{tr(
            $locale,
            "settings.tagline",
          )}
        </p>
        <p class="text-xs mt-1 mono muted">github.com/amigo-labs/amigo-krake</p>
      </div>
    </div>

    {#if view.kind === "package-manager"}
      <p class="text-sm mt-4 muted" data-testid="about-package-manager">
        {tr($locale, "updates.package_manager")}
      </p>
    {:else if view.kind === "desktop"}
      <div class="updates mt-4">
        {#if view.pending}
          <Banner tone="success" role="status">
            {tr($locale, "updates.pending", { version: view.pending })}
          </Banner>
        {/if}
        <UpdateModeChoices name="about-update-mode" value={view.mode} disabled={savingMode} onchange={chooseMode} />
        {#if notSaved}
          <Banner tone="warning" role="alert">{tr($locale, "updates.not_saved")}</Banner>
        {/if}
        {#if modeError}
          <Banner tone="danger" role="alert" title={tr($locale, "updates.mode_failed")}>{modeError}</Banner>
        {/if}
        <div class="check-row">
          <Button variant="soft" size="sm" iconLeft="refresh" loading={checking} onclick={check}>
            {tr($locale, "updates.check")}
          </Button>
          <span class="text-sm muted" role="status">
            {#if outcome?.kind === "current"}
              {tr($locale, "updates.up_to_date")}
            {:else if outcome?.kind === "ready"}
              <span class="mono">v{outcome.version}</span> — {tr($locale, "updates.ready")}
            {:else if outcome?.kind === "started"}
              {tr($locale, "plugins.update_started")}
            {:else if outcome?.kind === "package-manager"}
              {tr($locale, "updates.package_manager")}
            {/if}
          </span>
        </div>
        {#if checkError}
          <Banner tone="danger" role="alert" title={tr($locale, "updates.check_failed")}>{checkError}</Banner>
        {/if}
      </div>
    {/if}
  </Card>
</section>

<style>
  .title {
    color: var(--text-primary);
  }

  .muted {
    color: var(--text-secondary);
  }

  .mono {
    font-family: var(--font-mono);
  }

  .updates {
    display: grid;
    gap: var(--space-4);
    padding-top: var(--space-4);
    border-top: 1px solid var(--border-color);
  }

  .check-row {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-3);
  }
</style>
