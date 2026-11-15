<script lang="ts">
  import { t } from "$lib/i18n.svelte";
  import { app } from "$lib/state.svelte";

  let { current }: { current: "desktop" | "launchers" } = $props();
</script>

<div class="tabs" role="tablist" aria-label={t("Importquelle")}>
  <button class="chip" class:active={current === "desktop"} role="tab" aria-selected={current === "desktop"}
    onclick={() => (app.screen = { kind: "import", from: "desktop" })}>{t("Vom Desktop")}</button>
  <button class="chip" class:active={current === "launchers"} role="tab" aria-selected={current === "launchers"}
    onclick={() => (app.screen = { kind: "import", from: "launchers" })}>{t("Aus Launchern")}</button>
</div>

<style>
  .tabs {
    display: flex;
    gap: 8px;
    margin-top: 10px;
  }
</style>
