<script lang="ts">
  import { t } from "$lib/i18n.svelte";
  import { app } from "$lib/state.svelte";
  import { formatPlaytime, relativeDay } from "$lib/format";
  import type { Game } from "$lib/types";
  import Cover from "./Cover.svelte";

  let { games, size = 200 }: { games: Game[]; size?: number } = $props();

  function meta(g: Game) {
    if (app.running.includes(g.id)) return t("Läuft gerade");
    if (!g.installed) return t("Nicht installiert");
    if (!g.lastPlayed) return t("Noch nicht gespielt");
    return `${formatPlaytime(g.playtimeSecs)} · ${relativeDay(g.lastPlayed)}`;
  }
</script>

<ul class="grid" style="--size: {size}px">
  {#each games as g (g.id)}
    <li
      class="card"
      class:uninstalled={!g.installed}
      draggable="true"
      ondragstart={(e) => {
        e.dataTransfer?.setData("application/x-game-id", String(g.id));
        e.dataTransfer?.setData("text/plain", g.name);
      }}>
      <button class="cover-btn" onclick={() => (app.openGame(g.id))} aria-label={t("{name} öffnen", { name: g.name })}>
        <Cover game={g} />
      </button>
      <button class="play" onclick={() => app.launch(g.id)} aria-label={t("{name} spielen", { name: g.name })} title={t("Spielen")}>
        <svg aria-hidden="true" width="18" height="18" viewBox="0 0 24 24" fill="currentColor"><path d="M7 4v16l13-8z" /></svg>
      </button>
      <div class="info">
        <span class="name" title={g.name}>
          {#if g.favorite}<span class="fav" aria-label={t("Favorit")}>★</span>{/if}{g.name}
        </span>
        <span class="meta" class:running={app.running.includes(g.id)}>{meta(g)}</span>
      </div>
    </li>
  {/each}
</ul>

<style>
  .grid {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(var(--size), 1fr));
    gap: 32px 24px;
  }
  .card {
    position: relative;
    display: flex;
    flex-direction: column;
    gap: 10px;
    min-width: 0;
  }
  .cover-btn {
    padding: 0;
    border: 0;
    background: none;
    border-radius: 14px;
    transition: transform 0.18s ease;
  }
  .card:hover .cover-btn,
  .cover-btn:focus-visible {
    transform: translateY(-4px);
  }
  .play {
    position: absolute;
    right: 12px;
    top: calc(var(--size) * 1.5 - 60px);
    width: 48px;
    height: 48px;
    border-radius: 999px;
    border: 0;
    background: var(--accent);
    color: var(--on-accent);
    display: grid;
    place-items: center;
    opacity: 0;
    transform: translateY(6px);
    transition: opacity 0.18s, transform 0.18s;
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.45);
  }
  .card:hover .play,
  .play:focus-visible {
    opacity: 1;
    transform: translateY(-4px);
  }
  .info {
    display: flex;
    flex-direction: column;
    gap: 3px;
    padding: 0 2px;
    min-width: 0;
  }
  .name {
    font-weight: 600;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .fav {
    color: var(--star);
    margin-right: 6px;
  }
  .meta {
    font-size: 13px;
    color: var(--text-muted);
  }
  .uninstalled .cover-btn {
    opacity: 0.45;
    filter: grayscale(0.8);
  }
  .meta.running {
    color: var(--accent);
    font-weight: 600;
  }
</style>
