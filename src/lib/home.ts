import { app } from "./state.svelte";

/** The board shown at start instead of the library, if any. */
export function homeBoardId(): number | null {
  try {
    const v = Number(localStorage.getItem("homeBoard"));
    return Number.isInteger(v) && v > 0 ? v : null;
  } catch {
    return null;
  }
}

export function setHomeBoard(id: number | null) {
  try {
    if (id === null) localStorage.removeItem("homeBoard");
    else localStorage.setItem("homeBoard", String(id));
  } catch {
    // Storage unavailable: the choice lasts for this session only.
  }
}

/** Opens the start board, if one is set and still exists. */
export function openHome() {
  const id = homeBoardId();
  if (id !== null && app.boards.some((b) => b.id === id)) app.screen = { kind: "board", id };
}
