<script lang="ts">
  import { t } from "$lib/i18n.svelte";
  import { api } from "$lib/api";
  import { app } from "$lib/state.svelte";
  import type { Game } from "$lib/types";
  import Icon from "./Icon.svelte";

  let { game }: { game: Game } = $props();

  async function toggle(collectionId: number) {
    try {
      app.upsert(await api.setInCollection(collectionId, game.id, !game.collections.includes(collectionId)));
      await app.reloadMeta();
    } catch (e) {
      app.error(e);
    }
  }
</script>

<div class="list">
  {#each app.collections as c (c.id)}
    {@const on = game.collections.includes(c.id)}
    <button class="chip" class:active={on} aria-pressed={on} onclick={() => toggle(c.id)}>
      {#if on}<Icon name="check" size={14} stroke={3} />{/if}{c.name}
    </button>
  {/each}
  {#if !app.collections.length}
    <span class="muted">{t("Noch keine Sammlungen. Leg links in der Seitenleiste eine an.")}</span>
  {/if}
</div>

<style>
  .list {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }
</style>
