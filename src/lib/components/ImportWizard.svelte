<script lang="ts">
  import { onMount } from "svelte";
  import { api } from "$lib/api";
  import { app } from "$lib/state.svelte";
  import { placeholderColors } from "$lib/format";
  import { platformLabel, type Candidate } from "$lib/types";
  import Icon from "./Icon.svelte";

  let candidates = $state<Candidate[]>([]);
  let selected = $state<Record<string, boolean>>({});
  let scanning = $state(true);
  let importing = $state(false);
  let desktop = $state("");

  let games = $derived(candidates.filter((c) => c.likelyGame));
  let others = $derived(candidates.filter((c) => !c.likelyGame));
  let count = $derived(Object.values(selected).filter(Boolean).length);

  async function scan() {
    scanning = true;
    try {
      const [found, settings] = await Promise.all([api.scanDesktop(), api.getSettings()]);
      desktop = settings.desktopDirs.join(", ");
      candidates = found;
      selected = Object.fromEntries(found.map((c) => [c.path, c.likelyGame]));
    } catch (e) {
      app.error(e);
    } finally {
      scanning = false;
    }
  }

  function setAll(list: Candidate[], value: boolean) {
    for (const c of list) selected[c.path] = value;
  }

  async function doImport() {
    const paths = candidates.filter((c) => selected[c.path]).map((c) => c.path);
    if (!paths.length) return;
    importing = true;
    try {
      const report = await api.importShortcuts(paths);
      for (const g of report.imported) app.upsert(g);
      if (report.failed.length) {
        app.toast(
          `${report.failed.length} Verknüpfung(en) konnten nicht verschoben werden: ${report.failed[0].error}`,
          "error",
        );
      }
      if (report.imported.length) {
        app.toast(
          `${report.imported.length} Spiele in deinen Game Space verschoben. Cover werden geladen …`,
        );
      }
      if (report.keptOnDesktop.length) {
        const n = report.keptOnDesktop.length;
        app.toast(
          (n === 1
            ? `„${report.keptOnDesktop[0]}“ liegt auf dem öffentlichen Desktop. Die Verknüpfung wurde kopiert und bleibt dort`
            : `${n} Spiele liegen auf dem öffentlichen Desktop. Die Verknüpfungen wurden kopiert und bleiben dort`) +
            ", denn zum Entfernen braucht Windows Admin-Rechte.",
        );
      }
      app.selectSource({ kind: "all" });
    } catch (e) {
      app.error(e);
    } finally {
      importing = false;
    }
  }

  onMount(scan);
</script>

<div class="wizard">
  <header>
    <button class="btn icon" onclick={() => (app.screen = { kind: "library" })} aria-label="Schließen">
      <Icon name="close" size={20} />
    </button>
    <div class="head">
      <h1>Spiele vom Desktop importieren</h1>
      <span class="muted">
        {#if scanning}Desktop wird eingelesen …{:else}{candidates.length} Verknüpfungen gefunden in {desktop}{/if}
      </span>
    </div>
    <ol class="steps">
      <li class="done"><span class="dot"><Icon name="check" size={14} stroke={3} /></span>Desktop scannen</li>
      <li class="line" aria-hidden="true"></li>
      <li class="current" aria-current="step"><span class="dot">2</span>Spiele auswählen</li>
      <li class="line" aria-hidden="true"></li>
      <li><span class="dot">3</span>In Game Space verschieben</li>
    </ol>
  </header>

  <div class="body scroll">
    {#if !scanning && candidates.length === 0}
      <div class="empty">
        <h2>Keine Verknüpfungen gefunden</h2>
        <p class="muted">Auf deinem Desktop liegen keine .lnk-, .url- oder .exe-Dateien.</p>
        <button class="btn" onclick={scan}><Icon name="refresh" />Erneut scannen</button>
      </div>
    {/if}

    {#if games.length}
      <section>
        <div class="section-head">
          <h2>Erkannte Spiele</h2>
          <span class="muted">nach Plattform und Speicherort erkannt</span>
          <span class="spacer"></span>
          <button class="btn" onclick={() => setAll(games, true)}>Alle auswählen</button>
          <button class="btn" onclick={() => setAll(candidates, false)}>Auswahl leeren</button>
        </div>
        <div class="items">
          {#each games as c (c.path)}
            {@render item(c, true)}
          {/each}
        </div>
      </section>
    {/if}

    {#if others.length}
      <section>
        <div class="section-head">
          <h2>Wahrscheinlich keine Spiele</h2>
          <span class="muted">bleiben auf dem Desktop, außer du wählst sie aus</span>
        </div>
        <div class="items">
          {#each others as c (c.path)}
            {@render item(c, false)}
          {/each}
        </div>
      </section>
    {/if}
  </div>

  <footer>
    <div class="summary">
      <strong>{count} von {candidates.length} ausgewählt</strong>
      <span class="muted"><Icon name="undo" size={15} />Die Verknüpfungen werden in den Bibliotheksordner verschoben. Rückgängig jederzeit möglich.</span>
    </div>
    <span class="spacer"></span>
    <button class="btn big" onclick={() => (app.screen = { kind: "library" })}>Abbrechen</button>
    <button class="btn big primary" disabled={!count || importing} onclick={doImport}>
      {importing ? "Wird verschoben …" : `${count} verschieben`}
      <Icon name="arrowRight" />
    </button>
  </footer>
</div>

{#snippet item(c: Candidate, game: boolean)}
  {@const colors = placeholderColors(c.title)}
  <label class="item" class:on={selected[c.path]}>
    {#if game}
      <span class="thumb" style="--a: {colors[0]}; --b: {colors[1]}"></span>
    {:else}
      <span class="thumb neutral"><Icon name="layers" size={20} /></span>
    {/if}
    <span class="text">
      <span class="title" title={c.title}>{c.title}</span>
      <span class="file" title={c.info.target ?? c.path}>{c.fileName}</span>
      {#if c.platform !== "other"}<span class="plat">{platformLabel(c.platform)}</span>{/if}
    </span>
    <input type="checkbox" bind:checked={selected[c.path]} />
  </label>
{/snippet}

<style>
  .wizard {
    display: flex;
    flex-direction: column;
    height: 100vh;
  }
  header {
    display: flex;
    align-items: center;
    gap: 32px;
    padding: 32px clamp(24px, 6vw, 120px) 24px;
    border-bottom: 1px solid var(--border);
    flex-wrap: wrap;
  }
  .head {
    display: flex;
    flex-direction: column;
    gap: 4px;
    min-width: 0;
  }
  h1 {
    font-size: 30px;
  }
  .steps {
    margin: 0 0 0 auto;
    padding: 0;
    list-style: none;
    display: flex;
    align-items: center;
    gap: 12px;
    font-size: 14px;
    color: var(--text-muted);
  }
  .steps li {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .steps .current {
    color: var(--text);
    font-weight: 600;
  }
  .dot {
    width: 26px;
    height: 26px;
    border-radius: 999px;
    display: grid;
    place-items: center;
    border: 1px solid var(--border-strong);
    font-size: 13px;
  }
  .done .dot {
    background: var(--surface-2);
    border-color: transparent;
    color: var(--accent);
  }
  .current .dot {
    background: var(--accent);
    border-color: transparent;
    color: var(--on-accent);
    font-weight: 700;
  }
  .line {
    width: 32px;
    height: 1px;
    background: var(--border-strong);
  }
  .body {
    flex: 1;
    padding: 28px clamp(24px, 6vw, 120px);
    display: flex;
    flex-direction: column;
    gap: 36px;
  }
  section {
    display: flex;
    flex-direction: column;
    gap: 16px;
  }
  .section-head {
    display: flex;
    align-items: center;
    gap: 14px;
    flex-wrap: wrap;
  }
  .section-head h2 {
    font-size: 20px;
  }
  .spacer {
    flex: 1;
  }
  .items {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(300px, 1fr));
    gap: 12px;
  }
  .item {
    display: flex;
    align-items: center;
    gap: 14px;
    padding: 12px 16px 12px 12px;
    border-radius: 14px;
    border: 1px solid var(--border);
    background: #131419;
    cursor: pointer;
    min-width: 0;
  }
  .item.on {
    border-color: var(--accent);
    background: #191c23;
  }
  .thumb {
    width: 44px;
    height: 66px;
    border-radius: 8px;
    flex-shrink: 0;
    background: linear-gradient(170deg, var(--a), var(--b));
  }
  .thumb.neutral {
    height: 44px;
    background: var(--surface-3);
    display: grid;
    place-items: center;
    color: var(--text-muted);
  }
  .text {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 3px;
  }
  .title {
    font-weight: 600;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .file {
    font-size: 12px;
    color: var(--text-muted);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .plat {
    align-self: flex-start;
    margin-top: 3px;
    padding: 2px 8px;
    border-radius: 6px;
    background: var(--border);
    font-size: 11px;
    font-weight: 600;
    color: var(--text-2);
  }
  input[type="checkbox"] {
    width: 20px;
    height: 20px;
    accent-color: var(--accent);
    flex-shrink: 0;
  }
  .empty {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 12px;
  }
  footer {
    display: flex;
    align-items: center;
    gap: 16px;
    padding: 20px clamp(24px, 6vw, 120px);
    background: var(--bg-raised);
    border-top: 1px solid var(--border);
    flex-wrap: wrap;
  }
  .summary {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .summary strong {
    font-family: var(--font-display);
    font-size: 19px;
  }
  .summary .muted {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 14px;
  }
</style>
