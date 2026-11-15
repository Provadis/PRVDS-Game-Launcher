<script lang="ts">
  import { t } from "$lib/i18n.svelte";
  import { onMount } from "svelte";
  import { open } from "@tauri-apps/plugin-dialog";
  import { api } from "$lib/api";
  import { app } from "$lib/state.svelte";
  import type { ArtKind, ArtOption, Game } from "$lib/types";
  import Icon from "./Icon.svelte";

  let { game, kind, onclose }: { game: Game; kind: ArtKind; onclose: () => void } = $props();

  let options = $state<ArtOption[]>([]);
  let loading = $state(true);
  let applying = $state<string | null>(null);

  onMount(async () => {
    try {
      options = await api.artOptions(game.id, kind);
    } catch (e) {
      app.error(e);
    } finally {
      loading = false;
    }
  });

  async function pick(o: ArtOption) {
    applying = o.url;
    try {
      app.upsert(await api.setArtFromUrl(game.id, kind, o.url));
      onclose();
    } catch (e) {
      app.error(e);
    } finally {
      applying = null;
    }
  }

  async function pickFile() {
    const path = await open({ multiple: false, filters: [{ name: t("Bilder"), extensions: ["png", "jpg", "jpeg", "webp"] }] });
    if (typeof path !== "string") return;
    try {
      app.upsert(await api.setArtFromFile(game.id, kind, path));
      onclose();
    } catch (e) {
      app.error(e);
    }
  }
</script>

<svelte:window onkeydown={(e) => e.key === "Escape" && onclose()} />

<div class="backdrop" role="presentation" onclick={(e) => e.target === e.currentTarget && onclose()}>
  <div class="dialog" role="dialog" aria-modal="true" aria-labelledby="art-title">
    <header>
      <h2 id="art-title">{t(kind === "cover" ? "Cover für {name}" : "Hintergrundbild für {name}", { name: game.name })}</h2>
      <button class="btn" onclick={pickFile}><Icon name="folder" size={16} />{t("Eigenes Bild")}</button>
      <button class="btn icon" onclick={onclose} aria-label={t("Schließen")}><Icon name="close" /></button>
    </header>
    <div class="body scroll">
      {#if loading}
        <p class="muted">{t("Suche nach Bildern …")}</p>
      {:else if options.length === 0}
        <div class="empty">
          <p>{t("Keine Bilder gefunden.")}</p>
          <p class="muted">
            {t("Für mehr Treffer trag in den Einstellungen einen SteamGridDB-Key ein, oder wähle ein eigenes Bild. Du kannst ein Bild auch direkt auf das Cover ziehen.")}
          </p>
          <button class="btn" onclick={() => { onclose(); app.screen = { kind: "settings" }; }}>{t("Zu den Einstellungen")}</button>
        </div>
      {:else}
        <div class="grid" class:hero={kind === "hero"}>
          {#each options as o (o.url)}
            <button class="opt" onclick={() => pick(o)} disabled={!!applying} aria-label={t("Bild von {source} wählen", { source: o.source })}>
              <img src={o.thumb} alt="" loading="lazy" />
              <span class="src">{applying === o.url ? t("Wird geladen …") : o.source}</span>
            </button>
          {/each}
        </div>
      {/if}
    </div>
  </div>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 50;
    background: rgba(5, 6, 8, 0.7);
    display: grid;
    place-items: center;
    padding: 32px;
  }
  .dialog {
    width: min(1200px, 100%);
    max-height: 100%;
    display: flex;
    flex-direction: column;
    border-radius: 20px;
    background: var(--bg-raised);
    border: 1px solid var(--border-strong);
    box-shadow: 0 30px 80px rgba(0, 0, 0, 0.6);
    overflow: hidden;
  }
  header {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 20px 24px;
    border-bottom: 1px solid var(--border);
  }
  header h2 {
    flex: 1;
    font-size: 22px;
  }
  .body {
    padding: 24px;
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(160px, 1fr));
    gap: 16px;
  }
  .grid.hero {
    grid-template-columns: repeat(auto-fill, minmax(320px, 1fr));
  }
  .opt {
    position: relative;
    padding: 0;
    border: 2px solid transparent;
    border-radius: 12px;
    overflow: hidden;
    background: var(--surface);
    aspect-ratio: 2 / 3;
  }
  .hero .opt {
    aspect-ratio: 96 / 31;
  }
  .opt:hover,
  .opt:focus-visible {
    border-color: var(--accent);
  }
  .opt img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
  }
  .src {
    position: absolute;
    left: 8px;
    bottom: 8px;
    padding: 3px 8px;
    border-radius: 6px;
    background: rgba(10, 11, 14, 0.8);
    font-size: 11px;
    font-weight: 600;
  }
  .empty {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 8px;
  }
  .empty p {
    margin: 0;
    max-width: 640px;
    line-height: 1.5;
  }
</style>
