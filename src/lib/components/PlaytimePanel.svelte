<script lang="ts">
  import { api } from "$lib/api";
  import { app } from "$lib/state.svelte";
  import { dailyPlaytime, formatDuration, formatPlaytime, formatSessionRange } from "$lib/format";
  import type { Game, Session } from "$lib/types";

  let { game }: { game: Game } = $props();
  let sessions = $state<Session[]>([]);

  $effect(() => {
    // Reload when the game's playtime changes (a session ended).
    void game.playtimeSecs;
    api.sessions(game.id).then((s) => (sessions = s)).catch((e) => app.error(e));
  });

  let days = $derived(dailyPlaytime(sessions, 30));
  let total30 = $derived(days.reduce((a, d) => a + d.secs, 0));
  let max = $derived(Math.max(3600, ...days.map((d) => d.secs)));
  let avg = $derived(sessions.length ? sessions.reduce((a, s) => a + s.durationSecs, 0) / sessions.length : 0);
  const fmtDay = (d: Date) => d.toLocaleDateString("de-DE", { day: "numeric", month: "short" });
  let labels = $derived([0, 7, 14, 21, 29].map((i) => fmtDay(days[i].day)));
</script>

<section class="panel" aria-label="Spielzeit">
  <div class="head">
    <h2>Spielzeit</h2>
    <span class="muted">letzte 30 Tage · {formatPlaytime(total30)}</span>
    <span class="spacer"></span>
    <span class="muted">{sessions.length} Sitzungen · Ø {sessions.length ? formatDuration(avg) : "–"}</span>
  </div>
  <div class="chart" role="img" aria-label="Spielzeit pro Tag in den letzten 30 Tagen, insgesamt {formatPlaytime(total30)}">
    {#each days as d (d.day.getTime())}
      <div class="col" title="{fmtDay(d.day)}: {d.secs ? formatDuration(d.secs) : 'nicht gespielt'}">
        <div class="bar" class:empty={!d.secs} style="height: {d.secs ? Math.max(4, (d.secs / max) * 100) : 2}%"></div>
      </div>
    {/each}
  </div>
  <div class="axis muted">
    {#each labels as l, i (i)}<span>{l}</span>{/each}
  </div>
  {#if sessions.length}
    <ul class="sessions">
      {#each sessions.slice(0, 5) as s (s.id)}
        <li><span>{formatSessionRange(s.startedAt, s.endedAt)}</span><span class="muted">{formatDuration(s.durationSecs)}</span></li>
      {/each}
    </ul>
  {:else}
    <p class="muted">
      Noch keine gemessenen Sitzungen. Starte das Spiel über den Launcher, dann wird die Spielzeit automatisch erfasst.
    </p>
  {/if}
</section>

<style>
  .panel {
    padding: 26px 30px;
    border-radius: 20px;
    background: var(--surface);
    border: 1px solid var(--border);
    display: flex;
    flex-direction: column;
    gap: 18px;
  }
  .head {
    display: flex;
    align-items: baseline;
    gap: 14px;
    flex-wrap: wrap;
  }
  h2 {
    font-size: 20px;
    font-weight: 600;
  }
  .spacer {
    flex: 1;
  }
  .chart {
    height: 180px;
    display: flex;
    align-items: flex-end;
    gap: 6px;
    border-bottom: 1px solid var(--border-strong);
    padding-bottom: 2px;
  }
  .col {
    flex: 1;
    height: 100%;
    display: flex;
    align-items: flex-end;
  }
  .bar {
    width: 100%;
    border-radius: 5px 5px 2px 2px;
    background: var(--accent);
  }
  .bar.empty {
    background: var(--surface-3);
  }
  .axis {
    display: flex;
    justify-content: space-between;
    font-size: 12px;
  }
  .sessions {
    list-style: none;
    margin: 0;
    padding: 0;
  }
  .sessions li {
    display: flex;
    justify-content: space-between;
    padding: 11px 0;
    border-top: 1px solid var(--border);
    font-size: 14px;
  }
  p {
    margin: 0;
  }
</style>
