import { api } from "./api";
import { emptyQuery, type Collection, type Game, type GameQuery, type Layout, type SavedView } from "./types";

export type Screen =
  | { kind: "library" }
  | { kind: "detail"; id: number }
  | { kind: "import" }
  | { kind: "settings" };

export type Source =
  | { kind: "all" }
  | { kind: "favorites" }
  | { kind: "recent" }
  | { kind: "collection"; id: number }
  | { kind: "view"; id: number };

export interface Toast {
  id: number;
  text: string;
  kind: "info" | "error";
  action?: { label: string; run: () => void };
}

let toastId = 0;

class AppState {
  games = $state<Game[]>([]);
  byId = $derived(new Map(this.games.map((g) => [g.id, g])));
  collections = $state<Collection[]>([]);
  views = $state<SavedView[]>([]);
  tags = $state<string[]>([]);
  running = $state<number[]>([]);
  screen = $state<Screen>({ kind: "library" });
  source = $state<Source>({ kind: "all" });
  query = $state<GameQuery>(emptyQuery());
  layout = $state<Layout>({ mode: "grid", coverSize: 200 });
  visibleIds = $state<number[]>([]);
  loaded = $state(false);
  toasts = $state<Toast[]>([]);

  visible = $derived(this.visibleIds.map((id) => this.byId.get(id)).filter((g): g is Game => !!g));

  async load() {
    const [games, collections, views, tags, running] = await Promise.all([
      api.listGames(),
      api.listCollections(),
      api.listViews(),
      api.listTags(),
      api.runningGames(),
    ]);
    this.games = games;
    this.collections = collections;
    this.views = views;
    this.tags = tags;
    this.running = running;
    this.loaded = true;
  }

  async reloadMeta() {
    const [collections, tags] = await Promise.all([api.listCollections(), api.listTags()]);
    this.collections = collections;
    this.tags = tags;
  }

  upsert(game: Game) {
    const i = this.games.findIndex((g) => g.id === game.id);
    if (i >= 0) this.games[i] = game;
    else this.games.push(game);
  }

  remove(id: number) {
    this.games = this.games.filter((g) => g.id !== id);
  }

  /** The query sent to the backend: user filters plus the selected source. */
  effectiveQuery(): GameQuery {
    const q: GameQuery = $state.snapshot(this.query);
    switch (this.source.kind) {
      case "favorites":
        q.favoritesOnly = true;
        break;
      case "recent":
        q.played = "played";
        break;
      case "collection":
        q.collection = this.source.id;
        break;
    }
    return q;
  }

  async refreshVisible() {
    try {
      this.visibleIds = await api.queryGames(this.effectiveQuery());
    } catch (e) {
      this.error(e);
    }
  }

  selectSource(source: Source) {
    this.source = source;
    this.screen = { kind: "library" };
    if (source.kind === "view") {
      const v = this.views.find((x) => x.id === source.id);
      if (v) {
        this.query = structuredClone(v.definition.query);
        this.layout = structuredClone(v.definition.layout);
      }
      return;
    }
    const keepSearch = this.query.search;
    this.query = emptyQuery();
    this.query.search = keepSearch;
    if (source.kind === "recent") {
      this.query.sort = "lastPlayed";
      this.query.descending = true;
    }
  }

  toast(text: string, kind: Toast["kind"] = "info", action?: Toast["action"]) {
    const id = ++toastId;
    this.toasts.push({ id, text, kind, action });
    setTimeout(() => this.dismiss(id), action ? 9000 : 4500);
  }

  dismiss(id: number) {
    this.toasts = this.toasts.filter((t) => t.id !== id);
  }

  error(e: unknown) {
    this.toast(typeof e === "string" ? e : e instanceof Error ? e.message : String(e), "error");
  }

  async launch(id: number) {
    try {
      this.upsert(await api.launchGame(id));
      this.toast(`${this.byId.get(id)?.name ?? "Spiel"} wird gestartet`);
    } catch (e) {
      this.error(e);
    }
  }
}

export const app = new AppState();
