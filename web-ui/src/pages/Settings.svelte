<script lang="ts">
  import { onMount } from "svelte";
  import { features } from "../lib/stores";
  import { getConfig, putConfig, getWebhooks, type AppConfig } from "../lib/api";
  import { addToast } from "../lib/toast";
  import { saveOrRestore } from "../lib/settings";
  import { locale, tr, type Locale } from "../lib/i18n";
  import Icon from "@amigo-labs/krake-ui/components/Icon.svelte";

  import SettingsFeatures from "../components/settings/SettingsFeatures.svelte";
  import SettingsUsenet from "../components/settings/SettingsUsenet.svelte";
  import SettingsUsenetServers from "../components/settings/SettingsUsenetServers.svelte";
  import SettingsAppearance from "../components/settings/SettingsAppearance.svelte";
  import SettingsDownloads from "../components/settings/SettingsDownloads.svelte";
  import SettingsWebhooks from "../components/settings/SettingsWebhooks.svelte";
  import SettingsRssFeeds from "../components/settings/SettingsRssFeeds.svelte";
  import SettingsSecurity from "../components/settings/SettingsSecurity.svelte";
  import SettingsAbout from "../components/settings/SettingsAbout.svelte";
  import SkeletonCard from "../components/SkeletonCard.svelte";

  let config = $state<AppConfig | null>(null);
  let webhooks = $state<any[]>([]);

  onMount(async () => {
    try {
      const [cfg, wh] = await Promise.all([getConfig(), getWebhooks()]);
      config = cfg;
      webhooks = wh;
      if (cfg.features) features.set(cfg.features);
    } catch (e) { console.error("Failed to load config:", e); }
  });

  async function saveConfig() {
    if (!config) return;
    const result = await saveOrRestore(config, putConfig, getConfig);
    config = result.config;
    features.set(result.config.features);
    if (result.ok) addToast("success", tr($locale, "settings.saved"));
    else addToast("error", tr($locale, "settings.save_failed"), result.error);
  }
</script>

{#if config}
<div class="max-w-2xl space-y-8">
  <SettingsFeatures {config} onsave={saveConfig} />
  {#if config.features.usenet}
    <SettingsUsenetServers />
    <SettingsUsenet {config} onsave={saveConfig} />
    {#if config.features.rss_feeds}
      <SettingsRssFeeds />
    {/if}
  {/if}
  <SettingsAppearance />
  <SettingsDownloads {config} onsave={saveConfig} />
  <SettingsWebhooks bind:webhooks />
  <SettingsSecurity {config} onsave={saveConfig} />

  <!-- Language -->
  <section>
    <h3 class="text-lg font-bold mb-4" style="color: var(--text-primary)">{tr($locale, "settings.language")}</h3>
    <div class="rounded-xl p-5" style="background: var(--bg-surface); border: 1px solid var(--border-color)">
      <div class="flex items-center gap-3">
        <Icon name="globe" size={18} />
        <select
          value={$locale}
          onchange={(e) => locale.set((e.target as HTMLSelectElement).value as Locale)}
          class="flex-1 rounded-lg px-3 py-2 text-sm"
          style="background: var(--bg-surface-2); border: 1px solid var(--border-color); color: var(--text-primary)"
          aria-label={tr($locale, "settings.language")}
        >
          <option value="en" style="background: var(--bg-surface-2)">English</option>
          <option value="de" style="background: var(--bg-surface-2)">Deutsch</option>
        </select>
      </div>
    </div>
  </section>

  <!-- About -->
  <SettingsAbout />
</div>
{:else}
<SkeletonCard count={3} />
{/if}
