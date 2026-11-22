<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import { get } from "svelte/store";
  import {
    applyCoreUpdate,
    dismissUpdateQuestion,
    markUpdateAnnounced,
    setUpdateMode,
    type UpdateMode,
  } from "../lib/api";
  import { desktopMode, desktopModeKnown, refreshDesktopMode } from "../lib/desktopMode";
  import {
    DEFAULT_UPDATE_MODE,
    MODE_POLL_MS,
    announcementToShow,
    readyVersion,
    shouldAskQuestion,
  } from "../lib/desktopUpdates";
  import { locale, tr } from "../lib/i18n";
  import { openLayer, layerState } from "../lib/overlays.svelte";
  import { addToast } from "../lib/toast";
  import Dialog from "@amigo-labs/krake-ui/components/Dialog.svelte";
  import Button from "@amigo-labs/krake-ui/components/Button.svelte";
  import Banner from "@amigo-labs/krake-ui/components/Banner.svelte";
  import UpdateModeChoices from "./UpdateModeChoices.svelte";

  // The desktop app's first-start update question and its "notify"
  // announcement (spec desktop-app, AC-25 to AC-29). Both read the network-free
  // `GET /updates/mode`; outside the desktop app it answers 404 and nothing
  // here ever shows.

  /** The question was confirmed or closed in this window. */
  let questionHandled = $state(false);
  let choice = $state<UpdateMode>(DEFAULT_UPDATE_MODE);
  let confirming = $state(false);

  /** Versions this window answered (Later, close, a successful Install). */
  let handled = $state<ReadonlySet<string>>(new Set());
  /** The announcement on screen; kept while it installs or reports. */
  let announced = $state<string | null>(null);
  let phase = $state<"idle" | "installing" | "ready" | "error">("idle");
  let installError = $state("");
  let readyAs = $state("");

  let showQuestion = $derived(shouldAskQuestion($desktopMode, questionHandled));

  $effect(() => {
    // While idle, follow the server: a newer version replaces the one shown.
    if (phase !== "idle") return;
    announced = announcementToShow($desktopMode, handled, showQuestion);
  });

  const questionLayer = layerState("update-question");
  $effect(() => {
    if (!showQuestion) return;
    return openLayer({ id: "update-question", kind: "modal", label: "Update choice", onDismiss: closeQuestion });
  });

  const announceLayer = layerState("update-announcement");
  $effect(() => {
    if (!announced) return;
    return openLayer({
      id: "update-announcement",
      kind: "modal",
      label: "Update available",
      dismissible: phase !== "installing",
      onDismiss: closeAnnouncement,
    });
  });

  async function confirmQuestion() {
    if (confirming || questionHandled) return;
    confirming = true;
    try {
      desktopMode.set(await setUpdateMode(choice));
    } catch (e) {
      addToast("error", tr($locale, "updates.mode_failed"), e instanceof Error ? e.message : undefined);
    } finally {
      confirming = false;
      questionHandled = true;
    }
  }

  /** Escape, the close button or the backdrop: no choice is stored (AC-26). */
  function closeQuestion() {
    if (questionHandled || confirming) return;
    questionHandled = true;
    dismissUpdateQuestion().then(
      (state) => desktopMode.set(state),
      () => {
        // The app counts hiding the window as closing anyway (AC-26).
      },
    );
  }

  function markHandled(version: string) {
    handled = new Set([...handled, version]);
  }

  async function markAnnounced(version: string) {
    try {
      desktopMode.set(await markUpdateAnnounced(version));
    } catch {
      // Not marked: the next due check announces it again, as after a quit.
    }
  }

  /** Later, Escape or the close button marks the version (AC-28). */
  function later() {
    const version = announced;
    if (!version || phase === "installing") return;
    markHandled(version);
    announced = null;
    phase = "idle";
    void markAnnounced(version);
  }

  async function install() {
    const version = announced;
    if (!version || phase === "installing") return;
    phase = "installing";
    installError = "";
    try {
      readyAs = readyVersion(await applyCoreUpdate()) ?? version;
      phase = "ready";
      markHandled(version);
      await markAnnounced(version);
    } catch (e) {
      // A failed Install leaves the version unannounced (AC-28).
      installError = e instanceof Error ? e.message : String(e);
      phase = "error";
    }
  }

  function closeAnnouncement() {
    if (phase === "ready") {
      announced = null;
      phase = "idle";
    } else {
      later();
    }
  }

  // Ask again every minute and when the window comes back, so an
  // announcement found while it was hidden in the tray shows when it opens.
  let timer: ReturnType<typeof setInterval> | undefined;

  function poll() {
    // Not the desktop app: nothing to wait for.
    if (get(desktopModeKnown) && get(desktopMode) === null) return;
    void refreshDesktopMode();
  }

  function onVisible() {
    if (document.visibilityState === "visible") poll();
  }

  onMount(() => {
    void refreshDesktopMode();
    timer = setInterval(poll, MODE_POLL_MS);
    document.addEventListener("visibilitychange", onVisible);
    window.addEventListener("focus", poll);
  });

  onDestroy(() => {
    if (timer) clearInterval(timer);
    if (typeof document !== "undefined") document.removeEventListener("visibilitychange", onVisible);
    if (typeof window !== "undefined") window.removeEventListener("focus", poll);
  });
</script>

{#if showQuestion}
  <Dialog
    title={tr($locale, "updates.question_title")}
    description={tr($locale, "updates.question_desc")}
    size="sm"
    isTop={$questionLayer.isTop}
    closeLabel={tr($locale, "common.close")}
    onclose={closeQuestion}
  >
    <UpdateModeChoices
      name="update-question"
      value={choice}
      showLegend={false}
      disabled={confirming}
      onchange={(mode) => (choice = mode)}
    />
    {#snippet footer()}
      <Button variant="solid" loading={confirming} onclick={confirmQuestion}>
        {tr($locale, "updates.confirm")}
      </Button>
    {/snippet}
  </Dialog>
{:else if announced}
  <Dialog
    title={tr($locale, "updates.available_title", { version: announced })}
    size="sm"
    isTop={$announceLayer.isTop}
    dismissable={phase !== "installing"}
    closeLabel={tr($locale, "common.close")}
    onclose={closeAnnouncement}
  >
    {#if phase === "ready"}
      <Banner tone="success" role="status">
        <span class="version">{readyAs}</span> — {tr($locale, "updates.ready")}
      </Banner>
    {:else}
      <p class="text">{tr($locale, "updates.available_desc")}</p>
      {#if phase === "error"}
        <Banner tone="danger" role="alert" class="mt-3" title={tr($locale, "updates.failed")}>
          {installError}
        </Banner>
      {/if}
    {/if}
    {#snippet footer()}
      {#if phase === "ready"}
        <Button variant="solid" onclick={closeAnnouncement}>{tr($locale, "common.close")}</Button>
      {:else}
        <Button variant="ghost" disabled={phase === "installing"} onclick={later}>
          {tr($locale, "updates.later")}
        </Button>
        <Button variant="solid" loading={phase === "installing"} onclick={install}>
          {tr($locale, "updates.install")}
        </Button>
      {/if}
    {/snippet}
  </Dialog>
{/if}

<style>
  .text {
    margin: 0;
    font-size: var(--font-sm);
    color: var(--text-secondary);
  }

  .version {
    font-family: var(--font-mono);
    color: var(--text-primary);
  }
</style>
