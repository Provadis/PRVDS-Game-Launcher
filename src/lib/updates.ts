import { api } from "./api";
import { app } from "./state.svelte";
import type { UpdateChannel } from "./types";
import { t } from "./i18n.svelte";

// Per-device preferences; losing them only resets to the defaults.
function read(key: string): string | null {
  try {
    return localStorage.getItem(key);
  } catch {
    return null;
  }
}

function write(key: string, value: string) {
  try {
    localStorage.setItem(key, value);
  } catch {
    // Storage unavailable: the choice lasts for this session only.
  }
}

export function updateChannel(): UpdateChannel {
  return read("updateChannel") === "dev" ? "dev" : "stable";
}

export function setUpdateChannel(c: UpdateChannel) {
  write("updateChannel", c);
}

export function autoCheckUpdates(): boolean {
  return read("autoCheckUpdates") !== "false";
}

export function setAutoCheckUpdates(on: boolean) {
  write("autoCheckUpdates", String(on));
}

/** Quiet check at start-up: only speaks up when there is something new. */
export async function checkOnStart() {
  if (!autoCheckUpdates()) return;
  try {
    const r = await api.checkUpdate(updateChannel());
    if (r.update) {
      app.toast(t("Update auf Version {v} verfügbar", { v: r.update.version }), "info", {
        label: t("Ansehen"),
        run: () => (app.screen = { kind: "settings" }),
      });
    }
  } catch {
    // Offline or no release yet: try again next start.
  }
}
