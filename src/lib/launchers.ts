import { api } from "./api";
import { app } from "./state.svelte";

function read(key: string): string | null {
  try {
    return localStorage.getItem(key);
  } catch {
    return null;
  }
}

/** Whether the launchers are checked at start-up (default on). */
export function syncLaunchersOnStart(): boolean {
  return read("syncLaunchers") !== "false";
}

export function setSyncLaunchersOnStart(on: boolean) {
  try {
    localStorage.setItem("syncLaunchers", String(on));
  } catch {
    // Storage unavailable: the choice lasts for this session only.
  }
}

/** Whether desktop shortcuts of launcher games are tidied away (default off). */
export function tidyDesktopShortcuts(): boolean {
  return read("tidyDesktop") === "true";
}

export function setTidyDesktopShortcuts(on: boolean) {
  try {
    localStorage.setItem("tidyDesktop", String(on));
  } catch {
    // Storage unavailable: the choice lasts for this session only.
  }
}

/** Moves desktop shortcuts of launcher games into the library folder (positions are kept). */
export async function tidyDesktop(quiet = false) {
  try {
    const tidied = await api.tidyDesktop();
    for (const g of tidied) app.upsert(g);
    if (tidied.length && !quiet) {
      app.toast(
        tidied.length === 1
          ? `Desktop-Verknüpfung von „${tidied[0].name}“ weggeräumt`
          : `${tidied.length} Desktop-Verknüpfungen weggeräumt`,
      );
    }
    return tidied.length;
  } catch (e) {
    if (!quiet) app.error(e);
    return 0;
  }
}

/** Start-up check: marks removed games and offers newly installed ones. */
export async function syncOnStart() {
  if (tidyDesktopShortcuts()) await tidyDesktop();
  if (!syncLaunchersOnStart()) return;
  try {
    const r = await api.syncLaunchers();
    for (const g of [...r.uninstalled, ...r.reinstalled]) app.upsert(g);
    if (r.uninstalled.length) {
      const n = r.uninstalled.length;
      app.toast(
        n === 1
          ? `„${r.uninstalled[0].name}“ ist nicht mehr installiert.`
          : `${n} Spiele sind nicht mehr installiert.`,
      );
    }
    // Only speak up when there is more than last time, so games the user
    // chose not to import don't nag on every start.
    const before = Number(read("launcherNewGames") ?? "0");
    try {
      localStorage.setItem("launcherNewGames", String(r.newGames));
    } catch {
      // ignore
    }
    if (r.newGames > before) {
      app.toast(
        r.newGames === 1 ? "1 neues Spiel in deinen Launchern gefunden" : `${r.newGames} neue Spiele in deinen Launchern gefunden`,
        "info",
        { label: "Ansehen", run: () => (app.screen = { kind: "import", from: "launchers" }) },
      );
    }
  } catch {
    // A launcher that can't be read must not disturb the start.
  }
}
