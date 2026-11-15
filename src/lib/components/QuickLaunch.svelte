<script lang="ts">
  import { t } from "$lib/i18n.svelte";
  import { app } from "$lib/state.svelte";
  import { platformLabel, type Game } from "$lib/types";
  import { relativeDay } from "$lib/format";
  import Cover from "./Cover.svelte";
  import Icon from "./Icon.svelte";

  const LIMIT = 8;

  let query = $state("");
  let index = $state(0);
  let list = $state<HTMLElement>();

  /** Lowercase, without accents, ™ or punctuation: "Baldur's" finds "baldurs". */
  const norm = (s: string) =>
    s
      .normalize("NFKD")
      .toLowerCase()
      .replace(/[^\p{L}\p{N} ]+/gu, "")
      .replace(/\s+/g, " ")
      .trim();

  /** Lower is better; null means no match. */
  function rank(name: string, q: string): number | null {
    if (name.startsWith(q)) return 0;
    if (name.split(" ").some((w) => w.startsWith(q))) return 1;
    if (name.includes(q)) return 2;
    if (name.replace(/ /g, "").includes(q.replace(/ /g, ""))) return 2;
    // Initials, e.g. "bg3" for "Baldurs Gate 3".
    const initials = name
      .split(" ")
      .map((w) => (/^\d+$/.test(w) ? w : w[0]))
      .join("");
    if (initials.startsWith(q.replace(/ /g, ""))) return 3;
    return null;
  }

  let results = $derived.by((): Game[] => {
    const games = app.games.filter((g) => !g.hidden);
    const byRecent = (a: Game, b: Game) => (b.lastPlayed ?? 0) - (a.lastPlayed ?? 0) || a.name.localeCompare(b.name);
    const q = norm(query);
    if (!q) return games.filter((g) => g.lastPlayed).sort(byRecent).slice(0, LIMIT);
    return games
      .map((g) => ({ g, r: rank(norm(g.name), q) }))
      .filter((x): x is { g: Game; r: number } => x.r !== null)
      .sort((a, b) => a.r - b.r || byRecent(a.g, b.g))
      .slice(0, LIMIT)
      .map((x) => x.g);
  });

  $effect(() => {
    // A new search starts at the top.
    void query;
    index = 0;
  });

  $effect(() => {
    list?.querySelector(`[data-i="${index}"]`)?.scrollIntoView({ block: "nearest" });
  });

  // Focus after the click that opened the box has finished.
  function focus(el: HTMLInputElement) {
    el.focus();
    const id = requestAnimationFrame(() => el.focus());
    return { destroy: () => cancelAnimationFrame(id) };
  }

  const close = () => (app.quickOpen = false);

  function play(g: Game) {
    close();
    app.launch(g.id);
  }

  function details(g: Game) {
    close();
    app.openGame(g.id);
  }

  function onkeydown(e: KeyboardEvent) {
    if (e.key === "Escape") {
      e.preventDefault();
      close();
    } else if (e.key === "ArrowDown") {
      e.preventDefault();
      if (results.length) index = (index + 1) % results.length;
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      if (results.length) index = (index - 1 + results.length) % results.length;
    } else if (e.key === "Enter") {
      e.preventDefault();
      const g = results[index];
      if (!g) return;
      if (e.ctrlKey || e.metaKey || e.shiftKey) details(g);
      else play(g);
    }
  }
</script>

<div class="backdrop" role="presentation" onclick={(e) => e.target === e.currentTarget && close()}>
  <div class="box" role="dialog" aria-modal="true" aria-label={t("Schnellstart")}>
    <label class="field">
      <Icon name="search" size={22} />
      <span class="sr-only">{t("Spiel suchen")}</span>
      <input
        use:focus
        type="text"
        placeholder={t("Spiel suchen und starten …")}
        bind:value={query}
        {onkeydown}
        autocomplete="off"
        spellcheck="false"
        role="combobox"
        aria-expanded={results.length > 0}
        aria-controls="quick-results"
        aria-activedescendant={results[index] ? `quick-${results[index].id}` : undefined} />
      <kbd>Esc</kbd>
    </label>

    {#if results.length}
      {#if !query.trim()}<div class="label">{t("Zuletzt gespielt")}</div>{/if}
      <ul id="quick-results" class="results scroll" role="listbox" bind:this={list}>
        {#each results as g, i (g.id)}
          <li
            id="quick-{g.id}"
            data-i={i}
            role="option"
            aria-selected={i === index}
            class:on={i === index}
            onmousemove={() => (index = i)}>
            <button class="main" tabindex="-1" onclick={() => play(g)}>
              <span class="mini"><Cover game={g} showBadge={false} radius={6} /></span>
              <span class="text">
                <span class="name">{g.name}</span>
                <span class="meta muted">
                  {[platformLabel(g.platform), g.lastPlayed ? relativeDay(g.lastPlayed) : null, g.installed ? null : t("Nicht installiert")].filter(Boolean).join(" · ")}
                </span>
              </span>
              {#if app.running.includes(g.id)}<span class="badge">{t("Läuft gerade")}</span>{/if}
              <span class="go"><Icon name="arrowRight" size={16} />{t("Spielen")}</span>
            </button>
            <button class="info" tabindex="-1" onclick={() => details(g)} aria-label={t("Details zu {name}", { name: g.name })} title={t("Details")}>
              <Icon name="more" size={18} stroke={3} />
            </button>
          </li>
        {/each}
      </ul>
    {:else if query.trim()}
      <p class="empty muted">{t("Kein Spiel gefunden.")}</p>
    {/if}

    <footer class="muted">
      <span><kbd>↑</kbd><kbd>↓</kbd> {t("Auswählen")}</span>
      <span><kbd>Enter</kbd> {t("Spielen")}</span>
      <span><kbd>{t("Strg")}</kbd><kbd>Enter</kbd> {t("Details")}</span>
    </footer>
  </div>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 60;
    background: rgb(5 6 8 / 0.55);
    backdrop-filter: blur(6px);
    display: flex;
    justify-content: center;
    align-items: flex-start;
    padding: 14vh 24px 24px;
  }
  .box {
    width: min(640px, 100%);
    max-height: 100%;
    display: flex;
    flex-direction: column;
    border-radius: 18px;
    background: var(--bg-raised);
    border: 1px solid var(--border-strong);
    box-shadow: 0 30px 80px rgb(0 0 0 / 0.6);
    overflow: hidden;
    animation: pop 0.14s ease-out;
  }
  @keyframes pop {
    from {
      opacity: 0;
      transform: translateY(-8px) scale(0.98);
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .box {
      animation: none;
    }
  }
  .field {
    display: flex;
    align-items: center;
    gap: 14px;
    padding: 18px 20px;
    color: var(--text-muted);
    border-bottom: 1px solid var(--border);
  }
  .field input {
    flex: 1;
    min-width: 0;
    background: transparent;
    border: 0;
    outline: none;
    font-size: 20px;
    color: var(--text);
  }
  .label {
    padding: 12px 20px 2px;
    font-size: 11px;
    font-weight: 600;
    letter-spacing: 1px;
    text-transform: uppercase;
    color: var(--text-muted);
  }
  .results {
    list-style: none;
    margin: 0;
    padding: 8px;
    overflow-y: auto;
  }
  li {
    display: flex;
    align-items: center;
    border-radius: 12px;
  }
  li.on {
    background: var(--surface-2);
  }
  .main {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: center;
    gap: 14px;
    padding: 8px 10px;
    border: 0;
    background: transparent;
    text-align: left;
    color: inherit;
  }
  .mini {
    width: 36px;
    height: 54px;
    flex-shrink: 0;
  }
  .mini :global(.cover) {
    box-shadow: none;
  }
  .mini :global(.title) {
    display: none;
  }
  .text {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 3px;
  }
  .name {
    font-weight: 600;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .meta {
    font-size: 13px;
  }
  .badge {
    padding: 3px 8px;
    border-radius: 999px;
    background: var(--accent);
    color: var(--on-accent);
    font-size: 11px;
    font-weight: 600;
  }
  .go {
    display: none;
    align-items: center;
    gap: 6px;
    font-size: 13px;
    font-weight: 600;
    color: var(--accent);
  }
  li.on .go {
    display: flex;
  }
  .info {
    display: grid;
    place-items: center;
    width: 36px;
    height: 36px;
    margin-right: 6px;
    border: 0;
    border-radius: 8px;
    background: transparent;
    color: var(--text-muted);
  }
  .info:hover {
    background: var(--surface-3);
    color: var(--text);
  }
  .empty {
    margin: 0;
    padding: 20px;
  }
  footer {
    display: flex;
    gap: 18px;
    padding: 10px 20px;
    border-top: 1px solid var(--border);
    font-size: 12px;
  }
  kbd {
    display: inline-block;
    min-width: 18px;
    margin-right: 3px;
    padding: 1px 6px;
    border-radius: 5px;
    border: 1px solid var(--border-strong);
    background: var(--surface);
    font: 600 11px/1.5 inherit;
    text-align: center;
    color: var(--text-2);
  }
</style>
