<script lang="ts">
  import { convertFileSrc } from "@tauri-apps/api/core";
  import { api } from "$lib/api";
  import { app } from "$lib/state.svelte";
  import { formatPlaytime, placeholderColors, relativeDay } from "$lib/format";
  import { platformLabel, type ArtKind, type GamePatch } from "$lib/types";
  import ArtPicker from "./ArtPicker.svelte";
  import CollectionsPanel from "./CollectionsPanel.svelte";
  import Cover from "./Cover.svelte";
  import Icon from "./Icon.svelte";
  import PlaytimePanel from "./PlaytimePanel.svelte";
  import TagEditor from "./TagEditor.svelte";
  import TrackingPanel from "./TrackingPanel.svelte";

  let { id }: { id: number } = $props();
  let game = $derived(app.byId.get(id));
  let menuOpen = $state(false);
  let renaming = $state(false);
  let nameDraft = $state("");
  let picker = $state<ArtKind | null>(null);
  let dropping = $state(false);
  let refreshing = $state(false);
  let hero = $derived(game?.heroPath ? convertFileSrc(game.heroPath) : null);
  let colors = $derived(placeholderColors(game?.name ?? ""));
  let backLabel = $derived.by(() => {
    const b = app.back;
    if (b.kind === "board") return app.boards.find((x) => x.id === b.id)?.name ?? "Bibliothek";
    if (b.kind === "settings") return "Einstellungen";
    if (b.kind === "import") return "Import";
    return "Bibliothek";
  });

  // Drop an image file anywhere on the page to use it as cover.
  const hasFiles = (e: DragEvent) => !!e.dataTransfer?.types.includes("Files");

  function onDragOver(e: DragEvent) {
    if (!hasFiles(e)) return;
    e.preventDefault();
    dropping = true;
  }

  function onDragLeave(e: DragEvent) {
    // Only when the pointer leaves the window, not between child elements.
    if (!e.relatedTarget) dropping = false;
  }

  async function onDrop(e: DragEvent) {
    if (!hasFiles(e)) return;
    e.preventDefault();
    dropping = false;
    const file = Array.from(e.dataTransfer?.files ?? []).find((f) => /\.(png|jpe?g|webp|gif)$/i.test(f.name));
    if (!file || !game) return;
    try {
      const bytes = new Uint8Array(await file.arrayBuffer());
      app.upsert(await api.setArtFromBytes(game.id, "cover", file.name, bytes));
      app.toast("Cover übernommen");
    } catch (err) {
      app.error(err);
    }
  }

  async function refresh() {
    if (!game) return;
    menuOpen = false;
    refreshing = true;
    try {
      app.upsert(await api.refreshMetadata(game.id));
      app.toast("Infos aktualisiert");
    } catch (e) {
      app.error(e);
    } finally {
      refreshing = false;
    }
  }

  async function patch(p: GamePatch) {
    if (!game) return;
    try {
      app.upsert(await api.updateGame(game.id, p));
    } catch (e) {
      app.error(e);
    }
  }

  function startRename() {
    if (!game) return;
    nameDraft = game.name;
    renaming = true;
    menuOpen = false;
  }

  async function saveRename(e: SubmitEvent) {
    e.preventDefault();
    await patch({ name: nameDraft });
    renaming = false;
  }

  async function undo() {
    menuOpen = false;
    if (!game) return;
    if (game.source === "connector" || !game.journalId) {
      if (!confirm(`„${game.name}“ aus der Bibliothek entfernen? Das Spiel bleibt installiert, Spielzeit und Tags gehen verloren.${game.journalId ? " Die Desktop-Verknüpfung kommt zurück." : ""}`)) return;
      try {
        await api.removeGame(game.id);
        app.remove(game.id);
        app.goBack();
        app.toast(`„${game.name}“ entfernt`);
      } catch (e) {
        app.error(e);
      }
      return;
    }
    if (!confirm(`„${game.name}“ zurück auf den Desktop legen und aus der Bibliothek entfernen?`)) return;
    try {
      const path = await api.undoImport(game.id);
      app.remove(game.id);
      app.goBack();
      app.toast(`Verknüpfung wiederhergestellt: ${path}`);
    } catch (e) {
      app.error(e);
    }
  }
</script>

<svelte:window onclick={() => (menuOpen = false)} ondragover={onDragOver} ondragleave={onDragLeave} ondrop={onDrop} />

{#if game}
  <div class="detail scroll" style="--a: {colors[0]}; --b: {colors[1]}">
    <div class="hero" aria-hidden="true">
      {#if hero}<img src={hero} alt="" />{/if}
      <div class="fade"></div>
    </div>
    <button class="btn back" onclick={() => app.goBack()}>
      <Icon name="chevronLeft" />{backLabel}
    </button>
    <div class="top">
      <button class="cover" onclick={() => (picker = "cover")} title="Cover ändern" aria-label="Cover ändern">
        <Cover {game} radius={18} />
        <span class="cover-hint"><Icon name="image" size={16} />{dropping ? "Bild hier ablegen" : "Cover ändern"}</span>
      </button>
      <div class="info">
        <div class="pills">
          <span class="pill">{platformLabel(game.platform)}</span>
          {#if game.genre}<span class="pill">{game.genre}</span>{/if}
          {#if game.releaseYear}<span class="pill">{game.releaseYear}</span>{/if}
          {#if game.developer}<span class="pill">{game.developer}</span>{/if}
          {#if game.hidden}<span class="pill warn">Versteckt</span>{/if}
        </div>
        {#if renaming}
          <form class="rename" onsubmit={saveRename}>
            <label class="sr-only" for="rename">Name</label>
            <!-- svelte-ignore a11y_autofocus -->
            <input id="rename" class="input" bind:value={nameDraft} autofocus />
            <button class="btn primary" type="submit">Speichern</button>
            <button class="btn" type="button" onclick={() => (renaming = false)}>Abbrechen</button>
          </form>
        {:else}
          <h1>{game.name}</h1>
        {/if}
        <div class="actions">
          <button class="btn big primary" onclick={() => app.launch(game.id)}>
            <svg aria-hidden="true" width="18" height="18" viewBox="0 0 24 24" fill="currentColor"><path d="M7 4v16l13-8z" /></svg>
            {app.running.includes(game.id) ? "Läuft gerade" : "Spielen"}
          </button>
          <button
            class="btn big icon fav"
            class:on={game.favorite}
            onclick={() => patch({ favorite: !game.favorite })}
            aria-label={game.favorite ? "Aus Favoriten entfernen" : "Zu Favoriten"}
            aria-pressed={game.favorite}>
            <Icon name="star" size={22} fill={game.favorite} />
          </button>
          <div class="menu-wrap">
            <button
              class="btn big icon"
              aria-label="Mehr Optionen"
              aria-expanded={menuOpen}
              onclick={(e) => {
                e.stopPropagation();
                menuOpen = !menuOpen;
              }}><Icon name="more" size={22} stroke={3} /></button>
            {#if menuOpen}
              <div class="menu" role="menu">
                <button role="menuitem" onclick={startRename}><Icon name="edit" />Umbenennen</button>
                <button role="menuitem" onclick={() => { picker = "cover"; menuOpen = false; }}><Icon name="image" />Cover ändern</button>
                <button role="menuitem" onclick={() => { picker = "hero"; menuOpen = false; }}><Icon name="image" />Hintergrundbild ändern</button>
                <button role="menuitem" onclick={refresh} disabled={refreshing}><Icon name="refresh" />Infos neu laden</button>
                <button role="menuitem" onclick={() => patch({ hidden: !game.hidden })}>
                  <Icon name={game.hidden ? "eye" : "eyeOff"} />{game.hidden ? "Wieder anzeigen" : "Verstecken"}
                </button>
                <button role="menuitem" class="danger" onclick={undo}>
                  <Icon name={game.source === "desktop" && game.journalId ? "undo" : "trash"} />{game.source === "desktop" && game.journalId ? "Zurück auf den Desktop" : "Aus Bibliothek entfernen"}
                </button>
              </div>
            {/if}
          </div>
        </div>
      </div>
      <dl class="stats">
        <div><dt>Spielzeit</dt><dd>{formatPlaytime(game.playtimeSecs)}</dd></div>
        <div><dt>Zuletzt gespielt</dt><dd>{relativeDay(game.lastPlayed)}</dd></div>
        <div><dt>Starts</dt><dd>{game.playCount}</dd></div>
      </dl>
    </div>
    <div class="cols">
      <PlaytimePanel {game} />
      <TrackingPanel {game} />
    </div>
    {#if game.summary}
      <section class="panel">
        <h2>Über das Spiel</h2>
        <p class="summary">{game.summary}</p>
      </section>
    {/if}
    <div class="cols">
      <section class="panel">
        <h2>Sammlungen</h2>
        <CollectionsPanel {game} />
      </section>
      <section class="panel">
        <h2>Tags</h2>
        <TagEditor {game} />
      </section>
    </div>
  </div>
  {#if picker}
    <ArtPicker {game} kind={picker} onclose={() => (picker = null)} />
  {/if}
{/if}

<style>
  .detail {
    position: relative;
    flex: 1;
    height: 100vh;
    padding: 32px clamp(24px, 5vw, 120px) 64px;
    display: flex;
    flex-direction: column;
    gap: 40px;
  }
  .back {
    position: relative;
    align-self: flex-start;
    background: rgba(14, 15, 18, 0.6);
  }
  .hero {
    position: absolute;
    left: 0;
    right: 0;
    top: 0;
    height: 520px;
    background: linear-gradient(115deg, var(--a) 0%, var(--b) 75%, var(--bg) 100%);
    overflow: hidden;
  }
  .hero img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    object-position: center 25%;
  }
  .fade {
    position: absolute;
    inset: 0;
    background: linear-gradient(180deg, rgba(14, 15, 18, 0.15) 0%, rgba(14, 15, 18, 0.55) 55%, var(--bg) 100%);
  }
  .top,
  .panel {
    position: relative;
  }
  .top {
    margin-top: 180px;
  }
  .cols {
    position: relative;
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(440px, 1fr));
    gap: 28px;
    align-items: start;
  }
  .summary {
    margin: 0;
    line-height: 1.6;
    color: var(--text-2);
    max-width: 960px;
  }
  .top {
    display: flex;
    gap: 48px;
    align-items: flex-end;
    flex-wrap: wrap;
  }
  .cover {
    position: relative;
    width: 260px;
    flex-shrink: 0;
    padding: 0;
    border: 0;
    background: none;
    border-radius: 18px;
  }
  .cover-hint {
    position: absolute;
    left: 50%;
    bottom: 16px;
    transform: translateX(-50%);
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 7px 12px;
    border-radius: 999px;
    background: rgba(10, 11, 14, 0.85);
    font-size: 13px;
    font-weight: 600;
    white-space: nowrap;
    opacity: 0;
    transition: opacity 0.15s;
  }
  .cover:hover .cover-hint,
  .cover:focus-visible .cover-hint {
    opacity: 1;
  }
  .info {
    flex: 1;
    min-width: 320px;
    display: flex;
    flex-direction: column;
    gap: 20px;
  }
  .pills {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }
  .pill {
    padding: 5px 12px;
    border-radius: 999px;
    background: var(--surface-2);
    font-size: 13px;
    color: var(--text-2);
  }
  .pill.warn {
    color: var(--star);
  }
  h1 {
    font-size: clamp(40px, 4.4vw, 76px);
    line-height: 1;
  }
  .rename {
    display: flex;
    gap: 10px;
    max-width: 640px;
  }
  .rename .input {
    font-size: 22px;
    font-family: var(--font-display);
  }
  .actions {
    display: flex;
    gap: 12px;
    align-items: center;
  }
  .btn.big.icon {
    padding: 14px;
  }
  .fav.on {
    color: var(--star);
  }
  .menu-wrap {
    position: relative;
  }
  .menu {
    position: absolute;
    top: calc(100% + 8px);
    left: 0;
    z-index: 20;
    min-width: 240px;
    padding: 6px;
    border-radius: 12px;
    background: var(--surface-2);
    border: 1px solid var(--border-strong);
    box-shadow: 0 16px 40px rgba(0, 0, 0, 0.5);
    display: flex;
    flex-direction: column;
  }
  .menu button {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 10px 12px;
    border: 0;
    border-radius: 8px;
    background: transparent;
    text-align: left;
    font-size: 14px;
  }
  .menu button:disabled {
    opacity: 0.5;
  }
  .menu button:hover {
    background: var(--surface-3);
  }
  .menu .danger {
    color: var(--danger);
  }
  .stats {
    margin: 0;
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    border-radius: 18px;
    background: var(--surface);
    border: 1px solid var(--border);
    min-width: 480px;
  }
  .stats div {
    padding: 22px 26px;
    display: flex;
    flex-direction: column-reverse;
    gap: 6px;
  }
  .stats div + div {
    border-left: 1px solid var(--border);
  }
  dt {
    font-size: 13px;
    color: var(--text-muted);
  }
  dd {
    margin: 0;
    font-family: var(--font-display);
    font-size: 30px;
    font-weight: 600;
    white-space: nowrap;
  }
  .panel {
    padding: 26px 30px;
    border-radius: 20px;
    background: var(--surface);
    border: 1px solid var(--border);
    display: flex;
    flex-direction: column;
    gap: 16px;
  }
  .panel h2 {
    font-size: 20px;
    font-weight: 600;
  }
</style>
