<script lang="ts">
  import { t } from "$lib/i18n.svelte";
  import { formatDuration } from "$lib/format";
  import "../app.css";
  import { onMount } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import { app } from "$lib/state.svelte";
  import type { Game, TrackerEvent } from "$lib/types";
  import Sidebar from "$lib/components/Sidebar.svelte";
  import Library from "$lib/components/Library.svelte";
  import GameDetail from "$lib/components/GameDetail.svelte";
  import ImportWizard from "$lib/components/ImportWizard.svelte";
  import LauncherImport from "$lib/components/LauncherImport.svelte";
  import BoardView from "$lib/components/BoardView.svelte";
  import Settings from "$lib/components/Settings.svelte";
  import Toasts from "$lib/components/Toasts.svelte";
  import QuickLaunch from "$lib/components/QuickLaunch.svelte";
  import { checkOnStart } from "$lib/updates";
  import { syncOnStart } from "$lib/launchers";
  import { openHome } from "$lib/home";

  onMount(() => {
    try {
      const saved = JSON.parse(localStorage.getItem("layout") ?? "null");
      if (saved?.mode) app.layout = saved;
    } catch {
      // Storage unavailable: keep defaults.
    }
    app.load().then(() => {
      openHome();
      return syncOnStart();
    }).catch((e) => app.error(e));
    setTimeout(checkOnStart, 3000);
    const unlisten = [
      listen<Game>("game-updated", (e) => app.upsert(e.payload)),
      listen<TrackerEvent>("tracker", (e) => {
        const ev = e.payload;
        const name = app.byId.get(ev.gameId)?.name ?? t("Spiel");
        if (ev.type === "started") {
          if (!app.running.includes(ev.gameId)) app.running.push(ev.gameId);
        } else {
          app.running = app.running.filter((id) => id !== ev.gameId);
          if (ev.type === "ended") {
            const mins = Math.max(1, Math.round((ev.endedAt - ev.startedAt) / 60));
            app.toast(t("{name}: {time} gespielt", { name, time: formatDuration(mins * 60) }));
          } else {
            app.toast(t("{name}: Spielzeit konnte nicht gemessen werden. Trag in den Spieldetails die Exe ein.", { name }), "error", {
              label: t("Öffnen"),
              run: () => (app.openGame(ev.gameId)),
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

<!-- Files dropped into the window must not open as a page. Only a game's
     detail page takes them (as cover); everywhere else the drop is refused. -->
<svelte:window
  onkeydown={(e) => {
    // Ctrl+K (or Cmd+K) opens the quick launch search from anywhere.
    if ((e.ctrlKey || e.metaKey) && !e.altKey && e.key.toLowerCase() === "k") {
      e.preventDefault();
      app.quickOpen = !app.quickOpen;
    }
  }}
  ondragover={(e) => {
    if (!e.dataTransfer?.types.includes("Files")) return;
    e.preventDefault();
    e.dataTransfer.dropEffect = app.screen.kind === "detail" ? "copy" : "none";
  }}
  ondrop={(e) => {
    if (e.dataTransfer?.types.includes("Files")) e.preventDefault();
  }}
/>

<div class="app">
  {#if app.screen.kind === "import"}
    {#if app.screen.from === "launchers"}
      <LauncherImport />
    {:else}
      <ImportWizard />
    {/if}
  {:else}
    <Sidebar />
    {#if app.screen.kind === "detail"}
      {#key app.screen.id}
        <GameDetail id={app.screen.id} />
      {/key}
    {:else if app.screen.kind === "board"}
      <BoardView id={app.screen.id} />
    {:else if app.screen.kind === "settings"}
      <Settings />
    {:else}
      <Library />
    {/if}
  {/if}
</div>
<Toasts />
{#if app.quickOpen}<QuickLaunch />{/if}

<style>
  .app {
    display: flex;
    height: 100vh;
  }
  .app > :global(:only-child) {
    flex: 1;
  }
</style>
