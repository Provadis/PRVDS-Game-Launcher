<script lang="ts">
  import { onMount } from "svelte";
  import { api } from "$lib/api";
  import { app } from "$lib/state.svelte";
  import type { Settings, UpdateChannel, UpdateCheck } from "$lib/types";
  import { autoCheckUpdates, setAutoCheckUpdates, setUpdateChannel, updateChannel } from "$lib/updates";
  import { setSyncLaunchersOnStart, setTidyDesktopShortcuts, syncLaunchersOnStart, tidyDesktop, tidyDesktopShortcuts } from "$lib/launchers";
  import Icon from "./Icon.svelte";

  let settings = $state<Settings | null>(null);
  let sgdb = $state("");
  let igdbId = $state("");
  let igdbSecret = $state("");
  let saving = $state(false);
  let restoring = $state(false);
  let channel = $state<UpdateChannel>(updateChannel());
  let autoCheck = $state(autoCheckUpdates());
  let checking = $state(false);
  let installing = $state(false);
  let check = $state<UpdateCheck | null>(null);

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

  let reloading = $state(false);

  async function reloadAllArt() {
    if (
      !confirm(
        "Cover und Titelbilder aller Spiele neu laden? Offizielle Store-Bilder haben Vorrang. Selbst gewählte Bilder werden dabei ersetzt.",
      )
    )
      return;
    reloading = true;
    app.toast(`Lade Bilder für ${app.games.length} Spiele neu …`);
    let failed = 0;
    for (const g of app.games) {
      try {
        app.upsert(await api.reloadArt(g.id));
      } catch {
        failed++;
      }
    }
    reloading = false;
    app.toast(failed ? `Fertig, ${failed} Spiele ohne neue Bilder` : "Fertig");
  }

  async function checkNow() {
    checking = true;
    try {
      check = await api.checkUpdate(channel);
    } catch (e) {
      check = null;
      app.error(`Update-Prüfung fehlgeschlagen: ${e}`);
    } finally {
      checking = false;
    }
  }

  async function install() {
    installing = true;
    try {
      // On Windows the installer closes the app and starts the new version.
      await api.installUpdate();
    } catch (e) {
      app.error(`Update fehlgeschlagen: ${e}`);
      installing = false;
    }
  }

  function changeChannel(c: UpdateChannel) {
    channel = c;
    setUpdateChannel(c);
    check = null;
  }

  async function restoreAll() {
    if (!confirm("Wirklich ALLE Verknüpfungen zurück auf den Desktop legen? Desktop-Spiele werden dabei aus der Bibliothek entfernt.")) return;
    restoring = true;
    // Otherwise the next start would tidy the launcher shortcuts away again.
    tidy = false;
    setTidyDesktopShortcuts(false);
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
  let syncLaunchers = $state(syncLaunchersOnStart());
  let tidy = $state(tidyDesktopShortcuts());
  let tidying = $state(false);

  async function changeTidy() {
    setTidyDesktopShortcuts(tidy);
    if (!tidy) return;
    tidying = true;
    const n = await tidyDesktop(true);
    tidying = false;
    app.toast(n ? `${n} Desktop-Verknüpfungen weggeräumt. Positionen sind gemerkt.` : "Keine passenden Desktop-Verknüpfungen gefunden.");
  }
</script>

<main class="scroll">
  <h1>Einstellungen</h1>

  <form class="panel" onsubmit={save}>
    <h2>Cover und Metadaten</h2>
    <p class="muted">
      Ohne Key gibt es die offiziellen Store-Bilder: von Steam, von Epic (aus dem installierten Epic Launcher) und
      für Spiele anderer Launcher, die es auch auf Steam gibt. Für den Rest liefert IGDB die offizielle Box-Art samt
      Genre, Entwickler und Erscheinungsjahr, SteamGridDB springt ein, wo sonst nichts gefunden wird. Die Keys werden
      in der Windows-Anmeldeinformationsverwaltung gespeichert.
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
      <button class="btn" type="button" onclick={reloadAllArt} disabled={reloading}><Icon name="image" size={16} />{reloading ? "Lädt …" : "Alle Cover neu laden"}</button>
    </div>
  </form>

  <section class="panel">
    <h2>Updates</h2>
    <p class="muted">
      „Stabil“ bekommt nur fertige Versionen. „Dev“ bekommt zusätzlich jeden Entwicklungsstand, kann aber Fehler haben.
      Updates sind signiert, die App installiert nur Pakete von uns.
    </p>
    <div class="channels" role="radiogroup" aria-label="Update-Kanal">
      <button class="chip" class:active={channel === "stable"} role="radio" aria-checked={channel === "stable"} onclick={() => changeChannel("stable")}>Stabil</button>
      <button class="chip" class:active={channel === "dev"} role="radio" aria-checked={channel === "dev"} onclick={() => changeChannel("dev")}>Dev</button>
    </div>
    <label class="check">
      <input type="checkbox" bind:checked={autoCheck} onchange={() => setAutoCheckUpdates(autoCheck)} />
      <span>Beim Start nach Updates suchen</span>
    </label>
    <div class="row">
      <button class="btn" onclick={checkNow} disabled={checking || installing}><Icon name="refresh" size={16} />{checking ? "Sucht …" : "Jetzt nach Updates suchen"}</button>
      {#if check?.update}
        <button class="btn primary" onclick={install} disabled={installing}>{installing ? "Wird installiert …" : `Version ${check.update.version} installieren`}</button>
      {/if}
    </div>
    {#if check && !check.enabled}
      <p class="muted">Diese Version wurde ohne Update-Schlüssel gebaut (z. B. lokal). Updates sind hier ausgeschaltet.</p>
    {:else if check && !check.update}
      <p class="muted">Du bist auf dem neuesten Stand{settings ? ` (${settings.version})` : ""}.</p>
    {:else if check?.update?.notes}
      <p class="notes">{check.update.notes}</p>
    {/if}
  </section>

  <section class="panel">
    <h2>Launcher</h2>
    <p class="muted">
      Liest installierte Spiele aus Steam, Epic, Ubisoft Connect, EA App, Battle.net und Riot. Deinstallierte Spiele
      bleiben mit Spielzeit in der Bibliothek und werden als „Nicht installiert“ markiert.
    </p>
    <label class="check">
      <input type="checkbox" bind:checked={syncLaunchers} onchange={() => setSyncLaunchersOnStart(syncLaunchers)} />
      <span>Beim Start nach neuen und deinstallierten Spielen suchen</span>
    </label>
    <label class="check">
      <input type="checkbox" bind:checked={tidy} onchange={changeTidy} disabled={tidying} />
      <span>Desktop-Verknüpfungen importierter Spiele entfernen</span>
    </label>
    <p class="muted small">
      Verknüpfungen von Spielen aus den Launchern wandern wie beim Desktop-Import in den App-Ordner, inklusive Position.
      „Alle zurück auf den Desktop“ legt sie wieder an ihren alten Platz.
    </p>
    <div class="row">
      <button class="btn" onclick={() => (app.screen = { kind: "import", from: "launchers" })}><Icon name="layers" size={16} />Spiele aus Launchern einlesen</button>
    </div>
  </section>

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
    <p class="muted">Legt alle importierten Verknüpfungen zurück auf den Desktop und entfernt die Spiele aus der Bibliothek. Spiele aus den Launchern bleiben in der Bibliothek, nur ihre Verknüpfungen kommen zurück.</p>
    <div class="row">
      <button class="btn danger" onclick={restoreAll} disabled={restoring}><Icon name="undo" size={16} />Alle zurück auf den Desktop</button>
    </div>
  </section>

  {#if settings}
    <p class="about muted">PRVDS Game Launcher · Version {settings.version}</p>
  {/if}
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
  .channels {
    display: flex;
    gap: 8px;
  }
  .small {
    font-size: 13px;
    margin: -4px 0 0 30px;
  }
  .check {
    flex-direction: row;
    align-items: center;
    gap: 10px;
    color: var(--text);
  }
  .notes {
    white-space: pre-line;
    font-size: 14px;
    color: var(--text-2);
  }
  .about {
    font-size: 13px;
    padding: 0 4px;
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
