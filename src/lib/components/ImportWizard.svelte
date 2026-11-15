<script lang="ts">
  import { t } from "$lib/i18n.svelte";
  import { onMount } from "svelte";
  import { api } from "$lib/api";
  import { app } from "$lib/state.svelte";
  import { placeholderColors } from "$lib/format";
  import { platformLabel, type Candidate } from "$lib/types";
  import Icon from "./Icon.svelte";
  import ImportTabs from "./ImportTabs.svelte";
  import "./wizard.css";

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
          t("{n} Verknüpfung(en) konnten nicht verschoben werden: {error}", { n: report.failed.length, error: report.failed[0].error }),
          "error",
        );
      }
      if (report.imported.length) {
        app.toast(
          t("{n} Spiele in deinen Game Space verschoben. Cover werden geladen …", { n: report.imported.length }),
        );
      }
      if (report.keptOnDesktop.length) {
        const n = report.keptOnDesktop.length;
        app.toast(
          n === 1
            ? t("„{name}“ liegt auf dem öffentlichen Desktop. Die Verknüpfung wurde kopiert und bleibt dort, denn zum Entfernen braucht Windows Admin-Rechte.", { name: report.keptOnDesktop[0] })
            : t("{n} Spiele liegen auf dem öffentlichen Desktop. Die Verknüpfungen wurden kopiert und bleiben dort, denn zum Entfernen braucht Windows Admin-Rechte.", { n }),
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
    <button class="btn icon" onclick={() => (app.screen = { kind: "library" })} aria-label={t("Schließen")}>
      <Icon name="close" size={20} />
    </button>
    <div class="head">
      <h1>{t("Spiele vom Desktop importieren")}</h1>
      <span class="muted">
        {#if scanning}{t("Desktop wird eingelesen …")}{:else}{t("{n} Verknüpfungen gefunden in {dir}", { n: candidates.length, dir: desktop })}{/if}
      </span>
      <ImportTabs current="desktop" />
    </div>
    <ol class="steps">
      <li class="done"><span class="dot"><Icon name="check" size={14} stroke={3} /></span>{t("Desktop scannen")}</li>
      <li class="line" aria-hidden="true"></li>
      <li class="current" aria-current="step"><span class="dot">2</span>{t("Spiele auswählen")}</li>
      <li class="line" aria-hidden="true"></li>
      <li><span class="dot">3</span>{t("In Game Space verschieben")}</li>
    </ol>
  </header>

  <div class="body scroll">
    {#if !scanning && candidates.length === 0}
      <div class="empty">
        <h2>{t("Keine Verknüpfungen gefunden")}</h2>
        <p class="muted">{t("Auf deinem Desktop liegen keine .lnk-, .url- oder .exe-Dateien.")}</p>
        <button class="btn" onclick={scan}><Icon name="refresh" />{t("Erneut scannen")}</button>
      </div>
    {/if}

    {#if games.length}
      <section>
        <div class="section-head">
          <h2>{t("Erkannte Spiele")}</h2>
          <span class="muted">{t("nach Plattform und Speicherort erkannt")}</span>
          <span class="spacer"></span>
          <button class="btn" onclick={() => setAll(games, true)}>{t("Alle auswählen")}</button>
          <button class="btn" onclick={() => setAll(candidates, false)}>{t("Auswahl leeren")}</button>
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
          <h2>{t("Wahrscheinlich keine Spiele")}</h2>
          <span class="muted">{t("bleiben auf dem Desktop, außer du wählst sie aus")}</span>
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
      <strong>{t("{n} von {total} ausgewählt", { n: count, total: candidates.length })}</strong>
      <span class="muted"><Icon name="undo" size={15} />{t("Die Verknüpfungen werden in den Bibliotheksordner verschoben. Rückgängig jederzeit möglich.")}</span>
    </div>
    <span class="spacer"></span>
    <button class="btn big" onclick={() => (app.screen = { kind: "library" })}>{t("Abbrechen")}</button>
    <button class="btn big primary" disabled={!count || importing} onclick={doImport}>
      {importing ? t("Wird verschoben …") : t("{n} verschieben", { n: count })}
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
