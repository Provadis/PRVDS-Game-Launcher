export type Platform = "steam" | "epic" | "ubisoft" | "ea" | "battlenet" | "riot" | "other";

export const PLATFORMS: { id: Platform; label: string }[] = [
  { id: "steam", label: "Steam" },
  { id: "epic", label: "Epic" },
  { id: "ubisoft", label: "Ubisoft" },
  { id: "ea", label: "EA" },
  { id: "battlenet", label: "Battle.net" },
  { id: "riot", label: "Riot" },
  { id: "other", label: "Sonstige" },
];

export function platformLabel(p: Platform): string {
  return PLATFORMS.find((x) => x.id === p)?.label ?? p;
}

export type GameSource = "desktop" | "connector";

export interface Game {
  id: number;
  name: string;
  platform: Platform;
  platformId: string | null;
  shortcutPath: string;
  originalPath: string | null;
  journalId: string | null;
  launchTarget: string | null;
  launchArgs: string | null;
  /** "desktop": imported shortcut, "connector": read from a launcher. */
  source: GameSource;
  /** False once a launcher sync no longer finds the game. */
  installed: boolean;
  installDir: string | null;
  processExe: string | null;
  coverPath: string | null;
  heroPath: string | null;
  addedAt: number;
  lastPlayed: number | null;
  playCount: number;
  playtimeSecs: number;
  favorite: boolean;
  hidden: boolean;
  genre: string | null;
  releaseYear: number | null;
  developer: string | null;
  summary: string | null;
  tags: string[];
  collections: number[];
}

export type GamePatch = Partial<{
  name: string;
  favorite: boolean;
  hidden: boolean;
  installDir: string | null;
  processExe: string | null;
}>;

export interface Collection {
  id: number;
  name: string;
  gameCount: number;
}

export interface Session {
  id: number;
  gameId: number;
  startedAt: number;
  endedAt: number;
  durationSecs: number;
}

export type SortField =
  | "name"
  | "platform"
  | "addedAt"
  | "lastPlayed"
  | "playtime"
  | "playCount"
  | "releaseYear"
  | "genre";

export const SORT_FIELDS: { id: SortField; label: string; defaultDesc: boolean }[] = [
  { id: "name", label: "Name", defaultDesc: false },
  { id: "lastPlayed", label: "Zuletzt gespielt", defaultDesc: true },
  { id: "playtime", label: "Spielzeit", defaultDesc: true },
  { id: "addedAt", label: "Hinzugefügt", defaultDesc: true },
  { id: "playCount", label: "Anzahl Starts", defaultDesc: true },
  { id: "releaseYear", label: "Erscheinungsjahr", defaultDesc: true },
  { id: "platform", label: "Plattform", defaultDesc: false },
  { id: "genre", label: "Genre", defaultDesc: false },
];

export type PlayedFilter = "any" | "played" | "neverPlayed";

export interface GameQuery {
  search: string;
  platforms: Platform[];
  tags: string[];
  genre: string | null;
  collection: number | null;
  favoritesOnly: boolean;
  includeHidden: boolean;
  played: PlayedFilter;
  notPlayedForDays: number | null;
  playedWithinDays: number | null;
  sort: SortField;
  descending: boolean;
}

export function emptyQuery(): GameQuery {
  return {
    search: "",
    platforms: [],
    tags: [],
    genre: null,
    collection: null,
    favoritesOnly: false,
    includeHidden: false,
    played: "any",
    notPlayedForDays: null,
    playedWithinDays: null,
    sort: "name",
    descending: false,
  };
}

export type LayoutMode = "grid" | "list";

export interface Layout {
  mode: LayoutMode;
  coverSize: number;
}

export interface ViewDefinition {
  query: GameQuery;
  layout: Layout;
}

export interface SavedView {
  id: number;
  name: string;
  definition: ViewDefinition;
}

export type ShortcutKind = "lnk" | "url" | "exe";

export interface Candidate {
  path: string;
  fileName: string;
  title: string;
  kind: ShortcutKind;
  info: { target: string | null; arguments: string | null; workingDir: string | null; icon: string | null };
  platform: Platform;
  platformId: string | null;
  likelyGame: boolean;
}

export interface ImportReport {
  imported: Game[];
  failed: { path: string; error: string }[];
  /** Games whose shortcut stayed on the (public) desktop: no rights to delete it. */
  keptOnDesktop: string[];
}

export type ArtKind = "cover" | "hero";

export interface ArtOption {
  url: string;
  thumb: string;
  source: string;
}

export interface ArtworkKeys {
  steamgriddb: string | null;
  igdbClientId: string | null;
  igdbClientSecret: string | null;
}

export interface Settings {
  version: string;
  keys: ArtworkKeys;
  dataDir: string;
  shortcutsDir: string;
  desktopDirs: string[];
}

export type TrackerEvent =
  | { type: "started"; gameId: number; at: number }
  | { type: "ended"; gameId: number; startedAt: number; endedAt: number }
  | { type: "notDetected"; gameId: number };

export type UpdateChannel = "stable" | "dev";

export interface UpdateInfo {
  version: string;
  currentVersion: string;
  notes: string | null;
  date: string | null;
}

export interface UpdateCheck {
  /** False for builds without an update key (local builds). */
  enabled: boolean;
  update: UpdateInfo | null;
}

/** Launchers the connectors can read. */
export const CONNECTOR_PLATFORMS: Platform[] = ["steam", "epic", "ubisoft", "ea", "battlenet", "riot"];

export type Launch = { kind: "url"; url: string } | { kind: "exe"; path: string; args: string };

export interface InstalledGame {
  platform: Platform;
  platformId: string;
  name: string;
  installDir: string | null;
  launch: Launch;
  processExe: string | null;
}

export interface LauncherCandidate {
  game: InstalledGame;
  /** Library game it matches, if any. */
  existingId: number | null;
}

export interface SyncReport {
  newGames: number;
  uninstalled: Game[];
  reinstalled: Game[];
}

export interface BoardSlot {
  col: number;
  row: number;
  gameId: number;
}

export interface BoardField {
  id: string;
  title: string;
  /** Position in grid steps. */
  x: number;
  y: number;
  /** Size in cover slots. */
  cols: number;
  rows: number;
  slots: BoardSlot[];
}

export interface Board {
  id: number;
  name: string;
  fields: BoardField[];
}

export const MAX_FIELD_COLS = 12;
export const MAX_FIELD_ROWS = 8;
