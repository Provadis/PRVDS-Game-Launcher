<script lang="ts">
  import { t } from "$lib/i18n.svelte";
  import { api } from "$lib/api";
  import { app } from "$lib/state.svelte";
  import type { Game } from "$lib/types";
  import Icon from "./Icon.svelte";

  let { game }: { game: Game } = $props();
  let draft = $state("");
  const listId = "tag-suggestions";

  async function add(e: SubmitEvent) {
    e.preventDefault();
    const tag = draft.trim();
    if (!tag) return;
    try {
      app.upsert(await api.addTag(game.id, tag));
      draft = "";
      await app.reloadMeta();
    } catch (err) {
      app.error(err);
    }
  }

  async function remove(tag: string) {
    try {
      app.upsert(await api.removeTag(game.id, tag));
      await app.reloadMeta();
    } catch (err) {
      app.error(err);
    }
  }
</script>

<div class="tags">
  {#each game.tags as tag (tag)}
    <span class="tag">{tag}<button onclick={() => remove(tag)} aria-label={t("Tag {tag} entfernen", { tag })}><Icon name="close" size={12} stroke={2.5} /></button></span>
  {/each}
  <form onsubmit={add}>
    <label class="sr-only" for="new-tag">{t("Neuer Tag")}</label>
    <input id="new-tag" list={listId} placeholder="+ Tag" bind:value={draft} />
    <datalist id={listId}>
      {#each app.tags.filter((x) => !game.tags.includes(x)) as tag (tag)}<option value={tag}></option>{/each}
    </datalist>
  </form>
</div>

<style>
  .tags {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    align-items: center;
  }
  .tag {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 5px 6px 5px 12px;
    border-radius: 999px;
    background: var(--surface-2);
    font-size: 13px;
  }
  .tag button {
    display: flex;
    padding: 3px;
    border: 0;
    border-radius: 999px;
    background: transparent;
    color: var(--text-muted);
  }
  .tag button:hover {
    color: var(--text);
    background: var(--surface-3);
  }
  input {
    width: 110px;
    padding: 5px 12px;
    border-radius: 999px;
    border: 1px dashed #3a3f48;
    background: transparent;
    font-size: 13px;
    outline: none;
  }
  input:focus {
    border-style: solid;
    border-color: var(--accent);
  }
</style>
