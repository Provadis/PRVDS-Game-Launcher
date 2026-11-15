<script lang="ts">
  import { app } from "$lib/state.svelte";
  import GameGrid from "./GameGrid.svelte";
  import Icon from "./Icon.svelte";

  let title = $derived.by(() => {
    switch (app.source.kind) {
      case "favorites":
        return "Favoriten";
      case "recent":
        return "Zuletzt gespielt";
      case "collection": {
        const id = app.source.id;
        return app.collections.find((c) => c.id === id)?.name ?? "Sammlung";
      }
      case "view": {
        const id = app.source.id;
        return app.views.find((v) => v.id === id)?.name ?? "Ansicht";
      }
      default:
        return "Alle Spiele";
    }
  });
</script>

<main class="scroll">
  {#if app.loaded && app.games.length === 0}
    <section class="welcome">
      <span class="logo"><Icon name="gamepad" size={40} /></span>
      <h1>Willkommen in deinem Game Space</h1>
      <p class="muted">
        Lies deinen Desktop ein, wähle deine Spiele aus und räume die Verknüpfungen in die Bibliothek. Du kannst
        jedes Spiel jederzeit wieder zurück auf den Desktop legen.
      </p>
      <button class="btn big primary" onclick={() => (app.screen = { kind: "import" })}>
        <Icon name="download" />Desktop einlesen
      </button>
    </section>
  {:else}
    <div class="toolbar">
      <h2>{title}</h2>
      <span class="muted">{app.visible.length} Spiele</span>
    </div>
    {#if app.visible.length}
      <GameGrid games={app.visible} size={app.layout.coverSize} />
    {:else if app.loaded}
      <p class="muted">Keine Spiele für diese Ansicht.</p>
    {/if}
  {/if}
</main>

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
  .toolbar {
    display: flex;
    align-items: center;
    gap: 14px;
    flex-wrap: wrap;
  }
  .toolbar h2 {
    font-size: 28px;
    font-weight: 600;
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
