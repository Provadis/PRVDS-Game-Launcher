<script lang="ts">
  import { app } from "$lib/state.svelte";
  import { PLATFORMS, SORT_FIELDS, type Platform, type SortField } from "$lib/types";
  import Icon from "./Icon.svelte";

  let { title, count, onSaveView }: { title: string; count: number; onSaveView?: () => void } = $props();

  let filtersOpen = $state(false);

  let genres = $derived.by(() => {
    const set = new Set<string>();
    for (const g of app.games) for (const part of g.genre?.split(",") ?? []) if (part.trim()) set.add(part.trim());
    return [...set].sort((a, b) => a.localeCompare(b, "de"));
  });

  let activeFilters = $derived(
    (app.query.played !== "any" ? 1 : 0) +
      (app.query.notPlayedForDays ? 1 : 0) +
      (app.query.playedWithinDays ? 1 : 0) +
      app.query.tags.length +
      (app.query.genre ? 1 : 0) +
      (app.query.includeHidden ? 1 : 0),
  );

  function togglePlatform(p: Platform) {
    const list = app.query.platforms;
    app.query.platforms = list.includes(p) ? list.filter((x) => x !== p) : [...list, p];
  }

  function setSort(e: Event) {
    const field = (e.currentTarget as HTMLSelectElement).value as SortField;
    app.query.sort = field;
    app.query.descending = SORT_FIELDS.find((f) => f.id === field)?.defaultDesc ?? false;
  }

  function toggleTag(t: string) {
    const tags = app.query.tags;
    app.query.tags = tags.includes(t) ? tags.filter((x) => x !== t) : [...tags, t];
  }

  function resetFilters() {
    app.query.played = "any";
    app.query.notPlayedForDays = null;
    app.query.playedWithinDays = null;
    app.query.tags = [];
    app.query.genre = null;
    app.query.includeHidden = false;
  }

  const usedPlatforms = $derived(PLATFORMS.filter((p) => app.games.some((g) => g.platform === p.id)));
</script>

<div class="bar">
  <div class="row">
    <h2>{title}</h2>
    <span class="muted count">{count} {count === 1 ? "Spiel" : "Spiele"}</span>
    <label class="search">
      <Icon name="search" />
      <span class="sr-only">Suche</span>
      <input type="search" placeholder="Spiele durchsuchen" bind:value={app.query.search} />
    </label>
    <span class="spacer"></span>
    <label class="select">
      <span class="muted">Sortieren:</span>
      <select value={app.query.sort} onchange={setSort} aria-label="Sortieren nach">
        {#each SORT_FIELDS as f (f.id)}
          <option value={f.id}>{f.label}</option>
        {/each}
      </select>
      <Icon name="chevronDown" size={14} />
    </label>
    <button
      class="btn icon"
      onclick={() => (app.query.descending = !app.query.descending)}
      aria-label={app.query.descending ? "Absteigend" : "Aufsteigend"}
      title={app.query.descending ? "Absteigend" : "Aufsteigend"}>
      <Icon name={app.query.descending ? "arrowDown" : "arrowUp"} />
    </button>
    <button class="btn" class:on={filtersOpen || activeFilters} onclick={() => (filtersOpen = !filtersOpen)} aria-expanded={filtersOpen}>
      <Icon name="filter" size={16} />Filter{#if activeFilters}<span class="badge">{activeFilters}</span>{/if}
    </button>
    <div class="seg" role="group" aria-label="Layout">
      <button class:active={app.layout.mode === "grid"} onclick={() => (app.layout.mode = "grid")} aria-label="Grid">
        <Icon name="grid" />
      </button>
      <button class:active={app.layout.mode === "list"} onclick={() => (app.layout.mode = "list")} aria-label="Liste">
        <Icon name="list" />
      </button>
    </div>
  </div>

  <div class="row chips">
    {#if usedPlatforms.length > 1}
      <button class="chip" class:active={app.query.platforms.length === 0} onclick={() => (app.query.platforms = [])}>Alle</button>
      {#each usedPlatforms as p (p.id)}
        <button class="chip" class:active={app.query.platforms.includes(p.id)} onclick={() => togglePlatform(p.id)}>{p.label}</button>
      {/each}
    {/if}
    <span class="spacer"></span>
    {#if app.layout.mode === "grid"}
      <label class="size" title="Covergröße">
        <span class="sr-only">Covergröße</span>
        <Icon name="image" size={16} />
        <input type="range" min="140" max="320" step="10" bind:value={app.layout.coverSize} />
      </label>
    {/if}
    {#if onSaveView}
      <button class="btn" onclick={onSaveView}><Icon name="layers" size={16} />Ansicht speichern</button>
    {/if}
  </div>

  {#if filtersOpen}
    <div class="filters">
      <fieldset>
        <legend>Gespielt</legend>
        <div class="opts">
          <button class="chip" class:active={app.query.played === "any"} onclick={() => (app.query.played = "any")}>Alle</button>
          <button class="chip" class:active={app.query.played === "played"} onclick={() => (app.query.played = "played")}>Gespielt</button>
          <button class="chip" class:active={app.query.played === "neverPlayed"} onclick={() => (app.query.played = "neverPlayed")}>Noch nie gespielt</button>
        </div>
      </fieldset>
      <fieldset>
        <legend>Zeitraum</legend>
        <div class="opts">
          <label class="inline">
            Nicht gespielt seit
            <select
              value={app.query.notPlayedForDays ?? ""}
              onchange={(e) => (app.query.notPlayedForDays = e.currentTarget.value ? +e.currentTarget.value : null)}>
              <option value="">–</option>
              <option value="30">1 Monat</option>
              <option value="90">3 Monaten</option>
              <option value="180">6 Monaten</option>
              <option value="365">1 Jahr</option>
            </select>
          </label>
          <label class="inline">
            Gespielt in den letzten
            <select
              value={app.query.playedWithinDays ?? ""}
              onchange={(e) => (app.query.playedWithinDays = e.currentTarget.value ? +e.currentTarget.value : null)}>
              <option value="">–</option>
              <option value="7">7 Tagen</option>
              <option value="30">30 Tagen</option>
              <option value="90">90 Tagen</option>
            </select>
          </label>
        </div>
      </fieldset>
      {#if genres.length}
        <fieldset>
          <legend>Genre</legend>
          <select class="genre" value={app.query.genre ?? ""} onchange={(e) => (app.query.genre = e.currentTarget.value || null)}>
            <option value="">Alle Genres</option>
            {#each genres as g (g)}<option value={g}>{g}</option>{/each}
          </select>
        </fieldset>
      {/if}
      {#if app.tags.length}
        <fieldset>
          <legend>Tags</legend>
          <div class="opts">
            {#each app.tags as t (t)}
              <button class="chip" class:active={app.query.tags.includes(t)} onclick={() => toggleTag(t)}>{t}</button>
            {/each}
          </div>
        </fieldset>
      {/if}
      <fieldset>
        <legend>Sonstiges</legend>
        <label class="inline"><input type="checkbox" bind:checked={app.query.includeHidden} />Versteckte Spiele zeigen</label>
      </fieldset>
      <button class="btn reset" onclick={resetFilters} disabled={!activeFilters}>Filter zurücksetzen</button>
    </div>
  {/if}
</div>

<style>
  .bar {
    display: flex;
    flex-direction: column;
    gap: 16px;
  }
  .row {
    display: flex;
    align-items: center;
    gap: 12px;
    flex-wrap: wrap;
  }
  h2 {
    font-size: 28px;
    font-weight: 600;
  }
  .count {
    margin-right: 8px;
  }
  .spacer {
    flex: 1;
  }
  .search {
    display: flex;
    align-items: center;
    gap: 10px;
    width: 280px;
    max-width: 100%;
    padding: 10px 14px;
    border-radius: 12px;
    background: var(--surface);
    border: 1px solid var(--border);
    color: var(--text-muted);
  }
  .search:focus-within {
    border-color: var(--accent);
  }
  .search input {
    flex: 1;
    min-width: 0;
    background: transparent;
    border: 0;
    outline: none;
    color: var(--text);
  }
  .select {
    position: relative;
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 9px 14px;
    border-radius: 10px;
    background: var(--surface);
    border: 1px solid var(--border);
    font-size: 14px;
  }
  .select select {
    appearance: none;
    background: transparent;
    border: 0;
    outline: none;
    padding-right: 4px;
    font-weight: 600;
  }
  select option {
    background: var(--surface-2);
  }
  .btn.on {
    border-color: var(--accent);
  }
  .badge {
    min-width: 20px;
    height: 20px;
    padding: 0 6px;
    border-radius: 999px;
    background: var(--accent);
    color: var(--on-accent);
    font-size: 12px;
    display: grid;
    place-items: center;
  }
  .seg {
    display: flex;
    padding: 3px;
    border-radius: 10px;
    background: var(--surface);
    border: 1px solid var(--border);
  }
  .seg button {
    display: flex;
    padding: 7px 10px;
    border: 0;
    border-radius: 7px;
    background: transparent;
    color: var(--text-muted);
  }
  .seg button.active {
    background: var(--surface-3);
    color: var(--text);
  }
  .size {
    display: flex;
    align-items: center;
    gap: 8px;
    color: var(--text-muted);
  }
  .size input {
    width: 110px;
    accent-color: var(--accent);
  }
  .chips {
    gap: 8px;
  }
  .filters {
    display: flex;
    flex-wrap: wrap;
    gap: 20px 32px;
    padding: 20px 24px;
    border-radius: var(--radius);
    background: var(--surface);
    border: 1px solid var(--border);
    align-items: flex-end;
  }
  fieldset {
    border: 0;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  legend {
    padding: 0;
    margin-bottom: 10px;
    font-size: 12px;
    font-weight: 600;
    letter-spacing: 1px;
    text-transform: uppercase;
    color: var(--text-muted);
  }
  .opts {
    display: flex;
    gap: 8px;
    flex-wrap: wrap;
    align-items: center;
  }
  .inline {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 14px;
    color: var(--text-2);
  }
  .inline select,
  .genre {
    padding: 7px 10px;
    border-radius: 8px;
    background: var(--surface-2);
    border: 1px solid var(--border-strong);
  }
  .inline input {
    accent-color: var(--accent);
    width: 16px;
    height: 16px;
  }
  .reset {
    margin-left: auto;
  }
</style>
