<script lang="ts">
  import { theme, THEME_CHOICES, type ThemeChoice } from "../../lib/theme";
  import { locale, tr } from "../../lib/i18n";
  import SegmentedControl from "@amigo-labs/krake-ui/components/SegmentedControl.svelte";

  let options = $derived(
    THEME_CHOICES.map((choice) => ({ value: choice, label: tr($locale, `theme.${choice}`) })),
  );
</script>

<section>
  <h3 class="text-lg font-bold mb-4" style="color: var(--text-primary)">{tr($locale, "settings.appearance")}</h3>

  <!-- Theme choice: Dark, Light or System -->
  <div class="outline-card rounded-xl p-5" style="background: var(--bg-surface)">
    <span class="text-sm font-semibold mb-3 block" style="color: var(--text-primary)">{tr($locale, "settings.theme")}</span>
    <SegmentedControl
      {options}
      value={$theme}
      ariaLabel={tr($locale, "settings.theme")}
      onchange={(value) => theme.set(value as ThemeChoice)}
    />
  </div>
</section>
