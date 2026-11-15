//! Tauri commands called from the frontend.

use std::path::{Path, PathBuf};

use std::collections::HashMap;
use std::time::Duration;

use launcher_core::artwork::{ArtKind, ArtOption, ArtworkKeys};
use launcher_core::connectors::{self, CONNECTOR_PLATFORMS, InstalledGame};
use launcher_core::desktop_icons::{self, IconPosition};
use launcher_core::importer::EntryState;
use launcher_core::locate::locate;
use launcher_core::model::{Collection, Game, GamePatch, GameSource, NewGame, Platform, Session};
use launcher_core::playtime::Matcher;
use launcher_core::query::{GameQuery, SavedView, ViewDefinition};
use launcher_core::scanner::{self, Candidate};
use launcher_core::util::now;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::state::AppState;

type CmdResult<T> = Result<T, String>;

fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}

/// Runs blocking work (network, disk) off the main thread.
async fn blocking<T: Send + 'static>(
    f: impl FnOnce() -> CmdResult<T> + Send + 'static,
) -> CmdResult<T> {
    tauri::async_runtime::spawn_blocking(f).await.map_err(err)?
}

// ---- import -----------------------------------------------------------------

#[tauri::command]
pub fn scan_desktop(state: State<AppState>) -> Vec<Candidate> {
    // Shortcuts we could only copy (public desktop) are still there: hide them.
    let kept: Vec<String> = state
        .importer
        .entries()
        .unwrap_or_default()
        .into_iter()
        .filter(|e| e.original_kept && e.state == EntryState::Moved)
        .map(|e| e.original_path.to_lowercase())
        .collect();
    scanner::scan_dirs(&scanner::desktop_dirs())
        .into_iter()
        .filter(|c| !c.title.to_lowercase().contains("prvds game launcher"))
        .filter(|c| !kept.contains(&c.path.to_lowercase()))
        .collect()
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportFailure {
    path: String,
    error: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportReport {
    imported: Vec<Game>,
    failed: Vec<ImportFailure>,
    /// Names of games whose shortcut had to stay on the desktop (no rights
    /// to delete it there, typically the public desktop).
    kept_on_desktop: Vec<String>,
}

#[tauri::command]
pub fn import_shortcuts(
    app: AppHandle,
    state: State<AppState>,
    paths: Vec<String>,
) -> ImportReport {
    let mut imported = Vec::new();
    let mut failed = Vec::new();
    let mut kept_on_desktop = Vec::new();
    // Remember where the icons sat, so "back to desktop" can put them there again.
    let positions = desktop_icons::read_positions();
    for path in paths {
        let pos = desktop_position(&positions, Path::new(&path));
        match import_one(&state, Path::new(&path), pos) {
            Ok((game, kept)) => {
                if kept {
                    kept_on_desktop.push(game.name.clone());
                }
                imported.push(game);
            }
            Err(error) => failed.push(ImportFailure { path, error }),
        }
    }
    let ids: Vec<i64> = imported.iter().map(|g| g.id).collect();
    std::thread::spawn(move || enrich_games(app, ids));
    ImportReport {
        imported,
        failed,
        kept_on_desktop,
    }
}

/// The icon position of `path`, if the file lies directly on the desktop.
fn desktop_position(
    positions: &HashMap<String, IconPosition>,
    path: &Path,
) -> Option<IconPosition> {
    let on_desktop = path
        .parent()
        .is_some_and(|dir| scanner::desktop_dirs().iter().any(|d| d == dir));
    let name = path.file_name()?.to_string_lossy();
    on_desktop
        .then(|| positions.get(&desktop_icons::key(&name)).copied())
        .flatten()
}

/// Imports one shortcut. The flag says whether the original had to stay.
fn import_one(state: &AppState, path: &Path, pos: Option<IconPosition>) -> CmdResult<(Game, bool)> {
    let candidate = scanner::inspect(path).ok_or("Keine Verknüpfung")?;
    let entry = state.importer.move_in_at(path, pos).map_err(err)?;
    let install = locate(
        candidate.platform,
        candidate.platform_id.as_deref(),
        candidate
            .info
            .target
            .as_deref()
            .filter(|_| candidate.kind != scanner::ShortcutKind::Url),
        &candidate.title,
    );
    let new = NewGame {
        name: candidate.title,
        platform: Some(candidate.platform),
        platform_id: candidate.platform_id,
        shortcut_path: entry.stored_path.clone(),
        original_path: Some(entry.original_path.clone()),
        journal_id: Some(entry.id.clone()),
        launch_target: candidate.info.target,
        install_dir: install.install_dir,
        process_exe: install.process_exe,
        ..Default::default()
    };
    let lib = state.lib.lock().unwrap();
    match lib.add_game(&new) {
        Ok(g) => Ok((g, entry.original_kept)),
        Err(e) => {
            // Keep desktop and library consistent: put the shortcut back.
            let _ = state.importer.restore(&entry.id);
            Err(err(e))
        }
    }
}

// ---- launchers (connectors) ---------------------------------------------------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LauncherCandidate {
    game: InstalledGame,
    /// The library game it matches, if any.
    existing_id: Option<i64>,
}

/// All games installed in the given launchers (all when empty).
#[tauri::command]
pub async fn scan_launchers(
    app: AppHandle,
    platforms: Vec<Platform>,
) -> CmdResult<Vec<LauncherCandidate>> {
    blocking(move || {
        let platforms = if platforms.is_empty() {
            CONNECTOR_PLATFORMS.to_vec()
        } else {
            platforms
        };
        let found = connectors::scan(&platforms);
        let games = app
            .state::<AppState>()
            .lib
            .lock()
            .unwrap()
            .games()
            .map_err(err)?;
        Ok(found
            .into_iter()
            .map(|g| LauncherCandidate {
                existing_id: connectors::match_existing(&g, &games).map(|x| x.id),
                game: g,
            })
            .collect())
    })
    .await
}

/// Adds launcher games to the library. Games that are already there (e.g.
/// from a desktop shortcut) only get the missing store data filled in.
#[tauri::command]
pub fn import_launcher_games(
    app: AppHandle,
    state: State<AppState>,
    games: Vec<InstalledGame>,
) -> CmdResult<Vec<Game>> {
    let lib = state.lib.lock().unwrap();
    let existing = lib.games().map_err(err)?;
    let mut out = Vec::new();
    let mut new_ids = Vec::new();
    for g in games {
        let game = match connectors::match_existing(&g, &existing) {
            Some(known) => lib
                .fill_store_info(
                    known.id,
                    g.platform,
                    &g.platform_id,
                    g.install_dir.as_deref(),
                    g.process_exe.as_deref(),
                )
                .map_err(err)?,
            None => {
                let (target, args) = g.launch_parts();
                let added = lib
                    .add_game(&NewGame {
                        name: g.name.clone(),
                        platform: Some(g.platform),
                        platform_id: Some(g.platform_id.clone()),
                        launch_target: Some(target),
                        launch_args: args,
                        install_dir: g.install_dir.clone(),
                        process_exe: g.process_exe.clone(),
                        source: GameSource::Connector,
                        ..Default::default()
                    })
                    .map_err(err)?;
                new_ids.push(added.id);
                added
            }
        };
        out.push(game);
    }
    drop(lib);
    std::thread::spawn(move || enrich_games(app, new_ids));
    Ok(out)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncReport {
    /// Games newly found in the launchers that are not in the library yet.
    new_games: usize,
    /// Connector games no launcher has any more.
    uninstalled: Vec<Game>,
    /// Games that were marked uninstalled and are back.
    reinstalled: Vec<Game>,
}

/// Start-up check: which connector games are (still) installed?
#[tauri::command]
pub async fn sync_launchers(app: AppHandle) -> CmdResult<SyncReport> {
    blocking(move || {
        let found = connectors::scan(&CONNECTOR_PLATFORMS);
        let state = app.state::<AppState>();
        let lib = state.lib.lock().unwrap();
        let games = lib.games().map_err(err)?;
        let mut report = SyncReport {
            new_games: 0,
            uninstalled: Vec::new(),
            reinstalled: Vec::new(),
        };
        let mut seen = std::collections::HashSet::new();
        for f in &found {
            match connectors::match_existing(f, &games) {
                Some(g) => {
                    seen.insert(g.id);
                    if !g.installed {
                        lib.set_installed(g.id, true).map_err(err)?;
                        report.reinstalled.push(lib.game(g.id).map_err(err)?);
                    }
                }
                None => report.new_games += 1,
            }
        }
        for g in games
            .iter()
            .filter(|g| g.source == GameSource::Connector && g.installed && !seen.contains(&g.id))
        {
            lib.set_installed(g.id, false).map_err(err)?;
            report.uninstalled.push(lib.game(g.id).map_err(err)?);
        }
        Ok(report)
    })
    .await
}

/// Fetches covers, hero images and metadata for freshly imported games.
fn enrich_games(app: AppHandle, ids: Vec<i64>) {
    let state = app.state::<AppState>();
    for id in ids {
        let Ok(game) = state.lib.lock().unwrap().game(id) else {
            continue;
        };
        let mut patch = GamePatch::default();
        if game.cover_path.is_none()
            && let Some(o) = state.art.best(&game, ArtKind::Cover)
            && let Ok(p) = download_art(&state, &game, ArtKind::Cover, &o.url)
        {
            patch.cover_path = Some(Some(p));
        }
        if game.hero_path.is_none()
            && let Some(o) = state.art.best(&game, ArtKind::Hero)
            && let Ok(p) = download_art(&state, &game, ArtKind::Hero, &o.url)
        {
            patch.hero_path = Some(Some(p));
        }
        if let Some(m) = state.art.metadata(&game) {
            patch.genre = m.genre.map(Some);
            patch.release_year = m.release_year.map(Some);
            patch.developer = m.developer.map(Some);
            patch.summary = m.summary.map(Some);
        }
        if let Ok(updated) = state.lib.lock().unwrap().update_game(id, &patch) {
            let _ = app.emit("game-updated", updated);
        }
    }
}

fn art_file(state: &AppState, game: &Game, kind: ArtKind, ext: &str) -> PathBuf {
    let k = match kind {
        ArtKind::Cover => "cover",
        ArtKind::Hero => "hero",
    };
    state
        .covers_dir()
        .join(format!("{}-{k}-{}.{ext}", game.id, now()))
}

fn ext_of(s: &str) -> &str {
    let lower = s.to_ascii_lowercase();
    let clean = lower.split(['?', '#']).next().unwrap_or("");
    for e in ["png", "webp", "jpeg", "gif"] {
        if clean.ends_with(&format!(".{e}")) {
            return e;
        }
    }
    "jpg"
}

fn download_art(state: &AppState, game: &Game, kind: ArtKind, url: &str) -> CmdResult<String> {
    let dest = art_file(state, game, kind, ext_of(url));
    state.art.download(url, &dest).map_err(err)?;
    Ok(dest.to_string_lossy().into_owned())
}

fn remove_art_file(state: &AppState, path: Option<&str>) {
    if let Some(p) = path {
        // Only ever delete inside our own covers folder.
        if Path::new(p).starts_with(state.covers_dir()) {
            let _ = std::fs::remove_file(p);
        }
    }
}

// ---- games ------------------------------------------------------------------

#[tauri::command]
pub fn list_games(state: State<AppState>) -> CmdResult<Vec<Game>> {
    state.lib.lock().unwrap().games().map_err(err)
}

/// Ids of the games matching the query, in display order.
#[tauri::command]
pub fn query_games(state: State<AppState>, query: GameQuery) -> CmdResult<Vec<i64>> {
    let games = state.lib.lock().unwrap().games().map_err(err)?;
    Ok(query
        .apply(&games, now())
        .into_iter()
        .map(|g| g.id)
        .collect())
}

#[tauri::command]
pub fn get_game(state: State<AppState>, id: i64) -> CmdResult<Game> {
    state.lib.lock().unwrap().game(id).map_err(err)
}

#[tauri::command]
pub fn update_game(state: State<AppState>, id: i64, patch: GamePatch) -> CmdResult<Game> {
    state
        .lib
        .lock()
        .unwrap()
        .update_game(id, &patch)
        .map_err(err)
}

#[tauri::command]
pub fn launch_game(app: AppHandle, state: State<AppState>, id: i64) -> CmdResult<Game> {
    let game = state.lib.lock().unwrap().game(id).map_err(err)?;
    start(&game)?;
    let at = now();
    let lib = state.lib.lock().unwrap();
    lib.record_launch(id, at).map_err(err)?;
    state.tracker.lock().unwrap().launched(
        id,
        Matcher {
            install_dir: game.install_dir.clone(),
            exe_name: game.process_exe.clone(),
        },
        at,
    );
    let game = lib.game(id).map_err(err)?;
    let _ = app.emit("game-updated", game.clone());
    Ok(game)
}

/// Starts a game: through its shortcut (desktop imports) or straight via the
/// store URL / exe the launcher told us (connector games).
fn start(game: &Game) -> CmdResult<()> {
    if !game.shortcut_path.is_empty() {
        if !Path::new(&game.shortcut_path).exists() {
            return Err(format!(
                "Verknüpfung nicht gefunden: {}",
                game.shortcut_path
            ));
        }
        return open::that_detached(&game.shortcut_path).map_err(err);
    }
    let target = game
        .launch_target
        .as_deref()
        .ok_or("Für dieses Spiel ist nicht bekannt, wie es gestartet wird")?;
    if target.contains("://") {
        return open::that_detached(target).map_err(err);
    }
    if !Path::new(target).exists() {
        return Err(format!("Programm nicht gefunden: {target}"));
    }
    let mut cmd = std::process::Command::new(target);
    cmd.args(split_args(game.launch_args.as_deref().unwrap_or("")));
    if let Some(dir) = Path::new(target).parent() {
        cmd.current_dir(dir);
    }
    cmd.spawn().map(|_| ()).map_err(err)
}

/// Splits a command line into arguments, keeping "quoted parts" together.
fn split_args(args: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut quoted = false;
    for c in args.chars() {
        match c {
            '"' => quoted = !quoted,
            c if c.is_whitespace() && !quoted => {
                if !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                }
            }
            c => cur.push(c),
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

#[tauri::command]
pub fn running_games(state: State<AppState>) -> Vec<i64> {
    state
        .tracker
        .lock()
        .unwrap()
        .running()
        .into_iter()
        .collect()
}

#[tauri::command]
pub fn sessions(state: State<AppState>, id: i64) -> CmdResult<Vec<Session>> {
    state.lib.lock().unwrap().sessions(id).map_err(err)
}

/// Puts the shortcut back on the desktop and removes the game from the library.
#[tauri::command]
pub fn undo_import(state: State<AppState>, id: i64) -> CmdResult<String> {
    let (restored, pos) = restore_game(&state, id)?;
    place_icons(pos.into_iter().collect());
    Ok(restored.to_string_lossy().into_owned())
}

/// Puts a game's shortcut back and removes the game. Returns the restored
/// path and, if known, the icon's old desktop position for that file.
fn restore_game(state: &AppState, id: i64) -> CmdResult<(PathBuf, Option<(String, IconPosition)>)> {
    let lib = state.lib.lock().unwrap();
    let game = lib.game(id).map_err(err)?;
    let journal_id = game
        .journal_id
        .clone()
        .ok_or("Für dieses Spiel gibt es keinen Import-Eintrag")?;
    let position = state
        .importer
        .entry(&journal_id)
        .ok()
        .and_then(|e| e.desktop_position);
    let restored = state.importer.restore(&journal_id).map_err(err)?;
    lib.delete_game(id).map_err(err)?;
    remove_art_file(state, game.cover_path.as_deref());
    remove_art_file(state, game.hero_path.as_deref());
    let placement = position.zip(
        restored
            .file_name()
            .map(|n| n.to_string_lossy().into_owned()),
    );
    Ok((restored, placement.map(|(p, n)| (n, p))))
}

/// Moves restored icons back to their old spots, in the background: Explorer
/// needs a moment to show new files before they can be positioned.
fn place_icons(items: Vec<(String, IconPosition)>) {
    if !items.is_empty() {
        std::thread::spawn(move || {
            desktop_icons::apply_positions_when_visible(items, Duration::from_secs(5))
        });
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RestoreReport {
    restored: usize,
    failed: Vec<ImportFailure>,
}

#[tauri::command]
pub fn restore_all(state: State<AppState>) -> CmdResult<RestoreReport> {
    let games = state.lib.lock().unwrap().games().map_err(err)?;
    let mut restored = 0;
    let mut failed = Vec::new();
    let mut placements = Vec::new();
    for g in games.into_iter().filter(|g| g.journal_id.is_some()) {
        match restore_game(&state, g.id) {
            Ok((_, placement)) => {
                restored += 1;
                placements.extend(placement);
            }
            Err(error) => failed.push(ImportFailure {
                path: g.shortcut_path,
                error,
            }),
        }
    }
    place_icons(placements);
    Ok(RestoreReport { restored, failed })
}

// ---- tags & collections -----------------------------------------------------

#[tauri::command]
pub fn list_tags(state: State<AppState>) -> CmdResult<Vec<String>> {
    state.lib.lock().unwrap().tags().map_err(err)
}

#[tauri::command]
pub fn add_tag(state: State<AppState>, id: i64, tag: String) -> CmdResult<Game> {
    state.lib.lock().unwrap().add_tag(id, &tag).map_err(err)
}

#[tauri::command]
pub fn remove_tag(state: State<AppState>, id: i64, tag: String) -> CmdResult<Game> {
    state.lib.lock().unwrap().remove_tag(id, &tag).map_err(err)
}

#[tauri::command]
pub fn list_collections(state: State<AppState>) -> CmdResult<Vec<Collection>> {
    state.lib.lock().unwrap().collections().map_err(err)
}

#[tauri::command]
pub fn create_collection(state: State<AppState>, name: String) -> CmdResult<Collection> {
    state
        .lib
        .lock()
        .unwrap()
        .create_collection(&name)
        .map_err(err)
}

#[tauri::command]
pub fn rename_collection(state: State<AppState>, id: i64, name: String) -> CmdResult<()> {
    state
        .lib
        .lock()
        .unwrap()
        .rename_collection(id, &name)
        .map_err(err)
}

#[tauri::command]
pub fn delete_collection(state: State<AppState>, id: i64) -> CmdResult<()> {
    state.lib.lock().unwrap().delete_collection(id).map_err(err)
}

#[tauri::command]
pub fn set_in_collection(
    state: State<AppState>,
    collection_id: i64,
    game_id: i64,
    member: bool,
) -> CmdResult<Game> {
    state
        .lib
        .lock()
        .unwrap()
        .set_in_collection(collection_id, game_id, member)
        .map_err(err)
}

// ---- saved views ------------------------------------------------------------

#[tauri::command]
pub fn list_views(state: State<AppState>) -> CmdResult<Vec<SavedView>> {
    state.lib.lock().unwrap().views().map_err(err)
}

#[tauri::command]
pub fn save_view(
    state: State<AppState>,
    id: Option<i64>,
    name: String,
    definition: ViewDefinition,
) -> CmdResult<SavedView> {
    state
        .lib
        .lock()
        .unwrap()
        .save_view(id, &name, &definition)
        .map_err(err)
}

#[tauri::command]
pub fn delete_view(state: State<AppState>, id: i64) -> CmdResult<()> {
    state.lib.lock().unwrap().delete_view(id).map_err(err)
}

// ---- artwork & metadata -----------------------------------------------------

#[tauri::command]
pub async fn art_options(app: AppHandle, id: i64, kind: ArtKind) -> CmdResult<Vec<ArtOption>> {
    blocking(move || {
        let state = app.state::<AppState>();
        let game = state.lib.lock().unwrap().game(id).map_err(err)?;
        Ok(state.art.options(&game, kind))
    })
    .await
}

fn apply_art(state: &AppState, id: i64, kind: ArtKind, new_path: String) -> CmdResult<Game> {
    let lib = state.lib.lock().unwrap();
    let game = lib.game(id).map_err(err)?;
    let (old, patch) = match kind {
        ArtKind::Cover => (
            game.cover_path.clone(),
            GamePatch {
                cover_path: Some(Some(new_path)),
                ..Default::default()
            },
        ),
        ArtKind::Hero => (
            game.hero_path.clone(),
            GamePatch {
                hero_path: Some(Some(new_path)),
                ..Default::default()
            },
        ),
    };
    let updated = lib.update_game(id, &patch).map_err(err)?;
    remove_art_file(state, old.as_deref());
    Ok(updated)
}

#[tauri::command]
pub async fn set_art_from_url(
    app: AppHandle,
    id: i64,
    kind: ArtKind,
    url: String,
) -> CmdResult<Game> {
    blocking(move || {
        let state = app.state::<AppState>();
        let game = state.lib.lock().unwrap().game(id).map_err(err)?;
        let path = download_art(&state, &game, kind, &url)?;
        let updated = apply_art(&state, id, kind, path)?;
        let _ = app.emit("game-updated", updated.clone());
        Ok(updated)
    })
    .await
}

#[tauri::command]
pub fn set_art_from_file(
    app: AppHandle,
    state: State<AppState>,
    id: i64,
    kind: ArtKind,
    path: String,
) -> CmdResult<Game> {
    let game = state.lib.lock().unwrap().game(id).map_err(err)?;
    let ext = ext_of(&path);
    if !["png", "jpg", "jpeg", "webp", "gif"].contains(&ext)
        && !path.to_ascii_lowercase().ends_with(".jpg")
    {
        return Err("Bitte eine Bilddatei wählen (PNG, JPG, WEBP)".into());
    }
    let dest = art_file(&state, &game, kind, ext);
    std::fs::copy(&path, &dest).map_err(err)?;
    let updated = apply_art(&state, id, kind, dest.to_string_lossy().into_owned())?;
    let _ = app.emit("game-updated", updated.clone());
    Ok(updated)
}

/// Reloads metadata and fills in missing images.
#[tauri::command]
pub async fn refresh_metadata(app: AppHandle, id: i64) -> CmdResult<Game> {
    blocking(move || {
        enrich_games(app.clone(), vec![id]);
        app.state::<AppState>()
            .lib
            .lock()
            .unwrap()
            .game(id)
            .map_err(err)
    })
    .await
}

// ---- settings ---------------------------------------------------------------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    /// App version, e.g. `0.2.0` or `0.2.0-dev.42` for dev builds.
    version: String,
    keys: ArtworkKeys,
    data_dir: String,
    shortcuts_dir: String,
    desktop_dirs: Vec<String>,
}

#[tauri::command]
pub fn get_settings(app: AppHandle, state: State<AppState>) -> Settings {
    Settings {
        version: app.package_info().version.to_string(),
        keys: state.secrets.load(),
        data_dir: state.data_dir.to_string_lossy().into_owned(),
        shortcuts_dir: state
            .importer
            .shortcuts_dir()
            .to_string_lossy()
            .into_owned(),
        desktop_dirs: scanner::desktop_dirs()
            .iter()
            .map(|d| d.to_string_lossy().into_owned())
            .collect(),
    }
}

#[tauri::command]
pub fn save_keys(state: State<AppState>, keys: ArtworkKeys) -> CmdResult<()> {
    state.secrets.save(&keys).map_err(err)?;
    state.art.set_keys(keys);
    Ok(())
}

#[tauri::command]
pub fn open_data_folder(state: State<AppState>) -> CmdResult<()> {
    open::that_detached(&state.data_dir).map_err(err)
}
