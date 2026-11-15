<script lang="ts">
  import { onMount } from "svelte";
  import { api } from "$lib/api";
  import { app } from "$lib/state.svelte";
  import type { Settings } from "$lib/types";
  import Icon from "./Icon.svelte";

  let settings = $state<Settings | null>(null);
  let sgdb = $state("");
  let igdbId = $state("");
  let igdbSecret = $state("");
  let saving = $state(false);
  let restoring = $state(false);

  onMount(async () => {
    try {
      settings = await api.getSettings();
      sgdb = settings.keys.steamgriddb ?? "";
      igdbId = settings.keys.igdbClientId ?? "";
      igdbSecret = settings.keys.igdbClientSecret ?? "";
    } catch (e) {
      app.error(e);
    }
  });

  async function save(e: SubmitEvent) {
    e.preventDefault();
    saving = true;
    try {
      await api.saveKeys({
        steamgriddb: sgdb.trim() || null,
        igdbClientId: igdbId.trim() || null,
        igdbClientSecret: igdbSecret.trim() || null,
      });
      app.toast("API-Keys gespeichert");
    } catch (err) {
      app.error(err);
    } finally {
      saving = false;
    }
  }

  async function refetchMissing() {
    const missing = app.games.filter((g) => !g.coverPath || !g.genre);
    if (!missing.length) {
      app.toast("Alle Spiele haben schon Cover und Metadaten");
      return;
    }
    app.toast(`Lade Cover und Metadaten für ${missing.length} Spiele …`);
    for (const g of missing) {
      try {
        app.upsert(await api.refreshMetadata(g.id));
      } catch (e) {
        app.error(e);
      }
    }
    app.toast("Fertig");
  }

  async function restoreAll() {
    if (!confirm("Wirklich ALLE Verknüpfungen zurück auf den Desktop legen? Die Bibliothek wird dabei geleert.")) return;
    restoring = true;
    try {
      const r = await api.restoreAll();
      await app.load();
      app.toast(`${r.restored} Verknüpfungen zurück auf den Desktop gelegt`);
      if (r.failed.length) app.toast(`${r.failed.length} fehlgeschlagen: ${r.failed[0].error}`, "error");
    } catch (e) {
      app.error(e);
    } finally {
      restoring = false;
    }
  }
</script>

<main class="scroll">
  <h1>Einstellungen</h1>

  <form class="panel" onsubmit={save}>
    <h2>Cover und Metadaten</h2>
    <p class="muted">
      Steam-Spiele bekommen Cover und Infos ohne Key. Für alle anderen Plattformen brauchst du einen kostenlosen
      SteamGridDB-Key; IGDB liefert zusätzlich Genre, Entwickler und Erscheinungsjahr. Die Keys werden in der
      Windows-Anmeldeinformationsverwaltung gespeichert.
    </p>
    <label>
      <span>SteamGridDB API-Key</span>
      <input class="input" type="password" autocomplete="off" bind:value={sgdb} placeholder="Key von steamgriddb.com/profile/preferences/api" />
    </label>
    <div class="two">
      <label>
        <span>IGDB Client-ID (Twitch)</span>
        <input class="input" autocomplete="off" bind:value={igdbId} placeholder="aus dev.twitch.tv/console" />
      </label>
      <label>
        <span>IGDB Client-Secret</span>
        <input class="input" type="password" autocomplete="off" bind:value={igdbSecret} />
      </label>
    </div>
    <div class="row">
      <button class="btn primary" type="submit" disabled={saving}>{saving ? "Speichert …" : "Speichern"}</button>
      <button class="btn" type="button" onclick={refetchMissing}><Icon name="refresh" size={16} />Fehlende Cover und Infos laden</button>
    </div>
  </form>

  <section class="panel">
    <h2>Speicherort</h2>
    {#if settings}
      <dl>
        <dt>Datenordner</dt>
        <dd>{settings.dataDir}</dd>
        <dt>Verschobene Verknüpfungen</dt>
        <dd>{settings.shortcutsDir}</dd>
        <dt>Desktop</dt>
        <dd>{settings.desktopDirs.join(", ")}</dd>
      </dl>
    {/if}
    <div class="row">
      <button class="btn" onclick={() => api.openDataFolder().catch((e) => app.error(e))}><Icon name="folder" size={16} />Ordner öffnen</button>
    </div>
  </section>

  <section class="panel">
    <h2>Alles rückgängig machen</h2>
    <p class="muted">Legt alle importierten Verknüpfungen zurück auf den Desktop und entfernt die Spiele aus der Bibliothek.</p>
    <div class="row">
      <button class="btn danger" onclick={restoreAll} disabled={restoring}><Icon name="undo" size={16} />Alle zurück auf den Desktop</button>
    </div>
  </section>
</main>

<style>
  main {
    flex: 1;
    height: 100vh;
    padding: 40px clamp(24px, 5vw, 120px) 64px;
    display: flex;
    flex-direction: column;
    gap: 28px;
    max-width: 1100px;
  }
  h1 {
    font-size: 36px;
  }
  .panel {
    padding: 28px 32px;
    border-radius: 20px;
    background: var(--surface);
    border: 1px solid var(--border);
    display: flex;
    flex-direction: column;
    gap: 18px;
  }
  .panel h2 {
    font-size: 21px;
    font-weight: 600;
  }
  p {
    margin: 0;
    line-height: 1.55;
  }
  label {
    display: flex;
    flex-direction: column;
    gap: 8px;
    font-size: 14px;
    color: var(--text-2);
  }
  .two {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(260px, 1fr));
    gap: 18px;
  }
  .row {
    display: flex;
    gap: 12px;
    flex-wrap: wrap;
  }
  dl {
    margin: 0;
    display: grid;
    grid-template-columns: max-content 1fr;
    gap: 10px 24px;
    font-size: 14px;
  }
  dt {
    color: var(--text-muted);
  }
  dd {
    margin: 0;
    font-family: ui-monospace, monospace;
    overflow-wrap: anywhere;
  }
</style>
