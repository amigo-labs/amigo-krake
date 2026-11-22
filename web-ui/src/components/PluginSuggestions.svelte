<script lang="ts">
  // Plugin suggestions for the links in the add panel (spec
  // plugin-suggest-on-add, AC-10 to AC-14). Adding never waits for them.
  import { onDestroy } from "svelte";
  import { installPlugin, setPluginEnabled, suggestPlugins } from "../lib/api";
  import { createSuggestionLookup, domainView, type LinkSuggestion } from "../lib/pluginSuggest";
  import { addToast } from "../lib/toast";
  import { locale, tr } from "../lib/i18n";
  import Card from "@amigo-labs/krake-ui/components/Card.svelte";
  import Button from "@amigo-labs/krake-ui/components/Button.svelte";

  let { links }: { links: string[] } = $props();

  let suggestions = $state<LinkSuggestion[]>([]);
  let busyId = $state<string | null>(null);
  let expanded = $state<Record<string, boolean>>({});

  const lookup = createSuggestionLookup({
    fetch: suggestPlugins,
    onResult: (next) => (suggestions = next),
  });

  // Join so an unchanged list of links does not start a new lookup.
  let key = $derived(links.join("\n"));
  $effect(() => {
    lookup.update(key ? key.split("\n") : []);
  });
  onDestroy(() => lookup.dispose());

  async function act(s: LinkSuggestion) {
    busyId = s.id;
    const enable = s.action === "enable";
    try {
      if (enable) await setPluginEnabled(s.id, true);
      else await installPlugin(s.id, true);
      const name = { name: s.name };
      addToast(
        "success",
        enable ? tr($locale, "add.suggest_enabled", name) : tr($locale, "add.suggest_installed", name),
      );
      // Gone at once, even if the refresh below fails.
      suggestions = suggestions.filter((x) => x.id !== s.id);
      lookup.refresh();
    } catch (e) {
      addToast(
        "error",
        enable
          ? tr($locale, "add.suggest_enable_failed", { name: s.name })
          : tr($locale, "add.suggest_install_failed", { name: s.name }),
        e instanceof Error ? e.message : undefined,
      );
    } finally {
      busyId = null;
    }
  }
</script>

{#if suggestions.length > 0}
  <ul class="suggestions" role="list">
    {#each suggestions as s (s.id)}
      {@const view = domainView(s.domains)}
      <li>
        <Card padding="sm" data-suggestion={s.id}>
          <p class="suggest-line">{tr($locale, "add.suggest_line", { name: s.name, count: s.urls.length })}</p>
          <p class="suggest-domains">
            {tr($locale, "add.suggest_domains")}
            {#if view.kind === "any"}
              {tr($locale, "add.suggest_any_host")}
            {:else if view.kind === "none"}
              {tr($locale, "plugins.no_hosts")}
            {:else}
              <span class="mono">{(expanded[s.id] ? [...view.shown, ...view.hidden] : view.shown).join(", ")}</span>
              {#if view.hidden.length > 0 && !expanded[s.id]}
                <button type="button" class="show-all" onclick={() => (expanded[s.id] = true)}>
                  {tr($locale, "add.suggest_show_all", { count: view.shown.length + view.hidden.length })}
                </button>
              {/if}
            {/if}
          </p>
          <div class="suggest-actions">
            <Button size="sm" variant="soft" loading={busyId === s.id} disabled={busyId !== null} onclick={() => act(s)}>
              {s.action === "enable" ? tr($locale, "add.suggest_enable") : tr($locale, "plugins.install")}
            </Button>
          </div>
        </Card>
      </li>
    {/each}
  </ul>
{/if}

<style>
  .suggestions {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    gap: 0.5rem;
  }
  .suggest-line {
    font-size: 0.875rem;
    font-weight: 600;
    color: var(--text-primary);
  }
  .suggest-domains {
    margin-top: 0.25rem;
    font-size: 0.75rem;
    color: var(--text-secondary);
    overflow-wrap: anywhere;
  }
  .mono {
    font-family: var(--font-mono);
  }
  .show-all {
    margin-left: 0.25rem;
    color: var(--accent-ink);
    text-decoration: underline;
  }
  .suggest-actions {
    margin-top: 0.5rem;
    display: flex;
    justify-content: flex-end;
  }
</style>
