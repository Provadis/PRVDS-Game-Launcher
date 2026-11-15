<script lang="ts">
  import { api } from "$lib/api";
  import { app, type Source } from "$lib/state.svelte";
  import Icon from "./Icon.svelte";
  import NameDialog from "./NameDialog.svelte";

  type Dialog =
    | { kind: "newCollection" }
    | { kind: "renameCollection"; id: number; name: string }
    | { kind: "renameView"; id: number; name: string };

  let dialog = $state<Dialog | null>(null);
  let dropTarget = $state<number | null>(null);

  function isActive(s: Source) {
    if (app.screen.kind !== "library") return false;
    const cur = app.source;
    return cur.kind === s.kind && ("id" in cur ? cur.id === ("id" in s ? s.id : undefined) : true);
  }

  async function submitDialog(name: string) {
    const d = dialog;
    dialog = null;
    if (!d) return;
    try {
      if (d.kind === "newCollection") {
        const c = await api.createCollection(name);
        await app.reloadMeta();
        app.selectSource({ kind: "collection", id: c.id });
      } else if (d.kind === "renameCollection") {
        await api.renameCollection(d.id, name);
        await app.reloadMeta();
      } else {
        const v = app.views.find((x) => x.id === d.id);
        if (v) {
          const saved = await api.saveView(v.id, name, v.definition);
          app.views = app.views.map((x) => (x.id === saved.id ? saved : x));
        }
      }
    } catch (e) {
      app.error(e);
    }
  }

  async function deleteCollection(id: number, name: string) {
    if (!confirm(`Sammlung „${name}“ löschen? Die Spiele bleiben in der Bibliothek.`)) return;
    try {
      await api.deleteCollection(id);
      await app.reloadMeta();
      app.games = await api.listGames();
      if (app.source.kind === "collection" && app.source.id === id) app.selectSource({ kind: "all" });
    } catch (e) {
      app.error(e);
    }
  }

  async function deleteView(id: number, name: string) {
    if (!confirm(`Ansicht „${name}“ löschen?`)) return;
    try {
      await api.deleteView(id);
      app.views = app.views.filter((v) => v.id !== id);
      if (app.source.kind === "view" && app.source.id === id) app.selectSource({ kind: "all" });
    } catch (e) {
      app.error(e);
    }
  }

  // Games can be dragged from the grid onto a collection.
  async function dropOnCollection(e: DragEvent, collectionId: number) {
    e.preventDefault();
    dropTarget = null;
    const id = Number(e.dataTransfer?.getData("application/x-game-id"));
    if (!id) return;
    try {
      app.upsert(await api.setInCollection(collectionId, id, true));
      await app.reloadMeta();
      const c = app.collections.find((x) => x.id === collectionId);
      app.toast(`${app.byId.get(id)?.name} zu „${c?.name}“ hinzugefügt`);
    } catch (err) {
      app.error(err);
    }
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
    <button class="item" class:active={isActive({ kind: "recent" })} onclick={() => app.selectSource({ kind: "recent" })}>
      <Icon name="clock" />Zuletzt gespielt
    </button>
    <button class="item" class:active={isActive({ kind: "favorites" })} onclick={() => app.selectSource({ kind: "favorites" })}>
      <Icon name="star" />Favoriten<span class="count">{app.games.filter((g) => g.favorite && !g.hidden).length}</span>
    </button>
  </div>

  <div class="group">
    <div class="label">Sammlungen</div>
    {#each app.collections as c (c.id)}
      <div
        class="row"
        class:drop={dropTarget === c.id}
        role="listitem"
        ondragover={(e) => {
          if (e.dataTransfer?.types.includes("application/x-game-id")) {
            e.preventDefault();
            dropTarget = c.id;
          }
        }}
        ondragleave={() => (dropTarget = null)}
        ondrop={(e) => dropOnCollection(e, c.id)}>
        <button class="item" class:active={isActive({ kind: "collection", id: c.id })} onclick={() => app.selectSource({ kind: "collection", id: c.id })}>
          <Icon name="folder" size={16} /><span class="name">{c.name}</span><span class="count">{c.gameCount}</span>
        </button>
        <span class="tools">
          <button onclick={() => (dialog = { kind: "renameCollection", id: c.id, name: c.name })} aria-label="Sammlung {c.name} umbenennen"><Icon name="edit" size={14} /></button>
          <button onclick={() => deleteCollection(c.id, c.name)} aria-label="Sammlung {c.name} löschen"><Icon name="trash" size={14} /></button>
        </span>
      </div>
    {/each}
    <button class="item add" onclick={() => (dialog = { kind: "newCollection" })}><Icon name="plus" size={16} />Neue Sammlung</button>
  </div>

  <div class="group">
    <div class="label">Ansichten</div>
    {#each app.views as v (v.id)}
      <div class="row">
        <button class="item" class:active={isActive({ kind: "view", id: v.id })} onclick={() => app.selectSource({ kind: "view", id: v.id })}>
          <Icon name="filter" size={16} /><span class="name">{v.name}</span>
        </button>
        <span class="tools">
          <button onclick={() => (dialog = { kind: "renameView", id: v.id, name: v.name })} aria-label="Ansicht {v.name} umbenennen"><Icon name="edit" size={14} /></button>
          <button onclick={() => deleteView(v.id, v.name)} aria-label="Ansicht {v.name} löschen"><Icon name="trash" size={14} /></button>
        </span>
      </div>
    {/each}
    {#if !app.views.length}
      <p class="hint">Filter und Sortierung einstellen, dann „Ansicht speichern“.</p>
    {/if}
  </div>

  <div class="bottom">
    <button class="btn import" onclick={() => (app.screen = { kind: "import" })}>
      <Icon name="download" />Spiele importieren
    </button>
    <button class="item" class:active={app.screen.kind === "settings"} onclick={() => (app.screen = { kind: "settings" })}>
      <Icon name="settings" />Einstellungen
    </button>
  </div>
</nav>

{#if dialog}
  <NameDialog
    title={dialog.kind === "newCollection" ? "Neue Sammlung" : dialog.kind === "renameCollection" ? "Sammlung umbenennen" : "Ansicht umbenennen"}
    label="Name"
    initial={"name" in dialog ? dialog.name : ""}
    confirmLabel={dialog.kind === "newCollection" ? "Anlegen" : "Speichern"}
    onsubmit={submitDialog}
    onclose={() => (dialog = null)} />
{/if}

<style>
  nav {
    width: 260px;
    flex-shrink: 0;
    padding: 28px 16px;
    display: flex;
    flex-direction: column;
    gap: 26px;
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
  .label {
    padding: 0 12px 8px;
    font-size: 12px;
    font-weight: 600;
    letter-spacing: 1.2px;
    text-transform: uppercase;
    color: var(--text-muted);
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
    min-width: 0;
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
  .item.add {
    color: var(--text-muted);
    font-size: 14px;
  }
  .name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .count {
    margin-left: auto;
    color: var(--text-muted);
    font-weight: 500;
    font-size: 13px;
  }
  .row {
    position: relative;
    display: flex;
    align-items: center;
    border-radius: 10px;
  }
  .row.drop {
    outline: 2px dashed var(--accent);
  }
  .tools {
    position: absolute;
    right: 6px;
    display: none;
    gap: 2px;
    padding-left: 6px;
    background: var(--surface);
  }
  .row:hover .tools,
  .row:focus-within .tools {
    display: flex;
  }
  .row:hover .count {
    visibility: hidden;
  }
  .tools button {
    display: flex;
    padding: 6px;
    border: 0;
    border-radius: 6px;
    background: transparent;
    color: var(--text-muted);
  }
  .tools button:hover {
    color: var(--text);
    background: var(--surface-3);
  }
  .hint {
    margin: 0;
    padding: 0 12px;
    font-size: 13px;
    color: var(--text-muted);
    line-height: 1.45;
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
