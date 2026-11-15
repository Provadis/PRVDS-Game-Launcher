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

/** Start-up check: marks removed games and offers newly installed ones. */
export async function syncOnStart() {
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
