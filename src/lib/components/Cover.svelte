<script lang="ts">
  import { convertFileSrc } from "@tauri-apps/api/core";
  import { placeholderColors } from "$lib/format";
  import { platformLabel, type Game } from "$lib/types";

  let { game, showBadge = true, radius = 14 }: { game: Game; showBadge?: boolean; radius?: number } = $props();

  let failed = $state(false);
  let src = $derived(game.coverPath && !failed ? convertFileSrc(game.coverPath) : null);
  let colors = $derived(placeholderColors(game.name));

  $effect(() => {
    // Retry when the cover changes.
    void game.coverPath;
    failed = false;
  });
</script>

<div class="cover" style="border-radius: {radius}px; --a: {colors[0]}; --b: {colors[1]}">
  {#if src}
    <img {src} alt="" loading="lazy" onerror={() => (failed = true)} />
  {:else}
    <span class="title">{game.name}</span>
  {/if}
  {#if showBadge}
    <span class="badge">{platformLabel(game.platform)}</span>
  {/if}
</div>

<style>
  .cover {
    position: relative;
    aspect-ratio: 2 / 3;
    width: 100%;
    overflow: hidden;
    background: linear-gradient(170deg, var(--a) 0%, var(--b) 100%);
    box-shadow: 0 10px 30px rgba(0, 0, 0, 0.35);
    display: flex;
    flex-direction: column;
    justify-content: flex-end;
    container-type: inline-size;
  }
  img {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  .title {
    padding: 8cqw;
    font-family: var(--font-display);
    font-weight: 700;
    font-size: max(14px, 11cqw);
    line-height: 1.05;
    color: #fff;
    text-shadow: 0 2px 12px rgba(0, 0, 0, 0.5);
    overflow-wrap: anywhere;
    text-align: left;
  }
  .badge {
    position: absolute;
    top: 7cqw;
    left: 7cqw;
    padding: 4px 9px;
    border-radius: 8px;
    background: rgba(10, 11, 14, 0.72);
    color: var(--text);
    font-size: 11px;
    font-weight: 600;
    letter-spacing: 0.3px;
  }
</style>
