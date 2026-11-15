<script lang="ts">
  import { t } from "$lib/i18n.svelte";
  import { open } from "@tauri-apps/plugin-dialog";
  import { api } from "$lib/api";
  import { app } from "$lib/state.svelte";
  import type { BackgroundPreset, BoardBackground } from "$lib/types";
  import BoardBackdrop from "./BoardBackdrop.svelte";
  import Icon from "./Icon.svelte";

  let {
    value,
    onchange,
    onclose,
  }: { value: BoardBackground | null; onchange: (bg: BoardBackground | null) => void; onclose: () => void } = $props();

  const presets: { id: BackgroundPreset; label: string }[] = [
    { id: "aurora", label: "Aurora (animiert)" },
    { id: "nebula", label: "Nebel" },
    { id: "sunset", label: "Abendrot" },
    { id: "drift", label: "Lichter (animiert)" },
    { id: "synthwave", label: "Synthwave (animiert)" },
    { id: "peaks", label: "Berge bei Nacht" },
    { id: "ocean", label: "Ozean (animiert)" },
  ];
  let busy = $state(false);

  function set(patch: Partial<BoardBackground>) {
    const base: BoardBackground = value ?? { kind: "preset", value: "aurora", align: "center", dim: 35 };
    onchange({ ...base, ...patch });
  }

  async function pickFile() {
    const path = await open({
      multiple: false,
      filters: [{ name: t("Bilder und Videos"), extensions: ["jpg", "jpeg", "png", "webp", "gif", "mp4", "webm"] }],
    });
    if (typeof path !== "string") return;
    busy = true;
    try {
      const copy = await api.importBackground(path);
      const kind = /\.(mp4|webm)$/i.test(copy) ? "video" : "image";
      set({ kind, value: copy, align: value?.kind !== "preset" && value?.align ? value.align : "right" });
    } catch (e) {
      app.error(e);
    } finally {
      busy = false;
    }
  }
</script>

<div class="picker" role="dialog" aria-label={t("Hintergrund")}>
  <header>
    <h3>{t("Hintergrund")}</h3>
    <button class="icon-btn" onclick={onclose} aria-label={t("Schließen")}><Icon name="close" size={16} /></button>
  </header>
  <div class="tiles">
    <button class="tile none" class:on={!value} onclick={() => onchange(null)}>
      <span class="thumb"></span><span>{t("Keiner")}</span>
    </button>
    {#each presets as p (p.id)}
      <button class="tile" class:on={value?.kind === "preset" && value.value === p.id} onclick={() => set({ kind: "preset", value: p.id })}>
        <span class="thumb"><BoardBackdrop bg={{ kind: "preset", value: p.id, align: "center", dim: 0 }} /></span>
        <span>{t(p.label)}</span>
      </button>
    {/each}
  </div>
  <button class="btn own" onclick={pickFile} disabled={busy}>
    <Icon name="image" size={16} />{busy ? t("Wird kopiert …") : t("Eigenes Bild oder Video …")}
  </button>
  {#if value && value.kind !== "preset"}
    <p class="muted small">{value.kind === "video" ? t("Video gewählt. Welcher Teil soll sichtbar bleiben?") : t("Bild gewählt. Welcher Teil soll sichtbar bleiben?")}</p>
    <div class="seg" role="radiogroup" aria-label={t("Ausrichtung")}>
      {#each [["left", "Links"], ["center", "Mitte"], ["right", "Rechts"]] as [a, label] (a)}
        <button class="chip" class:active={value.align === a} role="radio" aria-checked={value.align === a} onclick={() => set({ align: a as BoardBackground["align"] })}>{t(label)}</button>
      {/each}
    </div>
  {/if}
  {#if value}
    <label class="dim">
      <span>{t("Abdunkeln")}</span>
      <input type="range" min="0" max="85" step="5" value={value.dim} oninput={(e) => set({ dim: Number(e.currentTarget.value) })} />
      <span class="muted">{value.dim} %</span>
    </label>
  {/if}
  <p class="muted small">
    {t("Die eingebauten Hintergründe sind für diese App gemacht. Eigene Dateien werden in den App-Ordner kopiert; freie Wallpaper gibt es z. B. bei Unsplash oder Pexels (dort auch Videos).")}
  </p>
</div>

<style>
  .picker {
    position: absolute;
    top: calc(100% + 8px);
    left: 0;
    z-index: 20;
    width: 420px;
    max-width: calc(100vw - 32px);
    padding: 16px;
    border-radius: 16px;
    background: var(--surface-2);
    border: 1px solid var(--border);
    box-shadow: 0 18px 48px rgb(0 0 0 / 0.5);
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }
  h3 {
    font-size: 16px;
  }
  .tiles {
    display: grid;
    grid-template-columns: repeat(4, 1fr);
    gap: 8px;
  }
  .tile {
    display: flex;
    flex-direction: column;
    gap: 4px;
    padding: 0;
    border: 0;
    background: none;
    color: var(--text-muted);
    font-size: 11px;
    text-align: left;
    cursor: pointer;
  }
  .thumb {
    position: relative;
    display: block;
    height: 54px;
    border-radius: 8px;
    overflow: hidden;
    outline: 2px solid transparent;
    background: var(--surface-3);
  }
  .tile.on .thumb {
    outline-color: var(--accent);
  }
  .tile.on {
    color: var(--text);
  }
  .own {
    justify-content: center;
  }
  .seg {
    display: flex;
    gap: 6px;
  }
  .dim {
    display: flex;
    align-items: center;
    gap: 10px;
    font-size: 13px;
  }
  .dim input {
    flex: 1;
  }
  .small {
    font-size: 12px;
  }
</style>
