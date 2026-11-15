<script lang="ts">
  import { t } from "$lib/i18n.svelte";
  import { untrack } from "svelte";
  import { api } from "$lib/api";
  import { app } from "$lib/state.svelte";
  import type { Game } from "$lib/types";

  let { game }: { game: Game } = $props();
  // Editable copies; the detail page is re-created per game.
  let dir = $state(untrack(() => game.installDir ?? ""));
  let exe = $state(untrack(() => game.processExe ?? ""));
  let dirty = $derived(dir !== (game.installDir ?? "") || exe !== (game.processExe ?? ""));

  async function save(e: SubmitEvent) {
    e.preventDefault();
    try {
      app.upsert(await api.updateGame(game.id, { installDir: dir.trim() || null, processExe: exe.trim() || null }));
      app.toast(t("Gespeichert"));
    } catch (err) {
      app.error(err);
    }
  }
</script>

<form class="panel" onsubmit={save}>
  <h2>{t("Details")}</h2>
  <dl>
    <dt>{t("Start über")}</dt>
    <dd>{game.launchTarget ?? game.shortcutPath}</dd>
    <dt>{t("Verknüpfung")}</dt>
    <dd>{game.shortcutPath}</dd>
    {#if game.originalPath}
      <dt>{t("Ursprünglich")}</dt>
      <dd>{game.originalPath}</dd>
    {/if}
  </dl>
  <p class="muted">
    {t("Für die Spielzeitmessung sucht der Launcher nach dem Start einen Prozess aus dem Installationsordner oder mit dem Namen der Exe. Wird ein Spiel nicht erkannt, trag hier den Ordner oder die Exe ein.")}
  </p>
  <div class="two">
    <label><span>{t("Installationsordner")}</span><input class="input" bind:value={dir} placeholder={t("z. B. {example}", { example: "D:\\SteamLibrary\\steamapps\\common\\Valheim" })} /></label>
    <label><span>{t("Exe des Spiels")}</span><input class="input" bind:value={exe} placeholder={t("z. B. {example}", { example: "valheim.exe" })} /></label>
  </div>
  <div><button class="btn" type="submit" disabled={!dirty}>{t("Speichern")}</button></div>
</form>

<style>
  .panel {
    padding: 26px 30px;
    border-radius: 20px;
    background: var(--surface);
    border: 1px solid var(--border);
    display: flex;
    flex-direction: column;
    gap: 16px;
  }
  h2 {
    font-size: 20px;
    font-weight: 600;
  }
  dl {
    margin: 0;
    display: grid;
    grid-template-columns: max-content 1fr;
    gap: 8px 20px;
    font-size: 14px;
  }
  dt {
    color: var(--text-muted);
  }
  dd {
    margin: 0;
    font-family: ui-monospace, monospace;
    font-size: 13px;
    overflow-wrap: anywhere;
  }
  p {
    margin: 0;
    font-size: 14px;
    line-height: 1.5;
  }
  .two {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(240px, 1fr));
    gap: 14px;
  }
  label {
    display: flex;
    flex-direction: column;
    gap: 6px;
    font-size: 13px;
    color: var(--text-2);
  }
</style>
