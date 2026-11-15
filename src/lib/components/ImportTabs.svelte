<script lang="ts">
  import { app } from "$lib/state.svelte";

  let { current }: { current: "desktop" | "launchers" } = $props();
</script>

<div class="tabs" role="tablist" aria-label="Importquelle">
  <button class="chip" class:active={current === "desktop"} role="tab" aria-selected={current === "desktop"}
    onclick={() => (app.screen = { kind: "import", from: "desktop" })}>Vom Desktop</button>
  <button class="chip" class:active={current === "launchers"} role="tab" aria-selected={current === "launchers"}
    onclick={() => (app.screen = { kind: "import", from: "launchers" })}>Aus Launchern</button>
</div>

<style>
  .tabs {
    display: flex;
    gap: 8px;
    margin-top: 10px;
  }
</style>
