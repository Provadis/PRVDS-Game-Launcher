<script lang="ts">
  import "../app.css";
  import { onMount } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import { app } from "$lib/state.svelte";
  import type { Game } from "$lib/types";
  import Sidebar from "$lib/components/Sidebar.svelte";
  import Library from "$lib/components/Library.svelte";
  import GameDetail from "$lib/components/GameDetail.svelte";
  import ImportWizard from "$lib/components/ImportWizard.svelte";
  import Toasts from "$lib/components/Toasts.svelte";

  onMount(() => {
    try {
      const saved = JSON.parse(localStorage.getItem("layout") ?? "null");
      if (saved?.mode) app.layout = saved;
    } catch {
      // Storage unavailable: keep defaults.
    }
    app.load().catch((e) => app.error(e));
    const unlisten = [listen<Game>("game-updated", (e) => app.upsert(e.payload))];
    return () => unlisten.forEach((p) => p.then((f) => f()));
  });

  $effect(() => {
    const json = JSON.stringify(app.layout);
    try {
      localStorage.setItem("layout", json);
    } catch {
      // ignore
    }
  });

  // Re-run the query whenever filters, the source or the library change.
  $effect(() => {
    JSON.stringify(app.query);
    void app.source;
    void app.games;
    const t = setTimeout(() => app.refreshVisible(), 60);
    return () => clearTimeout(t);
  });
</script>

<div class="app">
  {#if app.screen.kind === "import"}
    <ImportWizard />
  {:else}
    <Sidebar />
    {#if app.screen.kind === "detail"}
      {#key app.screen.id}
        <GameDetail id={app.screen.id} />
      {/key}
    {:else}
      <Library />
    {/if}
  {/if}
</div>
<Toasts />

<style>
  .app {
    display: flex;
    height: 100vh;
  }
  .app > :global(:only-child) {
    flex: 1;
  }
</style>
