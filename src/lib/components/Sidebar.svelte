<script lang="ts">
  import { app, type Source } from "$lib/state.svelte";
  import Icon from "./Icon.svelte";

  function isActive(s: Source) {
    if (app.screen.kind !== "library") return false;
    const cur = app.source;
    return cur.kind === s.kind && ("id" in cur ? cur.id === ("id" in s ? s.id : undefined) : true);
  }
</script>

<nav aria-label="Navigation">
  <div class="brand">
    <span class="logo"><Icon name="gamepad" size={20} stroke={2.2} /></span>
    <span>PRVDS</span>
  </div>

  <div class="group">
    <button class="item" class:active={isActive({ kind: "all" })} onclick={() => app.selectSource({ kind: "all" })}>
      <Icon name="grid" />Alle Spiele<span class="count">{app.games.filter((g) => !g.hidden).length}</span>
    </button>
  </div>

  <div class="bottom">
    <button class="btn import" onclick={() => (app.screen = { kind: "import" })}>
      <Icon name="download" />Vom Desktop importieren
    </button>
  </div>
</nav>

<style>
  nav {
    width: 260px;
    flex-shrink: 0;
    padding: 28px 16px;
    display: flex;
    flex-direction: column;
    gap: 28px;
    background: var(--bg-raised);
    border-right: 1px solid var(--border);
    overflow-y: auto;
    scrollbar-width: thin;
  }
  .brand {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 0 10px;
    font-family: var(--font-display);
    font-weight: 700;
    font-size: 20px;
    letter-spacing: 0.5px;
  }
  .logo {
    width: 34px;
    height: 34px;
    border-radius: 10px;
    background: var(--accent);
    color: var(--on-accent);
    display: grid;
    place-items: center;
  }
  .group {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .item {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 10px 12px;
    border-radius: 10px;
    border: 0;
    background: transparent;
    color: var(--text-2);
    font-size: 15px;
    text-align: left;
    width: 100%;
  }
  .item:hover {
    background: var(--surface);
    color: var(--text);
  }
  .item.active {
    background: var(--surface-2);
    color: var(--text);
    font-weight: 600;
  }
  .count {
    margin-left: auto;
    color: var(--text-muted);
    font-weight: 500;
    font-size: 13px;
  }
  .bottom {
    margin-top: auto;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .import {
    width: 100%;
    padding: 12px;
  }
</style>
