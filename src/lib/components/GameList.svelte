<script lang="ts">
  import { app } from "$lib/state.svelte";
  import { formatPlaytime, relativeDay } from "$lib/format";
  import { platformLabel, type Game } from "$lib/types";
  import Cover from "./Cover.svelte";

  let { games }: { games: Game[] } = $props();
</script>

<table>
  <thead>
    <tr>
      <th class="c-name" colspan="2">Name</th>
      <th>Plattform</th>
      <th>Genre</th>
      <th class="num">Spielzeit</th>
      <th>Zuletzt gespielt</th>
      <th class="num">Starts</th>
      <th><span class="sr-only">Aktion</span></th>
    </tr>
  </thead>
  <tbody>
    {#each games as g (g.id)}
      <tr onclick={() => (app.screen = { kind: "detail", id: g.id })}>
        <td class="thumb"><Cover game={g} showBadge={false} radius={6} /></td>
        <td class="c-name">
          <button class="link" onclick={(e) => { e.stopPropagation(); app.screen = { kind: "detail", id: g.id }; }}>
            {#if g.favorite}<span class="fav" aria-label="Favorit">★</span>{/if}{g.name}
          </button>
          {#if g.tags.length}<span class="tags">{g.tags.join(" · ")}</span>{/if}
        </td>
        <td>{platformLabel(g.platform)}</td>
        <td class="muted">{g.genre ?? "–"}</td>
        <td class="num">{formatPlaytime(g.playtimeSecs)}</td>
        <td class:running={app.running.includes(g.id)}>{app.running.includes(g.id) ? "Läuft gerade" : relativeDay(g.lastPlayed)}</td>
        <td class="num">{g.playCount}</td>
        <td class="act">
          <button class="btn primary" onclick={(e) => { e.stopPropagation(); app.launch(g.id); }}>Spielen</button>
        </td>
      </tr>
    {/each}
  </tbody>
</table>

<style>
  table {
    width: 100%;
    border-collapse: collapse;
    font-size: 14px;
  }
  th {
    text-align: left;
    font-size: 12px;
    font-weight: 600;
    letter-spacing: 0.8px;
    text-transform: uppercase;
    color: var(--text-muted);
    padding: 0 12px 10px;
    border-bottom: 1px solid var(--border);
  }
  td {
    padding: 8px 12px;
    border-bottom: 1px solid var(--border);
    white-space: nowrap;
  }
  tbody tr {
    cursor: pointer;
  }
  tbody tr:hover {
    background: var(--surface);
  }
  .thumb {
    width: 52px;
    padding-right: 0;
  }
  .thumb :global(.cover) {
    width: 40px;
    box-shadow: none;
  }
  .thumb :global(.title) {
    display: none;
  }
  .c-name {
    width: 40%;
    white-space: normal;
  }
  .link {
    border: 0;
    background: none;
    padding: 0;
    font-weight: 600;
    font-size: 15px;
    text-align: left;
  }
  .tags {
    display: block;
    font-size: 12px;
    color: var(--text-muted);
    margin-top: 2px;
  }
  .fav {
    color: var(--star);
    margin-right: 6px;
  }
  .num {
    text-align: right;
  }
  .running {
    color: var(--accent);
    font-weight: 600;
  }
  .act {
    text-align: right;
  }
  .act .btn {
    padding: 7px 14px;
  }
</style>
