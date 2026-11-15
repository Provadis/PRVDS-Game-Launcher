<script lang="ts">
  import { untrack } from "svelte";
  import { api } from "$lib/api";
  import { app } from "$lib/state.svelte";
  import { MAX_FIELD_COLS, MAX_FIELD_ROWS, type Board, type BoardField, type Game } from "$lib/types";
  import Cover from "./Cover.svelte";
  import Icon from "./Icon.svelte";

  let { id }: { id: number } = $props();

  // Geometry (px). Fields sit on a 24 px grid, covers are 2:3.
  const STEP = 24;
  const CW = 132;
  const CH = 198;
  const GAP = 14;
  const PAD = 16;
  const HEAD = 46;
  // Sizes are rounded up to whole grid steps, so field edges line up with
  // each other; the covers sit centred in the extra space.
  const toStep = (px: number) => Math.ceil(px / STEP) * STEP;
  const fieldW = (f: BoardField) => toStep(f.cols * CW + (f.cols - 1) * GAP + 2 * PAD);
  const fieldH = (f: BoardField) => toStep(HEAD + f.rows * CH + (f.rows - 1) * GAP + 2 * PAD);

  let board = $derived(app.boards.find((b) => b.id === id) ?? null);
  let draft = $state<Board | null>(null);
  let shown = $derived(draft ?? board);
  let editing = $derived(draft !== null);
  let saving = $state(false);
  let search = $state("");
  let dropSlot = $state<string | null>(null);
  /** The field being moved or resized, and whether it sits on another one. */
  let dragging = $state<{ id: string; blocked: boolean } | null>(null);

  let filled = $derived(shown ? shown.fields.reduce((n, f) => n + f.slots.length, 0) : 0);
  let canvasW = $derived(
    shown ? Math.max(0, ...shown.fields.map((f) => f.x * STEP + fieldW(f))) + (editing ? 240 : 0) : 0,
  );
  let canvasH = $derived(
    shown ? Math.max(0, ...shown.fields.map((f) => f.y * STEP + fieldH(f))) + (editing ? 320 : 0) : 0,
  );
  let pool = $derived(
    app.games
      .filter((g) => !g.hidden && g.name.toLowerCase().includes(search.trim().toLowerCase()))
      .sort((a, b) => a.name.localeCompare(b.name)),
  );

  function startEdit() {
    if (board) draft = structuredClone($state.snapshot(board));
  }

  function cancel() {
    draft = null;
  }

  async function save() {
    if (!draft) return;
    saving = true;
    try {
      const saved = await api.saveBoard($state.snapshot(draft));
      app.boards = app.boards.map((b) => (b.id === saved.id ? saved : b));
      draft = null;
    } catch (e) {
      app.error(e);
    } finally {
      saving = false;
    }
  }

  // ---- fields --------------------------------------------------------------

  function rect(f: BoardField, x = f.x, y = f.y, cols = f.cols, rows = f.rows) {
    const g = { ...f, cols, rows };
    return { l: x * STEP, t: y * STEP, r: x * STEP + fieldW(g), b: y * STEP + fieldH(g) };
  }

  function overlaps(f: BoardField, x: number, y: number, cols: number, rows: number) {
    const a = rect(f, x, y, cols, rows);
    return (draft?.fields ?? []).some((o) => {
      if (o.id === f.id) return false;
      const b = rect(o);
      // Keep at least one grid step between fields.
      return a.l < b.r + STEP && b.l < a.r + STEP && a.t < b.b + STEP && b.t < a.b + STEP;
    });
  }

  function addField() {
    if (!draft) return;
    const ids = new Set(draft.fields.map((f) => f.id));
    let n = draft.fields.length;
    while (ids.has(`f${n}`)) n++;
    const bottom = Math.max(0, ...draft.fields.map((f) => Math.ceil(rect(f).b / STEP) + 1));
    draft.fields.push({ id: `f${n}`, title: "Neues Feld", x: 0, y: bottom, cols: 4, rows: 1, slots: [] });
  }

  function removeField(f: BoardField) {
    if (!draft) return;
    if (f.slots.length && !confirm(`Feld „${f.title || "ohne Titel"}“ mit ${f.slots.length} Spielen entfernen?`)) return;
    draft.fields = draft.fields.filter((x) => x.id !== f.id);
  }

  /** Drag a field by its handle (move) or its corner (resize), snapping to
   * the grid. It may pass over other fields; it glows red while it would
   * overlap one, and snaps back if let go there. */
  function startDrag(e: PointerEvent, f: BoardField, mode: "move" | "resize") {
    if (!draft || e.button !== 0) return;
    e.preventDefault();
    const el = e.currentTarget as HTMLElement;
    el.setPointerCapture(e.pointerId);
    const start = { px: e.clientX, py: e.clientY, x: f.x, y: f.y, cols: f.cols, rows: f.rows };
    // Shrinking never cuts off a placed game.
    const minCols = Math.max(1, ...f.slots.map((s) => s.col + 1));
    const minRows = Math.max(1, ...f.slots.map((s) => s.row + 1));
    dragging = { id: f.id, blocked: false };
    const move = (ev: PointerEvent) => {
      const dx = ev.clientX - start.px;
      const dy = ev.clientY - start.py;
      if (mode === "move") {
        f.x = Math.max(0, start.x + Math.round(dx / STEP));
        f.y = Math.max(0, start.y + Math.round(dy / STEP));
      } else {
        f.cols = Math.min(MAX_FIELD_COLS, Math.max(minCols, start.cols + Math.round(dx / (CW + GAP))));
        f.rows = Math.min(MAX_FIELD_ROWS, Math.max(minRows, start.rows + Math.round(dy / (CH + GAP))));
      }
      dragging = { id: f.id, blocked: overlaps(f, f.x, f.y, f.cols, f.rows) };
    };
    const up = () => {
      if (dragging?.blocked) {
        f.x = start.x;
        f.y = start.y;
        f.cols = start.cols;
        f.rows = start.rows;
      }
      dragging = null;
      el.removeEventListener("pointermove", move);
      el.removeEventListener("pointerup", up);
      el.removeEventListener("pointercancel", up);
    };
    el.addEventListener("pointermove", move);
    el.addEventListener("pointerup", up);
    el.addEventListener("pointercancel", up);
  }

  // ---- slots ---------------------------------------------------------------

  const SLOT_MIME = "application/x-board-slot";
  const GAME_MIME = "application/x-game-id";

  function gameAt(f: BoardField, col: number, row: number): Game | null {
    const s = f.slots.find((x) => x.col === col && x.row === row);
    return s ? (app.byId.get(s.gameId) ?? null) : null;
  }

  function slotKey(f: BoardField, col: number, row: number) {
    return `${f.id}:${col}:${row}`;
  }

  function setSlot(f: BoardField, col: number, row: number, gameId: number | null) {
    f.slots = f.slots.filter((s) => !(s.col === col && s.row === row));
    if (gameId !== null) f.slots.push({ col, row, gameId });
  }

  function dragGame(e: DragEvent, gameId: number, from?: string) {
    e.dataTransfer?.setData(GAME_MIME, String(gameId));
    if (from) e.dataTransfer?.setData(SLOT_MIME, from);
    if (e.dataTransfer) e.dataTransfer.effectAllowed = "copyMove";
  }

  function canDrop(e: DragEvent) {
    return !!draft && !!e.dataTransfer?.types.includes(GAME_MIME);
  }

  function dropOnSlot(e: DragEvent, f: BoardField, col: number, row: number) {
    e.preventDefault();
    dropSlot = null;
    if (!draft) return;
    const gameId = Number(e.dataTransfer?.getData(GAME_MIME));
    if (!gameId) return;
    const from = e.dataTransfer?.getData(SLOT_MIME);
    const here = f.slots.find((s) => s.col === col && s.row === row)?.gameId ?? null;
    if (from) {
      // Moving within the board: swap with whatever sits here.
      const [fid, c, r] = from.split(":");
      const src = draft.fields.find((x) => x.id === fid);
      if (!src || (src.id === f.id && Number(c) === col && Number(r) === row)) return;
      setSlot(src, Number(c), Number(r), here);
    }
    setSlot(f, col, row, gameId);
  }

  function dropOnPool(e: DragEvent) {
    e.preventDefault();
    const from = e.dataTransfer?.getData(SLOT_MIME);
    if (!from || !draft) return;
    const [fid, c, r] = from.split(":");
    const src = draft.fields.find((x) => x.id === fid);
    if (src) setSlot(src, Number(c), Number(r), null);
  }

  // Leave edit mode when switching to another board.
  $effect(() => {
    void id;
    untrack(() => (draft = null));
  });
</script>

<svelte:window onkeydown={(e) => editing && e.key === "Escape" && cancel()} />

{#if shown}
  <div class="board-screen">
    <main class="scroll">
      <div class="bar">
        {#if draft}
          <input class="name-input" bind:value={draft.name} aria-label="Name des Boards" />
          <span class="badge">Bearbeiten</span>
          <span class="spacer"></span>
          <button class="btn" onclick={addField}><Icon name="plus" size={16} />Feld hinzufügen</button>
          <button class="btn" onclick={cancel}>Abbrechen</button>
          <button class="btn primary" onclick={save} disabled={saving || !draft.name.trim()}>{saving ? "Speichert …" : "Fertig"}</button>
        {:else}
          <h1>{shown.name}</h1>
          <span class="muted">{shown.fields.length} {shown.fields.length === 1 ? "Feld" : "Felder"} · {filled} {filled === 1 ? "Platz" : "Plätze"} belegt</span>
          <span class="spacer"></span>
          <button class="btn" onclick={startEdit}><Icon name="edit" size={16} />Bearbeiten</button>
        {/if}
      </div>

      {#if editing}
        <p class="muted hint">Spiele aus der Liste rechts in einen Platz ziehen, Felder am Griff verschieben und an der Ecke größer ziehen. Leere Plätze bleiben als Lücke.</p>
      {/if}

      {#if !shown.fields.length && !editing}
        <div class="empty">
          <p class="muted">Dieses Board ist noch leer.</p>
          <button class="btn primary" onclick={() => { startEdit(); addField(); }}><Icon name="plus" size={16} />Erstes Feld anlegen</button>
        </div>
      {/if}

      <div class="canvas" class:editing style="width: {canvasW}px; height: {canvasH}px; --cw: {CW}px; --ch: {CH}px; --gap: {GAP}px; --pad: {PAD}px; --head: {HEAD}px; --step: {STEP}px">
        {#each shown.fields as f (f.id)}
          <section
            class="field"
            class:dragging={dragging?.id === f.id}
            class:blocked={dragging?.id === f.id && dragging.blocked}
            style="left: {f.x * STEP}px; top: {f.y * STEP}px; width: {fieldW(f)}px; height: {fieldH(f)}px" aria-label={f.title || "Feld"}>
            <header>
              {#if editing}
                <button class="handle" onpointerdown={(e) => startDrag(e, f, "move")} aria-label="Feld verschieben" title="Verschieben">
                  <Icon name="grip" size={18} />
                </button>
                <input class="title-input" bind:value={f.title} placeholder="Titel" aria-label="Titel des Felds" />
                <span class="muted size">{f.cols} × {f.rows}</span>
                <button class="icon-btn" onclick={() => removeField(f)} aria-label="Feld entfernen" title="Feld entfernen"><Icon name="trash" size={15} /></button>
              {:else}
                <h2>{f.title}</h2>
                <span class="muted">{f.slots.length} {f.slots.length === 1 ? "Spiel" : "Spiele"}</span>
              {/if}
            </header>
            <div class="slots" style="grid-template-columns: repeat({f.cols}, var(--cw)); grid-template-rows: repeat({f.rows}, var(--ch))">
              {#each { length: f.rows } as _, row}
                {#each { length: f.cols } as _, col}
                  {@const g = gameAt(f, col, row)}
                  {@const key = slotKey(f, col, row)}
                  <div
                    class="slot"
                    class:filled={!!g}
                    class:drop={dropSlot === key}
                    role="listitem"
                    ondragover={(e) => {
                      if (canDrop(e)) {
                        e.preventDefault();
                        dropSlot = key;
                      }
                    }}
                    ondragleave={() => dropSlot === key && (dropSlot = null)}
                    ondrop={(e) => dropOnSlot(e, f, col, row)}>
                    {#if g}
                      <div class="game" class:dim={!g.installed} draggable={editing} ondragstart={(e) => dragGame(e, g.id, key)} role="presentation">
                        {#if editing}
                          <Cover game={g} showBadge={false} radius={10} />
                          <button class="remove" onclick={() => setSlot(f, col, row, null)} aria-label="{g.name} aus dem Feld nehmen" title="Entfernen"><Icon name="close" size={14} /></button>
                        {:else}
                          <button class="open" onclick={() => (app.screen = { kind: "detail", id: g.id })} aria-label="{g.name} öffnen" title={g.name}>
                            <Cover game={g} showBadge={false} radius={10} />
                          </button>
                          <button class="play" onclick={() => app.launch(g.id)} aria-label="{g.name} spielen" title="Spielen">
                            <svg aria-hidden="true" width="16" height="16" viewBox="0 0 24 24" fill="currentColor"><path d="M7 4v16l13-8z" /></svg>
                          </button>
                        {/if}
                      </div>
                    {:else if editing}
                      <span class="plus" aria-hidden="true">+</span>
                    {/if}
                  </div>
                {/each}
              {/each}
            </div>
            {#if editing}
              <button class="resize" onpointerdown={(e) => startDrag(e, f, "resize")} aria-label="Feldgröße ändern" title="Größe ändern"></button>
            {/if}
          </section>
        {/each}
      </div>
    </main>

    {#if editing}
      <aside
        class="pool"
        aria-label="Spiele"
        ondragover={(e) => e.dataTransfer?.types.includes(SLOT_MIME) && e.preventDefault()}
        ondrop={dropOnPool}>
        <h2>Spiele</h2>
        <input class="input" type="search" placeholder="Suchen …" bind:value={search} />
        <p class="muted small">Ein Spiel darf in mehreren Feldern und Boards stehen. Zum Entfernen hierher zurückziehen.</p>
        <ul class="scroll">
          {#each pool as g (g.id)}
            <li draggable="true" ondragstart={(e) => dragGame(e, g.id)}>
              <span class="mini"><Cover game={g} showBadge={false} radius={5} /></span>
              <span class="pname" title={g.name}>{g.name}</span>
            </li>
          {/each}
        </ul>
      </aside>
    {/if}
  </div>
{:else}
  <main class="scroll"><p class="muted">Dieses Board gibt es nicht mehr.</p></main>
{/if}

<style>
  .board-screen {
    flex: 1;
    min-width: 0;
    display: flex;
    height: 100vh;
  }
  main {
    flex: 1;
    min-width: 0;
    overflow: auto;
    padding: 36px 48px;
    display: flex;
    flex-direction: column;
    gap: 28px;
  }
  .bar {
    display: flex;
    align-items: center;
    gap: 14px;
    flex-wrap: wrap;
  }
  h1 {
    font-size: 28px;
  }
  .name-input {
    font-family: var(--font-display);
    font-size: 26px;
    font-weight: 600;
    background: transparent;
    border: 0;
    border-bottom: 2px solid var(--accent);
    color: var(--text);
    padding: 2px 0;
    min-width: 0;
    width: 320px;
    outline: none;
  }
  .badge {
    padding: 5px 12px;
    border-radius: 999px;
    background: var(--surface-3);
    font-size: 13px;
    font-weight: 600;
  }
  .hint {
    font-size: 14px;
    margin: -16px 0 0;
  }
  .spacer {
    flex: 1;
  }
  .empty {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 12px;
  }
  .canvas {
    position: relative;
    flex-shrink: 0;
    min-width: 100%;
  }
  .canvas.editing {
    background-image: radial-gradient(circle, var(--border) 1px, transparent 1px);
    background-size: var(--step) var(--step);
  }
  .field {
    position: absolute;
    box-sizing: border-box;
    padding: 0 var(--pad) var(--pad);
    border-radius: 18px;
    background: var(--surface);
    border: 1px solid var(--border);
  }
  .editing .field {
    border: 2px dashed var(--border-strong);
  }
  .field {
    display: flex;
    flex-direction: column;
  }
  .field.dragging {
    z-index: 5;
    box-shadow: 0 12px 32px rgb(0 0 0 / 0.45);
  }
  .field.blocked {
    border: 2px solid #ef4444;
    box-shadow:
      0 0 0 4px rgb(239 68 68 / 0.25),
      0 0 28px rgb(239 68 68 / 0.45);
  }
  .field header {
    height: var(--head);
    display: flex;
    align-items: center;
    gap: 10px;
    min-width: 0;
  }
  .field h2 {
    font-size: 19px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .field header .muted {
    font-size: 13px;
    white-space: nowrap;
  }
  .handle {
    cursor: grab;
    touch-action: none;
    border: 0;
    background: none;
    color: var(--text-muted);
    padding: 4px;
    border-radius: 6px;
    display: grid;
    place-items: center;
  }
  .handle:active {
    cursor: grabbing;
  }
  .title-input {
    flex: 1;
    min-width: 0;
    font-family: var(--font-display);
    font-size: 18px;
    font-weight: 600;
    background: transparent;
    border: 0;
    border-bottom: 2px solid var(--accent);
    color: var(--text);
    outline: none;
  }
  .size {
    margin-left: auto;
  }
  .icon-btn {
    border: 0;
    background: none;
    color: var(--text-muted);
    padding: 4px;
    border-radius: 6px;
    display: grid;
    place-items: center;
  }
  .icon-btn:hover {
    color: var(--text);
    background: var(--surface-3);
  }
  .slots {
    flex: 1;
    display: grid;
    gap: var(--gap);
    justify-content: center;
    align-content: start;
  }
  .slot {
    position: relative;
    border-radius: 10px;
  }
  .editing .slot:not(.filled) {
    border: 2px dashed var(--border-strong);
    display: grid;
    place-items: center;
  }
  .slot.drop {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
  }
  .plus {
    color: var(--text-muted);
    font-size: 26px;
  }
  .game {
    position: relative;
    height: 100%;
  }
  .game.dim :global(.cover) {
    opacity: 0.45;
    filter: grayscale(0.8);
  }
  .editing .game {
    cursor: grab;
  }
  .open {
    padding: 0;
    border: 0;
    background: none;
    width: 100%;
    border-radius: 10px;
    transition: transform 0.18s ease;
  }
  .game:hover .open,
  .open:focus-visible {
    transform: translateY(-3px);
  }
  .play {
    position: absolute;
    right: 8px;
    bottom: 8px;
    width: 38px;
    height: 38px;
    border-radius: 999px;
    border: 0;
    background: var(--accent);
    color: var(--on-accent);
    display: grid;
    place-items: center;
    opacity: 0;
    transition: opacity 0.18s;
  }
  .game:hover .play,
  .play:focus-visible {
    opacity: 1;
  }
  .remove {
    position: absolute;
    top: 6px;
    right: 6px;
    width: 26px;
    height: 26px;
    border-radius: 8px;
    border: 0;
    background: rgba(10, 11, 14, 0.78);
    color: var(--text);
    display: grid;
    place-items: center;
  }
  .resize {
    position: absolute;
    right: 4px;
    bottom: 4px;
    width: 20px;
    height: 20px;
    border: 0;
    padding: 0;
    background: none;
    border-right: 3px solid var(--accent);
    border-bottom: 3px solid var(--accent);
    border-radius: 0 0 8px 0;
    cursor: nwse-resize;
    touch-action: none;
  }
  .pool {
    width: 320px;
    flex-shrink: 0;
    box-sizing: border-box;
    padding: 36px 20px 20px;
    border-left: 1px solid var(--border);
    background: var(--bg-raised);
    display: flex;
    flex-direction: column;
    gap: 12px;
    min-height: 0;
  }
  .pool h2 {
    font-size: 20px;
  }
  .small {
    font-size: 13px;
    margin: 0;
  }
  .pool ul {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 6px;
    flex: 1;
    min-height: 0;
  }
  .pool li {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 6px 8px;
    border-radius: 10px;
    background: var(--surface);
    cursor: grab;
  }
  .pool li:hover {
    background: var(--surface-2);
  }
  .mini {
    width: 30px;
    flex-shrink: 0;
  }
  .mini :global(.cover) {
    box-shadow: none;
  }
  .mini :global(.title) {
    display: none;
  }
  .pname {
    font-size: 14px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
</style>
