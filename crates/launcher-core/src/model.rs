use serde::{Deserialize, Serialize};

/// Store/launcher a game belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord)]
#[serde(rename_all = "lowercase")]
pub enum Platform {
    Steam,
    Epic,
    Ubisoft,
    Ea,
    Battlenet,
    Other,
}

impl Platform {
    pub const ALL: [Platform; 6] = [
        Platform::Steam,
        Platform::Epic,
        Platform::Ubisoft,
        Platform::Ea,
        Platform::Battlenet,
        Platform::Other,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Platform::Steam => "steam",
            Platform::Epic => "epic",
            Platform::Ubisoft => "ubisoft",
            Platform::Ea => "ea",
            Platform::Battlenet => "battlenet",
            Platform::Other => "other",
        }
    }

    pub fn parse(s: &str) -> Platform {
        match s {
            "steam" => Platform::Steam,
            "epic" => Platform::Epic,
            "ubisoft" => Platform::Ubisoft,
            "ea" => Platform::Ea,
            "battlenet" => Platform::Battlenet,
            _ => Platform::Other,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Platform::Steam => "Steam",
            Platform::Epic => "Epic",
            Platform::Ubisoft => "Ubisoft",
            Platform::Ea => "EA",
            Platform::Battlenet => "Battle.net",
            Platform::Other => "Sonstige",
        }
    }
}

/// A game in the library.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Game {
    pub id: i64,
    pub name: String,
    pub platform: Platform,
    /// Store specific id (Steam AppID, Epic app name, Ubisoft id, ...).
    pub platform_id: Option<String>,
    /// Shortcut file inside the library folder that is opened to start the game.
    pub shortcut_path: String,
    /// Where the shortcut lived before it was imported.
    pub original_path: Option<String>,
    /// Import journal entry, used for undo.
    pub journal_id: Option<String>,
    /// Target of a `.lnk` (exe path) or URL of a `.url`.
    pub launch_target: Option<String>,
    pub install_dir: Option<String>,
    /// Exe file name used to detect the running game for playtime tracking.
    pub process_exe: Option<String>,
    pub cover_path: Option<String>,
    pub hero_path: Option<String>,
    pub added_at: i64,
    pub last_played: Option<i64>,
    pub play_count: i64,
    pub playtime_secs: i64,
    pub favorite: bool,
    pub hidden: bool,
    pub genre: Option<String>,
    pub release_year: Option<i32>,
    pub developer: Option<String>,
    pub summary: Option<String>,
    pub tags: Vec<String>,
    pub collections: Vec<i64>,
}

/// Data needed to add a game.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewGame {
    pub name: String,
    pub platform: Option<Platform>,
    pub platform_id: Option<String>,
    pub shortcut_path: String,
    pub original_path: Option<String>,
    pub journal_id: Option<String>,
    pub launch_target: Option<String>,
    pub install_dir: Option<String>,
    pub process_exe: Option<String>,
}

/// Fields the user (or the metadata fetcher) can change. `None` means "keep".
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GamePatch {
    pub name: Option<String>,
    pub favorite: Option<bool>,
    pub hidden: Option<bool>,
    pub install_dir: Option<Option<String>>,
    pub process_exe: Option<Option<String>>,
    pub cover_path: Option<Option<String>>,
    pub hero_path: Option<Option<String>>,
    pub genre: Option<Option<String>>,
    pub release_year: Option<Option<i32>>,
    pub developer: Option<Option<String>>,
    pub summary: Option<Option<String>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Collection {
    pub id: i64,
    pub name: String,
    pub game_count: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Session {
    pub id: i64,
    pub game_id: i64,
    pub started_at: i64,
    pub ended_at: i64,
    pub duration_secs: i64,
}
