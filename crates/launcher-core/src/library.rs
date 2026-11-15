//! SQLite backed game library.

use std::collections::HashSet;
use std::path::Path;

use rusqlite::{Connection, OptionalExtension, Row, params};

use crate::boards::{Board, BoardField, BoardLayout, normalize_fields};
use crate::error::{Error, Result};
use crate::model::{Collection, Game, GamePatch, GameSource, NewGame, Platform, Session};
use crate::query::{SavedView, ViewDefinition};
use crate::util::now;

const MIGRATIONS: &[&str] = &[
    r#"
CREATE TABLE games (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL,
    platform TEXT NOT NULL,
    platform_id TEXT,
    shortcut_path TEXT NOT NULL,
    original_path TEXT,
    journal_id TEXT,
    launch_target TEXT,
    install_dir TEXT,
    process_exe TEXT,
    cover_path TEXT,
    hero_path TEXT,
    added_at INTEGER NOT NULL,
    last_played INTEGER,
    play_count INTEGER NOT NULL DEFAULT 0,
    playtime_secs INTEGER NOT NULL DEFAULT 0,
    favorite INTEGER NOT NULL DEFAULT 0,
    hidden INTEGER NOT NULL DEFAULT 0,
    genre TEXT,
    release_year INTEGER,
    developer TEXT,
    summary TEXT
);
CREATE TABLE tags (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL UNIQUE COLLATE NOCASE
);
CREATE TABLE game_tags (
    game_id INTEGER NOT NULL REFERENCES games(id) ON DELETE CASCADE,
    tag_id INTEGER NOT NULL REFERENCES tags(id) ON DELETE CASCADE,
    PRIMARY KEY (game_id, tag_id)
);
CREATE TABLE collections (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL,
    position INTEGER NOT NULL DEFAULT 0
);
CREATE TABLE collection_games (
    collection_id INTEGER NOT NULL REFERENCES collections(id) ON DELETE CASCADE,
    game_id INTEGER NOT NULL REFERENCES games(id) ON DELETE CASCADE,
    PRIMARY KEY (collection_id, game_id)
);
CREATE TABLE views (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL,
    definition TEXT NOT NULL,
    position INTEGER NOT NULL DEFAULT 0
);
CREATE TABLE sessions (
    id INTEGER PRIMARY KEY,
    game_id INTEGER NOT NULL REFERENCES games(id) ON DELETE CASCADE,
    started_at INTEGER NOT NULL,
    ended_at INTEGER NOT NULL,
    duration_secs INTEGER NOT NULL
);
CREATE INDEX sessions_game ON sessions(game_id, started_at);
"#,
    // 0.4: games read straight from the launchers.
    r#"
ALTER TABLE games ADD COLUMN launch_args TEXT;
ALTER TABLE games ADD COLUMN source TEXT NOT NULL DEFAULT 'desktop';
ALTER TABLE games ADD COLUMN installed INTEGER NOT NULL DEFAULT 1;
"#,
    // 0.5: boards (hand-arranged views), stored as one JSON layout each.
    r#"
CREATE TABLE boards (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL,
    layout TEXT NOT NULL DEFAULT '{}',
    position INTEGER NOT NULL DEFAULT 0
);
"#,
];

pub struct Library {
    conn: Connection,
}

const GAME_COLUMNS: &str = "id, name, platform, platform_id, shortcut_path, original_path, journal_id, \
    launch_target, install_dir, process_exe, cover_path, hero_path, added_at, last_played, play_count, \
    playtime_secs, favorite, hidden, genre, release_year, developer, summary, launch_args, source, installed";

fn game_from_row(row: &Row) -> rusqlite::Result<Game> {
    Ok(Game {
        id: row.get(0)?,
        name: row.get(1)?,
        platform: Platform::parse(&row.get::<_, String>(2)?),
        platform_id: row.get(3)?,
        shortcut_path: row.get(4)?,
        original_path: row.get(5)?,
        journal_id: row.get(6)?,
        launch_target: row.get(7)?,
        launch_args: row.get(22)?,
        source: GameSource::parse(&row.get::<_, String>(23)?),
        installed: row.get(24)?,
        install_dir: row.get(8)?,
        process_exe: row.get(9)?,
        cover_path: row.get(10)?,
        hero_path: row.get(11)?,
        added_at: row.get(12)?,
        last_played: row.get(13)?,
        play_count: row.get(14)?,
        playtime_secs: row.get(15)?,
        favorite: row.get(16)?,
        hidden: row.get(17)?,
        genre: row.get(18)?,
        release_year: row.get(19)?,
        developer: row.get(20)?,
        summary: row.get(21)?,
        tags: Vec::new(),
        collections: Vec::new(),
    })
}

impl Library {
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|source| Error::Io {
                path: parent.into(),
                source,
            })?;
        }
        Self::init(Connection::open(path)?)
    }

    pub fn open_in_memory() -> Result<Self> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(conn: Connection) -> Result<Self> {
        conn.execute_batch("PRAGMA foreign_keys = ON; PRAGMA journal_mode = WAL;")?;
        let version: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
        for (i, sql) in MIGRATIONS.iter().enumerate().skip(version as usize) {
            conn.execute_batch(&format!(
                "BEGIN; {sql} PRAGMA user_version = {}; COMMIT;",
                i + 1
            ))?;
        }
        Ok(Self { conn })
    }

    // ---- games -------------------------------------------------------------

    pub fn add_game(&self, g: &NewGame) -> Result<Game> {
        self.conn.execute(
            "INSERT INTO games (name, platform, platform_id, shortcut_path, original_path, journal_id, \
             launch_target, install_dir, process_exe, added_at, launch_args, source) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                g.name,
                g.platform.unwrap_or(Platform::Other).as_str(),
                g.platform_id,
                g.shortcut_path,
                g.original_path,
                g.journal_id,
                g.launch_target,
                g.install_dir,
                g.process_exe,
                now(),
                g.launch_args,
                g.source.as_str()
            ],
        )?;
        self.game(self.conn.last_insert_rowid())
    }

    /// Marks a game as (not) installed, e.g. after a launcher sync.
    pub fn set_installed(&self, id: i64, installed: bool) -> Result<()> {
        self.conn.execute(
            "UPDATE games SET installed = ?2 WHERE id = ?1",
            params![id, installed],
        )?;
        Ok(())
    }

    /// Links (or unlinks) a launcher game with the journal entry of its
    /// desktop shortcut, so the shortcut can be put back later.
    pub fn set_desktop_shortcut(
        &self,
        id: i64,
        journal_id: Option<&str>,
        original_path: Option<&str>,
    ) -> Result<()> {
        self.conn.execute(
            "UPDATE games SET journal_id = ?2, original_path = ?3 WHERE id = ?1",
            params![id, journal_id, original_path],
        )?;
        Ok(())
    }

    /// Fills in store data a launcher knows and the game lacks (a desktop
    /// import without platform id, say). Never overwrites what is set.
    pub fn fill_store_info(
        &self,
        id: i64,
        platform: Platform,
        platform_id: &str,
        install_dir: Option<&str>,
        process_exe: Option<&str>,
    ) -> Result<Game> {
        self.conn.execute(
            "UPDATE games SET \
               platform = CASE WHEN platform = 'other' THEN ?2 ELSE platform END, \
               platform_id = COALESCE(platform_id, ?3), \
               install_dir = COALESCE(install_dir, ?4), \
               process_exe = COALESCE(process_exe, ?5), \
               installed = 1 \
             WHERE id = ?1",
            params![id, platform.as_str(), platform_id, install_dir, process_exe],
        )?;
        self.game(id)
    }

    pub fn game(&self, id: i64) -> Result<Game> {
        let mut game = self
            .conn
            .query_row(
                &format!("SELECT {GAME_COLUMNS} FROM games WHERE id = ?1"),
                [id],
                game_from_row,
            )
            .optional()?
            .ok_or_else(|| Error::NotFound(format!("Spiel {id}")))?;
        game.tags = self.tags_of(id)?;
        game.collections = self.collections_of(id)?;
        Ok(game)
    }

    pub fn games(&self) -> Result<Vec<Game>> {
        let mut stmt = self.conn.prepare(&format!(
            "SELECT {GAME_COLUMNS} FROM games ORDER BY name COLLATE NOCASE"
        ))?;
        let mut games: Vec<Game> = stmt
            .query_map([], game_from_row)?
            .collect::<rusqlite::Result<_>>()?;

        let mut tag_stmt = self.conn.prepare(
            "SELECT gt.game_id, t.name FROM game_tags gt JOIN tags t ON t.id = gt.tag_id ORDER BY t.name COLLATE NOCASE",
        )?;
        let tags: Vec<(i64, String)> = tag_stmt
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<rusqlite::Result<_>>()?;
        let mut col_stmt = self
            .conn
            .prepare("SELECT game_id, collection_id FROM collection_games")?;
        let cols: Vec<(i64, i64)> = col_stmt
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<rusqlite::Result<_>>()?;
        let index: std::collections::HashMap<i64, usize> =
            games.iter().enumerate().map(|(i, g)| (g.id, i)).collect();
        for (gid, name) in tags {
            if let Some(&i) = index.get(&gid) {
                games[i].tags.push(name);
            }
        }
        for (gid, cid) in cols {
            if let Some(&i) = index.get(&gid) {
                games[i].collections.push(cid);
            }
        }
        Ok(games)
    }

    /// Paths of all shortcuts that came from the given original location.
    pub fn original_paths(&self) -> Result<Vec<String>> {
        let mut stmt = self
            .conn
            .prepare("SELECT original_path FROM games WHERE original_path IS NOT NULL")?;
        let rows = stmt
            .query_map([], |r| r.get(0))?
            .collect::<rusqlite::Result<_>>()?;
        Ok(rows)
    }

    pub fn update_game(&self, id: i64, p: &GamePatch) -> Result<Game> {
        let tx = self.conn.unchecked_transaction()?;
        macro_rules! set {
            ($field:expr, $col:literal) => {
                if let Some(v) = &$field {
                    tx.execute(
                        concat!("UPDATE games SET ", $col, " = ?1 WHERE id = ?2"),
                        params![v, id],
                    )?;
                }
            };
        }
        if let Some(name) = &p.name {
            let name = name.trim();
            if name.is_empty() {
                return Err(Error::Other("Der Name darf nicht leer sein".into()));
            }
            tx.execute(
                "UPDATE games SET name = ?1 WHERE id = ?2",
                params![name, id],
            )?;
        }
        set!(p.favorite, "favorite");
        set!(p.hidden, "hidden");
        set!(p.install_dir, "install_dir");
        set!(p.process_exe, "process_exe");
        set!(p.cover_path, "cover_path");
        set!(p.hero_path, "hero_path");
        set!(p.genre, "genre");
        set!(p.release_year, "release_year");
        set!(p.developer, "developer");
        set!(p.summary, "summary");
        tx.commit()?;
        self.game(id)
    }

    pub fn delete_game(&self, id: i64) -> Result<()> {
        self.conn.execute("DELETE FROM games WHERE id = ?1", [id])?;
        Ok(())
    }

    pub fn record_launch(&self, id: i64, at: i64) -> Result<()> {
        self.conn.execute(
            "UPDATE games SET play_count = play_count + 1, last_played = ?1 WHERE id = ?2",
            params![at, id],
        )?;
        Ok(())
    }

    // ---- sessions ----------------------------------------------------------

    pub fn add_session(&self, game_id: i64, started_at: i64, ended_at: i64) -> Result<Session> {
        let duration = (ended_at - started_at).max(0);
        let tx = self.conn.unchecked_transaction()?;
        tx.execute(
            "INSERT INTO sessions (game_id, started_at, ended_at, duration_secs) VALUES (?1, ?2, ?3, ?4)",
            params![game_id, started_at, ended_at, duration],
        )?;
        let id = tx.last_insert_rowid();
        tx.execute(
            "UPDATE games SET playtime_secs = playtime_secs + ?1, \
             last_played = MAX(COALESCE(last_played, 0), ?2) WHERE id = ?3",
            params![duration, ended_at, game_id],
        )?;
        tx.commit()?;
        Ok(Session {
            id,
            game_id,
            started_at,
            ended_at,
            duration_secs: duration,
        })
    }

    pub fn sessions(&self, game_id: i64) -> Result<Vec<Session>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, game_id, started_at, ended_at, duration_secs FROM sessions WHERE game_id = ?1 ORDER BY started_at DESC",
        )?;
        let rows = stmt
            .query_map([game_id], |r| {
                Ok(Session {
                    id: r.get(0)?,
                    game_id: r.get(1)?,
                    started_at: r.get(2)?,
                    ended_at: r.get(3)?,
                    duration_secs: r.get(4)?,
                })
            })?
            .collect::<rusqlite::Result<_>>()?;
        Ok(rows)
    }

    // ---- tags --------------------------------------------------------------

    fn tags_of(&self, game_id: i64) -> Result<Vec<String>> {
        let mut stmt = self.conn.prepare(
            "SELECT t.name FROM game_tags gt JOIN tags t ON t.id = gt.tag_id WHERE gt.game_id = ?1 ORDER BY t.name COLLATE NOCASE",
        )?;
        let rows = stmt
            .query_map([game_id], |r| r.get(0))?
            .collect::<rusqlite::Result<_>>()?;
        Ok(rows)
    }

    pub fn tags(&self) -> Result<Vec<String>> {
        let mut stmt = self.conn.prepare(
            "SELECT name FROM tags WHERE id IN (SELECT tag_id FROM game_tags) ORDER BY name COLLATE NOCASE",
        )?;
        let rows = stmt
            .query_map([], |r| r.get(0))?
            .collect::<rusqlite::Result<_>>()?;
        Ok(rows)
    }

    pub fn add_tag(&self, game_id: i64, tag: &str) -> Result<Game> {
        let tag = tag.trim();
        if tag.is_empty() {
            return Err(Error::Other("Leerer Tag".into()));
        }
        self.conn
            .execute("INSERT OR IGNORE INTO tags (name) VALUES (?1)", [tag])?;
        self.conn.execute(
            "INSERT OR IGNORE INTO game_tags (game_id, tag_id) SELECT ?1, id FROM tags WHERE name = ?2",
            params![game_id, tag],
        )?;
        self.game(game_id)
    }

    pub fn remove_tag(&self, game_id: i64, tag: &str) -> Result<Game> {
        self.conn.execute(
            "DELETE FROM game_tags WHERE game_id = ?1 AND tag_id IN (SELECT id FROM tags WHERE name = ?2)",
            params![game_id, tag],
        )?;
        self.game(game_id)
    }

    // ---- collections -------------------------------------------------------

    fn collections_of(&self, game_id: i64) -> Result<Vec<i64>> {
        let mut stmt = self
            .conn
            .prepare("SELECT collection_id FROM collection_games WHERE game_id = ?1")?;
        let rows = stmt
            .query_map([game_id], |r| r.get(0))?
            .collect::<rusqlite::Result<_>>()?;
        Ok(rows)
    }

    pub fn collections(&self) -> Result<Vec<Collection>> {
        let mut stmt = self.conn.prepare(
            "SELECT c.id, c.name, (SELECT COUNT(*) FROM collection_games cg WHERE cg.collection_id = c.id) \
             FROM collections c ORDER BY c.position, c.id",
        )?;
        let rows = stmt
            .query_map([], |r| {
                Ok(Collection {
                    id: r.get(0)?,
                    name: r.get(1)?,
                    game_count: r.get(2)?,
                })
            })?
            .collect::<rusqlite::Result<_>>()?;
        Ok(rows)
    }

    pub fn create_collection(&self, name: &str) -> Result<Collection> {
        let name = name.trim();
        if name.is_empty() {
            return Err(Error::Other("Der Name darf nicht leer sein".into()));
        }
        self.conn.execute(
            "INSERT INTO collections (name, position) VALUES (?1, (SELECT COALESCE(MAX(position), 0) + 1 FROM collections))",
            [name],
        )?;
        Ok(Collection {
            id: self.conn.last_insert_rowid(),
            name: name.into(),
            game_count: 0,
        })
    }

    pub fn rename_collection(&self, id: i64, name: &str) -> Result<()> {
        let name = name.trim();
        if name.is_empty() {
            return Err(Error::Other("Der Name darf nicht leer sein".into()));
        }
        self.conn.execute(
            "UPDATE collections SET name = ?1 WHERE id = ?2",
            params![name, id],
        )?;
        Ok(())
    }

    pub fn delete_collection(&self, id: i64) -> Result<()> {
        self.conn
            .execute("DELETE FROM collections WHERE id = ?1", [id])?;
        Ok(())
    }

    pub fn set_in_collection(
        &self,
        collection_id: i64,
        game_id: i64,
        member: bool,
    ) -> Result<Game> {
        if member {
            self.conn.execute(
                "INSERT OR IGNORE INTO collection_games (collection_id, game_id) VALUES (?1, ?2)",
                params![collection_id, game_id],
            )?;
        } else {
            self.conn.execute(
                "DELETE FROM collection_games WHERE collection_id = ?1 AND game_id = ?2",
                params![collection_id, game_id],
            )?;
        }
        self.game(game_id)
    }

    // ---- saved views -------------------------------------------------------

    pub fn views(&self) -> Result<Vec<SavedView>> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, name, definition FROM views ORDER BY position, id")?;
        let rows: Vec<(i64, String, String)> = stmt
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
            .collect::<rusqlite::Result<_>>()?;
        rows.into_iter()
            .map(|(id, name, def)| {
                Ok(SavedView {
                    id,
                    name,
                    definition: serde_json::from_str(&def)?,
                })
            })
            .collect()
    }

    /// Creates a view (`id = None`) or overwrites an existing one.
    pub fn save_view(
        &self,
        id: Option<i64>,
        name: &str,
        def: &ViewDefinition,
    ) -> Result<SavedView> {
        let name = name.trim();
        if name.is_empty() {
            return Err(Error::Other("Der Name darf nicht leer sein".into()));
        }
        let json = serde_json::to_string(def)?;
        let id = match id {
            Some(id) => {
                self.conn.execute(
                    "UPDATE views SET name = ?1, definition = ?2 WHERE id = ?3",
                    params![name, json, id],
                )?;
                id
            }
            None => {
                self.conn.execute(
                    "INSERT INTO views (name, definition, position) VALUES (?1, ?2, (SELECT COALESCE(MAX(position), 0) + 1 FROM views))",
                    params![name, json],
                )?;
                self.conn.last_insert_rowid()
            }
        };
        Ok(SavedView {
            id,
            name: name.into(),
            definition: def.clone(),
        })
    }

    pub fn delete_view(&self, id: i64) -> Result<()> {
        self.conn.execute("DELETE FROM views WHERE id = ?1", [id])?;
        Ok(())
    }

    // ---- boards --------------------------------------------------------------

    fn game_ids(&self) -> Result<HashSet<i64>> {
        let mut stmt = self.conn.prepare("SELECT id FROM games")?;
        let ids = stmt
            .query_map([], |r| r.get(0))?
            .collect::<rusqlite::Result<_>>()?;
        Ok(ids)
    }

    /// All boards. Games deleted since a board was saved are left out.
    pub fn boards(&self) -> Result<Vec<Board>> {
        let ids = self.game_ids()?;
        let mut stmt = self
            .conn
            .prepare("SELECT id, name, layout FROM boards ORDER BY position, id")?;
        let rows: Vec<(i64, String, String)> = stmt
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
            .collect::<rusqlite::Result<_>>()?;
        rows.into_iter()
            .map(|(id, name, layout)| {
                // A broken layout must not hide the board: show it empty.
                let layout: BoardLayout = serde_json::from_str(&layout).unwrap_or_default();
                Ok(Board {
                    id,
                    name,
                    fields: normalize_fields(layout.fields, |g| ids.contains(&g)),
                })
            })
            .collect()
    }

    pub fn board(&self, id: i64) -> Result<Board> {
        self.boards()?
            .into_iter()
            .find(|b| b.id == id)
            .ok_or_else(|| Error::Other("Board nicht gefunden".into()))
    }

    /// A new board with one empty field, so there is something to fill.
    pub fn create_board(&self, name: &str) -> Result<Board> {
        let name = name.trim();
        if name.is_empty() {
            return Err(Error::Other("Der Name darf nicht leer sein".into()));
        }
        let layout = BoardLayout {
            fields: vec![BoardField {
                id: "f0".into(),
                title: "Neues Feld".into(),
                ..Default::default()
            }],
        };
        self.conn.execute(
            "INSERT INTO boards (name, layout, position) VALUES (?1, ?2, (SELECT COALESCE(MAX(position), 0) + 1 FROM boards))",
            params![name, serde_json::to_string(&layout)?],
        )?;
        self.board(self.conn.last_insert_rowid())
    }

    /// Saves name and fields of an existing board (as cleaned up by
    /// [`normalize_fields`]) and returns what was stored.
    pub fn save_board(&self, board: &Board) -> Result<Board> {
        let name = board.name.trim();
        if name.is_empty() {
            return Err(Error::Other("Der Name darf nicht leer sein".into()));
        }
        let ids = self.game_ids()?;
        let layout = BoardLayout {
            fields: normalize_fields(board.fields.clone(), |g| ids.contains(&g)),
        };
        let changed = self.conn.execute(
            "UPDATE boards SET name = ?1, layout = ?2 WHERE id = ?3",
            params![name, serde_json::to_string(&layout)?, board.id],
        )?;
        if changed == 0 {
            return Err(Error::Other("Board nicht gefunden".into()));
        }
        self.board(board.id)
    }

    pub fn delete_board(&self, id: i64) -> Result<()> {
        self.conn
            .execute("DELETE FROM boards WHERE id = ?1", [id])?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::query::{GameQuery, SortField};

    fn new_game(name: &str) -> NewGame {
        NewGame {
            name: name.into(),
            platform: Some(Platform::Steam),
            platform_id: Some("1".into()),
            shortcut_path: format!("C:/lib/{name}.url"),
            ..Default::default()
        }
    }

    #[test]
    fn connector_games() {
        let lib = Library::open_in_memory().unwrap();
        let riot = lib
            .add_game(&NewGame {
                name: "VALORANT".into(),
                platform: Some(Platform::Riot),
                platform_id: Some("valorant".into()),
                launch_target: Some("C:/Riot Games/Riot Client/RiotClientServices.exe".into()),
                launch_args: Some("--launch-product=valorant --launch-patchline=live".into()),
                source: GameSource::Connector,
                ..Default::default()
            })
            .unwrap();
        assert_eq!(riot.source, GameSource::Connector);
        assert!(riot.installed && riot.shortcut_path.is_empty());
        assert!(riot.launch_args.unwrap().contains("valorant"));
        lib.set_installed(riot.id, false).unwrap();
        assert!(!lib.game(riot.id).unwrap().installed);

        // A desktop import without store data gets it filled in, nothing overwritten.
        let desk = lib
            .add_game(&NewGame {
                name: "Diablo IV".into(),
                shortcut_path: "C:/lib/Diablo IV.lnk".into(),
                install_dir: Some("D:/Games/Diablo IV".into()),
                ..Default::default()
            })
            .unwrap();
        let filled = lib
            .fill_store_info(
                desk.id,
                Platform::Battlenet,
                "Fen",
                Some("C:/other"),
                Some("Diablo IV.exe"),
            )
            .unwrap();
        assert_eq!(filled.platform, Platform::Battlenet);
        assert_eq!(filled.platform_id.as_deref(), Some("Fen"));
        assert_eq!(filled.install_dir.as_deref(), Some("D:/Games/Diablo IV"));
        assert_eq!(filled.process_exe.as_deref(), Some("Diablo IV.exe"));
        assert_eq!(filled.source, GameSource::Desktop);
    }

    #[test]
    fn migrates_a_v1_database() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(&format!("{} PRAGMA user_version = 1;", MIGRATIONS[0]))
            .unwrap();
        conn.execute(
            "INSERT INTO games (name, platform, shortcut_path, added_at) VALUES ('Old', 'steam', 'x.url', 1)",
            [],
        )
        .unwrap();
        let lib = Library::init(conn).unwrap();
        let g = &lib.games().unwrap()[0];
        assert_eq!(g.source, GameSource::Desktop);
        assert!(g.installed && g.launch_args.is_none());
    }

    #[test]
    fn crud_and_relations() {
        let lib = Library::open_in_memory().unwrap();
        let g = lib.add_game(&new_game("Elden Ring")).unwrap();
        assert_eq!(g.platform, Platform::Steam);
        assert_eq!(g.play_count, 0);

        let g = lib
            .update_game(
                g.id,
                &GamePatch {
                    favorite: Some(true),
                    genre: Some(Some("RPG".into())),
                    ..Default::default()
                },
            )
            .unwrap();
        assert!(g.favorite);
        assert_eq!(g.genre.as_deref(), Some("RPG"));
        assert!(
            lib.update_game(
                g.id,
                &GamePatch {
                    name: Some("  ".into()),
                    ..Default::default()
                }
            )
            .is_err()
        );

        let g = lib.add_tag(g.id, "Souls").unwrap();
        let g = lib.add_tag(g.id, "souls").unwrap(); // case-insensitive duplicate
        assert_eq!(g.tags, vec!["Souls".to_string()]);
        assert_eq!(lib.tags().unwrap(), vec!["Souls".to_string()]);

        let c = lib.create_collection("Mit Freunden").unwrap();
        let g = lib.set_in_collection(c.id, g.id, true).unwrap();
        assert_eq!(g.collections, vec![c.id]);
        assert_eq!(lib.collections().unwrap()[0].game_count, 1);

        let all = lib.games().unwrap();
        assert_eq!(all[0].tags, vec!["Souls".to_string()]);
        assert_eq!(all[0].collections, vec![c.id]);

        let g = lib.remove_tag(g.id, "Souls").unwrap();
        assert!(g.tags.is_empty());
        lib.delete_collection(c.id).unwrap();
        assert!(lib.game(g.id).unwrap().collections.is_empty());
        lib.delete_game(g.id).unwrap();
        assert!(lib.game(g.id).is_err());
    }

    #[test]
    fn sessions_update_playtime() {
        let lib = Library::open_in_memory().unwrap();
        let g = lib.add_game(&new_game("Hades II")).unwrap();
        lib.record_launch(g.id, 1_000).unwrap();
        lib.add_session(g.id, 1_000, 4_600).unwrap();
        lib.add_session(g.id, 10_000, 10_600).unwrap();
        let g = lib.game(g.id).unwrap();
        assert_eq!(g.play_count, 1);
        assert_eq!(g.playtime_secs, 4_200);
        assert_eq!(g.last_played, Some(10_600));
        let s = lib.sessions(g.id).unwrap();
        assert_eq!(s.len(), 2);
        assert_eq!(s[0].started_at, 10_000);
    }

    #[test]
    fn saved_views_roundtrip() {
        let lib = Library::open_in_memory().unwrap();
        let def = ViewDefinition {
            query: GameQuery {
                platforms: vec![Platform::Steam],
                sort: SortField::Playtime,
                descending: true,
                ..Default::default()
            },
            ..Default::default()
        };
        let v = lib.save_view(None, "Steam nach Spielzeit", &def).unwrap();
        let v2 = lib.save_view(Some(v.id), "Steam", &def).unwrap();
        assert_eq!(v.id, v2.id);
        let all = lib.views().unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].name, "Steam");
        assert_eq!(all[0].definition, def);
        lib.delete_view(v.id).unwrap();
        assert!(lib.views().unwrap().is_empty());
    }

    #[test]
    fn reopening_keeps_data() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("library.db");
        {
            let lib = Library::open(&path).unwrap();
            lib.add_game(&new_game("Valheim")).unwrap();
        }
        let lib = Library::open(&path).unwrap();
        assert_eq!(lib.games().unwrap().len(), 1);
    }

    #[test]
    fn boards_round_trip() {
        use crate::boards::BoardSlot;
        let lib = Library::open_in_memory().unwrap();
        let a = lib.add_game(&new_game("Elden Ring")).unwrap();
        let b = lib.add_game(&new_game("Hades II")).unwrap();
        let mut board = lib.create_board(" Mein Game Space ").unwrap();
        assert_eq!(board.name, "Mein Game Space");
        assert_eq!(board.fields.len(), 1);
        board.fields[0].title = "Gerade dran".into();
        board.fields[0].slots = vec![
            BoardSlot {
                col: 0,
                row: 0,
                game_id: a.id,
            },
            BoardSlot {
                col: 2,
                row: 0,
                game_id: b.id,
            },
        ];
        board.fields.push(BoardField {
            id: "f1".into(),
            title: "Story".into(),
            y: 2,
            slots: vec![BoardSlot {
                col: 1,
                row: 0,
                game_id: a.id,
            }],
            ..Default::default()
        });
        let saved = lib.save_board(&board).unwrap();
        assert_eq!(saved, board);
        assert_eq!(lib.boards().unwrap(), vec![board.clone()]);

        // Deleting a game leaves a gap, nothing else moves.
        lib.delete_game(a.id).unwrap();
        let after = lib.board(board.id).unwrap();
        assert_eq!(
            after.fields[0].slots,
            vec![BoardSlot {
                col: 2,
                row: 0,
                game_id: b.id
            }]
        );
        assert!(after.fields[1].slots.is_empty());

        lib.delete_board(board.id).unwrap();
        assert!(lib.boards().unwrap().is_empty());
        assert!(lib.save_board(&board).is_err());
    }
}
