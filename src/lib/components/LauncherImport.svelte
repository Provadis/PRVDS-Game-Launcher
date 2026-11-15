<script lang="ts">
  import { t } from "$lib/i18n.svelte";
  import { onMount } from "svelte";
  import { api } from "$lib/api";
  import { app } from "$lib/state.svelte";
  import { placeholderColors } from "$lib/format";
  import { CONNECTOR_PLATFORMS, platformLabel, type LauncherCandidate, type Platform } from "$lib/types";
  import { tidyDesktop, tidyDesktopShortcuts } from "$lib/launchers";
  import Icon from "./Icon.svelte";
  import ImportTabs from "./ImportTabs.svelte";
  import "./wizard.css";

  let candidates = $state<LauncherCandidate[]>([]);
  let selected = $state<Record<string, boolean>>({});
  let scanning = $state(true);
  let importing = $state(false);

  const keyOf = (c: LauncherCandidate) => `${c.game.platform}:${c.game.platformId}`;

  let groups = $derived(
    CONNECTOR_PLATFORMS.map((p) => ({ platform: p, items: candidates.filter((c) => c.game.platform === p) })).filter(
      (g) => g.items.length,
    ),
  );
  let fresh = $derived(candidates.filter((c) => c.existingId === null).length);
  let count = $derived(candidates.filter((c) => selected[keyOf(c)]).length);

  async function scan() {
    scanning = true;
    try {
      const found = await api.scanLaunchers();
      candidates = found;
      // Preselect what isn't in the library yet.
      selected = Object.fromEntries(found.map((c) => [keyOf(c), c.existingId === null]));
    } catch (e) {
      app.error(e);
    } finally {
      scanning = false;
    }
  }

  function setGroup(p: Platform, value: boolean) {
    for (const c of candidates) if (c.game.platform === p) selected[keyOf(c)] = value;
  }

  async function doImport() {
    const games = candidates.filter((c) => selected[keyOf(c)]).map((c) => $state.snapshot(c.game));
    if (!games.length) return;
    importing = true;
    try {
      const added = await api.importLauncherGames(games);
      for (const g of added) app.upsert(g);
      app.toast(t("{n} Spiele übernommen. Cover werden geladen …", { n: added.length }));
      if (tidyDesktopShortcuts()) await tidyDesktop();
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
      <h1>{t("Spiele aus Launchern einlesen")}</h1>
      <span class="muted">
        {#if scanning}{t("Launcher werden durchsucht …")}{:else}{t("{n} installierte Spiele gefunden, {fresh} davon neu", { n: candidates.length, fresh })}{/if}
      </span>
      <ImportTabs current="launchers" />
    </div>
  </header>

  <div class="body scroll">
    {#if !scanning && candidates.length === 0}
      <div class="empty">
        <h2>{t("Keine installierten Spiele gefunden")}</h2>
        <p class="muted">
          {t("Gelesen werden Steam, Epic, Ubisoft Connect, EA App, Battle.net und Riot. Ist ein Launcher installiert, aber leer, findet sich hier nichts.")}
        </p>
        <button class="btn" onclick={scan}><Icon name="refresh" />{t("Erneut suchen")}</button>
      </div>
    {/if}

    {#each groups as group (group.platform)}
      <section>
        <div class="section-head">
          <h2>{platformLabel(group.platform)}</h2>
          <span class="muted">{t("{n} installiert", { n: group.items.length })}</span>
          <span class="spacer"></span>
          <button class="btn" onclick={() => setGroup(group.platform, true)}>{t("Alle auswählen")}</button>
          <button class="btn" onclick={() => setGroup(group.platform, false)}>{t("Keine")}</button>
        </div>
        <div class="items">
          {#each group.items as c (keyOf(c))}
            {@render item(c)}
          {/each}
        </div>
      </section>
    {/each}
  </div>

  <footer>
    <div class="summary">
      <strong>{t("{n} von {total} ausgewählt", { n: count, total: candidates.length })}</strong>
      <span class="muted">{t("Gestartet wird über den jeweiligen Launcher.")} {tidyDesktopShortcuts() ? t("Passende Desktop-Verknüpfungen werden weggeräumt.") : t("Dein Desktop bleibt unverändert.")}</span>
    </div>
    <span class="spacer"></span>
    <button class="btn big" onclick={() => (app.screen = { kind: "library" })}>{t("Abbrechen")}</button>
    <button class="btn big primary" disabled={!count || importing} onclick={doImport}>
      {importing ? t("Wird übernommen …") : t("{n} übernehmen", { n: count })}
      <Icon name="arrowRight" />
    </button>
  </footer>
</div>

{#snippet item(c: LauncherCandidate)}
  {@const colors = placeholderColors(c.game.name)}
  {@const k = keyOf(c)}
  <label class="item" class:on={selected[k]}>
    <span class="thumb" style="--a: {colors[0]}; --b: {colors[1]}"></span>
    <span class="text">
      <span class="title" title={c.game.name}>{c.game.name}</span>
      <span class="file" title={c.game.installDir ?? ""}>{c.game.installDir ?? c.game.platformId}</span>
      {#if c.existingId !== null}<span class="plat">{t("Schon in der Bibliothek")}</span>{/if}
    </span>
    <input type="checkbox" bind:checked={selected[k]} />
  </label>
{/snippet}
