<script lang="ts">
  import { convertFileSrc } from "@tauri-apps/api/core";
  import { app } from "$lib/state.svelte";
  import { formatPlaytime, placeholderColors, relativeDay } from "$lib/format";
  import { platformLabel, type Game } from "$lib/types";

  let { game }: { game: Game } = $props();
  let colors = $derived(placeholderColors(game.name));
  let hero = $derived(game.heroPath ? convertFileSrc(game.heroPath) : null);
</script>

<section class="hero" aria-label="Weiterspielen" style="--a: {colors[0]}; --b: {colors[1]}">
  {#if hero}<img src={hero} alt="" />{/if}
  <div class="shade"></div>
  <div class="content">
    <div class="text">
      <span class="eyebrow">Weiterspielen</span>
      <h1>{game.name}</h1>
      <span class="meta">
        {relativeDay(game.lastPlayed)} · {formatPlaytime(game.playtimeSecs)} gespielt · {platformLabel(game.platform)}
      </span>
    </div>
    <div class="actions">
      <button class="btn big" onclick={() => (app.screen = { kind: "detail", id: game.id })}>Details</button>
      <button class="btn big primary" onclick={() => app.launch(game.id)}>
        <svg aria-hidden="true" width="18" height="18" viewBox="0 0 24 24" fill="currentColor"><path d="M7 4v16l13-8z" /></svg>
        Spielen
      </button>
    </div>
  </div>
</section>

<style>
  .hero {
    position: relative;
    flex-shrink: 0;
    height: clamp(220px, 26vh, 340px);
    border-radius: 20px;
    overflow: hidden;
    background: linear-gradient(115deg, var(--a) 0%, var(--b) 70%);
    display: flex;
    align-items: flex-end;
  }
  img {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    object-fit: cover;
    object-position: center 30%;
  }
  .shade {
    position: absolute;
    inset: 0;
    background: linear-gradient(90deg, rgba(10, 11, 14, 0.85) 0%, rgba(10, 11, 14, 0.35) 55%, rgba(10, 11, 14, 0.1) 100%);
  }
  .content {
    position: relative;
    width: 100%;
    padding: 40px 48px;
    display: flex;
    align-items: flex-end;
    gap: 24px;
    flex-wrap: wrap;
  }
  .text {
    display: flex;
    flex-direction: column;
    gap: 12px;
    flex: 1;
    min-width: 0;
  }
  .eyebrow {
    font-size: 13px;
    font-weight: 600;
    letter-spacing: 1.4px;
    text-transform: uppercase;
    color: #f2d7b8;
  }
  h1 {
    font-size: clamp(36px, 3.6vw, 60px);
    line-height: 1;
  }
  .meta {
    color: #ddd8d2;
  }
  .actions {
    display: flex;
    gap: 12px;
  }
  .actions .btn:not(.primary) {
    background: rgba(14, 15, 18, 0.55);
    border-color: rgba(255, 255, 255, 0.15);
  }
</style>
