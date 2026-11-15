<script lang="ts">
  import { t } from "$lib/i18n.svelte";
  import { api } from "$lib/api";
  import { app } from "$lib/state.svelte";
  import NameDialog from "./NameDialog.svelte";
  import GameGrid from "./GameGrid.svelte";
  import GameList from "./GameList.svelte";
  import HeroBanner from "./HeroBanner.svelte";
  import Icon from "./Icon.svelte";
  import Toolbar from "./Toolbar.svelte";

  // Most recently played game, shown on the start page.
  let continueGame = $derived.by(() => {
    if (app.source.kind !== "all" || app.query.search) return null;
    return app.games
      .filter((g) => g.lastPlayed && !g.hidden)
      .sort((a, b) => (b.lastPlayed ?? 0) - (a.lastPlayed ?? 0))[0] ?? null;
  });

  let saving = $state(false);
  let overwrite = $state(true);
  let currentView = $derived(app.source.kind === "view" ? app.views.find((v) => v.id === (app.source as { id: number }).id) : undefined);

  async function saveView(name: string) {
    saving = false;
    try {
      const definition = { query: app.effectiveQuery(), layout: $state.snapshot(app.layout) };
      const id = currentView && overwrite ? currentView.id : null;
      const v = await api.saveView(id, name, definition);
      app.views = id ? app.views.map((x) => (x.id === v.id ? v : x)) : [...app.views, v];
      app.source = { kind: "view", id: v.id };
      app.toast(t("Ansicht „{name}“ gespeichert", { name: v.name }));
    } catch (e) {
      app.error(e);
    }
  }

  let title = $derived.by(() => {
    switch (app.source.kind) {
      case "favorites":
        return t("Favoriten");
      case "recent":
        return t("Zuletzt gespielt");
      case "collection": {
        const id = app.source.id;
        return app.collections.find((c) => c.id === id)?.name ?? t("Sammlung");
      }
      case "view": {
        const id = app.source.id;
        return app.views.find((v) => v.id === id)?.name ?? t("Ansicht");
      }
      default:
        return t("Alle Spiele");
    }
  });
</script>

<main class="scroll">
  {#if app.loaded && app.games.length === 0}
    <section class="welcome">
      <span class="logo"><Icon name="gamepad" size={40} /></span>
      <h1>{t("Willkommen in deinem Game Space")}</h1>
      <p class="muted">
        {t("Lies deinen Desktop oder deine Launcher ein, wähle deine Spiele aus und räume die Verknüpfungen in die Bibliothek. Du kannst jedes Spiel jederzeit wieder zurück auf den Desktop legen.")}
      </p>
      <button class="btn big primary" onclick={() => (app.screen = { kind: "import" })}>
        <Icon name="download" />{t("Desktop einlesen")}
      </button>
      <button class="btn big" onclick={() => (app.screen = { kind: "import", from: "launchers" })}>
        <Icon name="layers" />{t("Aus Launchern einlesen")}
      </button>
    </section>
  {:else}
    {#if continueGame}<HeroBanner game={continueGame} />{/if}
    <Toolbar {title} count={app.visible.length} onSaveView={() => { overwrite = true; saving = true; }} />
    {#if app.visible.length}
      {#if app.layout.mode === "list"}
        <GameList games={app.visible} />
      {:else}
        <GameGrid games={app.visible} size={app.layout.coverSize} />
      {/if}
    {:else if app.loaded}
      <p class="muted">{t("Keine Spiele für diese Ansicht und diese Filter.")}</p>
    {/if}
  {/if}
</main>

{#if saving}
  <NameDialog
    title={t("Ansicht speichern")}
    label={t("Name der Ansicht")}
    initial={currentView?.name ?? ""}
    onsubmit={saveView}
    onclose={() => (saving = false)}>
    {#snippet extra()}
      {#if currentView}
        <label class="overwrite">
          <input type="checkbox" bind:checked={overwrite} />{t("„{name}“ überschreiben", { name: currentView.name })}
        </label>
      {:else}
        <p class="muted hint">{t("Speichert Filter, Sortierung und Layout. Die Ansicht erscheint links in der Seitenleiste.")}</p>
      {/if}
    {/snippet}
  </NameDialog>
{/if}

<style>
  main {
    flex: 1;
    min-width: 0;
    height: 100vh;
    padding: 32px clamp(24px, 3.5vw, 64px) 64px;
    display: flex;
    flex-direction: column;
    gap: 28px;
  }
  .overwrite {
    display: flex;
    align-items: center;
    gap: 10px;
    font-size: 14px;
  }
  .overwrite input {
    accent-color: var(--accent);
    width: 16px;
    height: 16px;
  }
  .hint {
    margin: 0;
    font-size: 14px;
    line-height: 1.5;
  }
  .welcome {
    margin: auto;
    max-width: 560px;
    display: flex;
    flex-direction: column;
    align-items: center;
    text-align: center;
    gap: 18px;
  }
  .welcome h1 {
    font-size: 36px;
  }
  .welcome p {
    line-height: 1.6;
    margin: 0;
  }
  .logo {
    width: 80px;
    height: 80px;
    border-radius: 22px;
    background: var(--accent);
    color: var(--on-accent);
    display: grid;
    place-items: center;
  }
</style>
