<script lang="ts">
  import "../app.css";
  import { onMount } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import { app } from "$lib/state.svelte";
  import type { Game, TrackerEvent } from "$lib/types";
  import Sidebar from "$lib/components/Sidebar.svelte";
  import Library from "$lib/components/Library.svelte";
  import GameDetail from "$lib/components/GameDetail.svelte";
  import ImportWizard from "$lib/components/ImportWizard.svelte";
  import Settings from "$lib/components/Settings.svelte";
  import Toasts from "$lib/components/Toasts.svelte";

  onMount(() => {
    try {
      const saved = JSON.parse(localStorage.getItem("layout") ?? "null");
      if (saved?.mode) app.layout = saved;
    } catch {
      // Storage unavailable: keep defaults.
    }
    app.load().catch((e) => app.error(e));
    const unlisten = [
      listen<Game>("game-updated", (e) => app.upsert(e.payload)),
      listen<TrackerEvent>("tracker", (e) => {
        const ev = e.payload;
        const name = app.byId.get(ev.gameId)?.name ?? "Spiel";
        if (ev.type === "started") {
          if (!app.running.includes(ev.gameId)) app.running.push(ev.gameId);
        } else {
          app.running = app.running.filter((id) => id !== ev.gameId);
          if (ev.type === "ended") {
            const mins = Math.max(1, Math.round((ev.endedAt - ev.startedAt) / 60));
            app.toast(`${name}: ${mins >= 60 ? `${Math.floor(mins / 60)} Std. ${mins % 60} Min.` : `${mins} Min.`} gespielt`);
          } else {
            app.toast(`${name}: Spielzeit konnte nicht gemessen werden. Trag in den Spieldetails die Exe ein.`, "error", {
              label: "Öffnen",
              run: () => (app.screen = { kind: "detail", id: ev.gameId }),
            });
          }
        }
      }),
    ];
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
    {:else if app.screen.kind === "settings"}
      <Settings />
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
