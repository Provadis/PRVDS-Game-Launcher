<script lang="ts">
  import { api } from "$lib/api";
  import { app } from "$lib/state.svelte";
  import { formatPlaytime, relativeDay } from "$lib/format";
  import { platformLabel } from "$lib/types";
  import Cover from "./Cover.svelte";
  import Icon from "./Icon.svelte";

  let { id }: { id: number } = $props();
  let game = $derived(app.byId.get(id));

  async function undo() {
    if (!game) return;
    if (!confirm(`„${game.name}“ zurück auf den Desktop legen und aus der Bibliothek entfernen?`)) return;
    try {
      const path = await api.undoImport(game.id);
      app.remove(game.id);
      app.screen = { kind: "library" };
      app.toast(`Verknüpfung wiederhergestellt: ${path}`);
    } catch (e) {
      app.error(e);
    }
  }
</script>

{#if game}
  <div class="detail scroll">
    <button class="btn back" onclick={() => (app.screen = { kind: "library" })}>
      <Icon name="chevronLeft" />Bibliothek
    </button>
    <div class="top">
      <div class="cover"><Cover {game} radius={18} /></div>
      <div class="info">
        <span class="muted">{platformLabel(game.platform)}</span>
        <h1>{game.name}</h1>
        <div class="actions">
          <button class="btn big primary" onclick={() => app.launch(game.id)}>
            <svg aria-hidden="true" width="18" height="18" viewBox="0 0 24 24" fill="currentColor"><path d="M7 4v16l13-8z" /></svg>
            Spielen
          </button>
          <button class="btn big" onclick={undo}><Icon name="undo" />Zurück auf den Desktop</button>
        </div>
        <p class="muted">
          Spielzeit {formatPlaytime(game.playtimeSecs)} · Zuletzt gespielt {relativeDay(game.lastPlayed)} · {game.playCount} Starts
        </p>
      </div>
    </div>
  </div>
{/if}

<style>
  .detail {
    flex: 1;
    height: 100vh;
    padding: 32px clamp(24px, 5vw, 120px);
    display: flex;
    flex-direction: column;
    gap: 32px;
  }
  .back {
    align-self: flex-start;
  }
  .top {
    display: flex;
    gap: 48px;
    align-items: flex-end;
    flex-wrap: wrap;
  }
  .cover {
    width: 260px;
  }
  .info {
    display: flex;
    flex-direction: column;
    gap: 16px;
  }
  h1 {
    font-size: 56px;
    line-height: 1;
  }
  .actions {
    display: flex;
    gap: 12px;
  }
</style>
