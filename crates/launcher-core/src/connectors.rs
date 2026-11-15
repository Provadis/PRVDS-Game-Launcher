//! Reads the installed games straight from the store launchers, without any
//! desktop shortcut: Steam, Epic, Ubisoft Connect, EA App, Battle.net, Riot.
//!
//! Every launcher is read from its own files or registry keys, never through
//! a login. Parsing is split from the file system access so it can be tested
//! on any OS; the registry parts only run on Windows.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::locate::{epic_manifest_dir, riot_product_info, steam_roots, vdf_values};
use crate::model::{Game, Platform};
use crate::util::{file_name_of, normalize_name};

/// How a connector game is started.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Launch {
    /// A store URL such as `steam://rungameid/570`.
    Url { url: String },
    /// An executable with arguments.
    Exe { path: String, args: String },
}

/// A game found in a launcher's install data.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledGame {
    pub platform: Platform,
    /// Store id (Steam AppID, Epic app name, Ubisoft id, Riot product, ...).
    pub platform_id: String,
    pub name: String,
    pub install_dir: Option<String>,
    pub launch: Launch,
    /// Game process for the playtime tracker, if known.
    pub process_exe: Option<String>,
}

impl InstalledGame {
    /// `(launch_target, launch_args)` as stored in the library.
    pub fn launch_parts(&self) -> (String, Option<String>) {
        match &self.launch {
            Launch::Url { url } => (url.clone(), None),
            Launch::Exe { path, args } => (
                path.clone(),
                (!args.trim().is_empty()).then(|| args.clone()),
            ),
        }
    }
}

/// The launchers this module can read.
pub const CONNECTOR_PLATFORMS: [Platform; 6] = [
    Platform::Steam,
    Platform::Epic,
    Platform::Ubisoft,
    Platform::Ea,
    Platform::Battlenet,
    Platform::Riot,
];

/// All installed games of the given launchers, sorted by name.
pub fn scan(platforms: &[Platform]) -> Vec<InstalledGame> {
    let want = |p: Platform| platforms.contains(&p);
    let mut out = Vec::new();
    if want(Platform::Steam) {
        out.extend(steam_games(&steam_roots()));
    }
    if want(Platform::Epic)
        && let Some(dir) = epic_manifest_dir()
    {
        out.extend(epic_games(&dir));
    }
    if want(Platform::Riot) {
        out.extend(riot_games(&program_data().join("Riot Games")));
    }
    let uninstall = if want(Platform::Ea) || want(Platform::Battlenet) {
        os::uninstall_entries()
    } else {
        Vec::new()
    };
    if want(Platform::Ubisoft) {
        out.extend(os::ubisoft_games());
    }
    if want(Platform::Ea) {
        out.extend(ea_games(&uninstall));
    }
    if want(Platform::Battlenet) {
        out.extend(battlenet_games(&uninstall));
    }
    // One entry per store id.
    let mut seen = HashSet::new();
    out.retain(|g| seen.insert((g.platform, g.platform_id.to_lowercase())));
    out.sort_by_key(|g| g.name.to_lowercase());
    out
}

fn program_data() -> PathBuf {
    std::env::var_os("ProgramData")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"C:\ProgramData"))
}

// ---- Steam --------------------------------------------------------------------

/// Runtimes and tools Steam installs like games.
const STEAM_TOOL_PREFIXES: &[&str] = &[
    "proton",
    "steam linux runtime",
    "steamworks common redistributables",
    "steamvr",
    "steam audio",
];

pub fn steam_games(roots: &[PathBuf]) -> Vec<InstalledGame> {
    let mut libraries: Vec<PathBuf> = Vec::new();
    for root in roots {
        libraries.push(root.clone());
        if let Ok(text) = std::fs::read_to_string(root.join("steamapps").join("libraryfolders.vdf"))
        {
            libraries.extend(vdf_values(&text, "path").into_iter().map(PathBuf::from));
        }
    }
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for lib in libraries {
        let steamapps = lib.join("steamapps");
        let Ok(dir) = std::fs::read_dir(&steamapps) else {
            continue;
        };
        for entry in dir.flatten() {
            let name = entry.file_name().to_string_lossy().to_lowercase();
            if !(name.starts_with("appmanifest_") && name.ends_with(".acf")) {
                continue;
            }
            if let Ok(text) = std::fs::read_to_string(entry.path())
                && let Some(g) = parse_appmanifest(&text, &steamapps)
                && seen.insert(g.platform_id.clone())
            {
                out.push(g);
            }
        }
    }
    out
}

/// One `appmanifest_<id>.acf`. Only fully installed games, no tools.
pub fn parse_appmanifest(text: &str, steamapps: &Path) -> Option<InstalledGame> {
    let first = |key: &str| vdf_values(text, key).into_iter().next();
    let appid = first("appid")?;
    let name = first("name")?;
    let installdir = first("installdir")?;
    // StateFlags bit 4 = fully installed.
    let flags: u32 = first("StateFlags")
        .and_then(|f| f.parse().ok())
        .unwrap_or(4);
    let lower = name.to_lowercase();
    if flags & 4 == 0
        || appid == "228980"
        || STEAM_TOOL_PREFIXES.iter().any(|p| lower.starts_with(p))
    {
        return None;
    }
    Some(InstalledGame {
        platform: Platform::Steam,
        launch: Launch::Url {
            url: format!("steam://rungameid/{appid}"),
        },
        platform_id: appid,
        name,
        install_dir: Some(
            steamapps
                .join("common")
                .join(installdir)
                .to_string_lossy()
                .into_owned(),
        ),
        process_exe: None,
    })
}

// ---- Epic ---------------------------------------------------------------------

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct EpicItem {
    #[serde(default)]
    app_name: String,
    #[serde(default)]
    display_name: String,
    #[serde(default)]
    install_location: String,
    #[serde(default)]
    launch_executable: String,
    #[serde(default)]
    catalog_namespace: String,
    #[serde(default)]
    catalog_item_id: String,
    #[serde(default)]
    main_game_app_name: String,
    #[serde(default)]
    app_categories: Vec<String>,
    #[serde(default, rename = "bIsIncompleteInstall")]
    incomplete: bool,
}

pub fn epic_games(manifest_dir: &Path) -> Vec<InstalledGame> {
    let Ok(dir) = std::fs::read_dir(manifest_dir) else {
        return Vec::new();
    };
    dir.flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x == "item"))
        .filter_map(|e| std::fs::read(e.path()).ok())
        .filter_map(|bytes| parse_epic_item(&bytes))
        .collect()
}

/// One `.item` manifest. Skips DLCs, engines/tools and broken installs.
pub fn parse_epic_item(bytes: &[u8]) -> Option<InstalledGame> {
    let m: EpicItem = serde_json::from_slice(bytes).ok()?;
    let is_game = m.app_categories.is_empty() || m.app_categories.iter().any(|c| c == "games");
    let is_dlc = !m.main_game_app_name.is_empty() && m.main_game_app_name != m.app_name;
    if m.incomplete || !is_game || is_dlc || m.app_name.is_empty() || m.install_location.is_empty()
    {
        return None;
    }
    let id = format!(
        "{}:{}:{}",
        m.catalog_namespace, m.catalog_item_id, m.app_name
    );
    let url = format!(
        "com.epicgames.launcher://apps/{}?action=launch&silent=true",
        id.replace(':', "%3A")
    );
    Some(InstalledGame {
        platform: Platform::Epic,
        platform_id: id,
        name: if m.display_name.is_empty() {
            m.app_name.clone()
        } else {
            m.display_name
        },
        install_dir: Some(m.install_location),
        launch: Launch::Url { url },
        process_exe: (!m.launch_executable.is_empty())
            .then(|| file_name_of(&m.launch_executable).to_string()),
    })
}

// ---- Uninstall registry (EA, Battle.net) --------------------------------------

/// One "Programs and Features" entry.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UninstallEntry {
    pub name: String,
    pub publisher: String,
    pub install_location: String,
}

// ---- EA -----------------------------------------------------------------------

/// EA games ship `__Installer\installerdata.xml`, which names the game exe.
pub fn ea_games(entries: &[UninstallEntry]) -> Vec<InstalledGame> {
    entries
        .iter()
        .filter(|e| e.publisher.to_lowercase().contains("electronic arts"))
        .filter_map(|e| {
            let dir = Path::new(&e.install_location);
            let xml =
                std::fs::read_to_string(dir.join("__Installer").join("installerdata.xml")).ok()?;
            let exe = ea_exe_from_installerdata(&xml)?;
            let path = dir.join(&exe);
            Some(InstalledGame {
                platform: Platform::Ea,
                platform_id: ea_content_id(&xml).unwrap_or_else(|| normalize_name(&e.name)),
                name: e.name.clone(),
                install_dir: Some(e.install_location.clone()),
                process_exe: Some(file_name_of(&exe).to_string()),
                launch: Launch::Exe {
                    path: path.to_string_lossy().into_owned(),
                    args: String::new(),
                },
            })
        })
        .collect()
}

/// `<filePath>[HKEY_LOCAL_MACHINE\SOFTWARE\...\Install Dir]bf6.exe</filePath>` → `bf6.exe`.
pub fn ea_exe_from_installerdata(xml: &str) -> Option<String> {
    let mut rest = xml;
    while let Some(start) = rest.find("<filePath>") {
        let after = &rest[start + "<filePath>".len()..];
        let end = after.find("</filePath>")?;
        let value = after[..end].trim();
        let exe = value.rsplit(']').next().unwrap_or(value).trim();
        let exe = exe.trim_start_matches(['\\', '/']);
        if exe.to_lowercase().ends_with(".exe") && !exe.to_lowercase().contains("installer") {
            return Some(exe.to_string());
        }
        rest = &after[end..];
    }
    None
}

fn ea_content_id(xml: &str) -> Option<String> {
    let start = xml.find("<contentID>")? + "<contentID>".len();
    let end = xml[start..].find("</contentID>")? + start;
    let id = xml[start..end].trim();
    (!id.is_empty()).then(|| id.to_string())
}

// ---- Battle.net ---------------------------------------------------------------

/// Battle.net launch codes (`battlenet://<code>`) of Blizzard's games.
pub fn battlenet_code(name: &str) -> Option<&'static str> {
    let n = normalize_name(name);
    let codes: &[(&str, &str)] = &[
        ("diablo iv", "Fen"),
        ("diablo iii", "D3"),
        ("diablo ii resurrected", "OSI"),
        ("world of warcraft", "WoW"),
        ("overwatch", "Pro"),
        ("overwatch 2", "Pro"),
        ("hearthstone", "WTCG"),
        ("heroes of the storm", "Hero"),
        ("starcraft ii", "S2"),
        ("starcraft", "S1"),
        ("warcraft iii reforged", "W3"),
    ];
    codes
        .iter()
        .find(|(k, _)| normalize_name(k) == n)
        .map(|(_, c)| *c)
}

pub fn battlenet_games(entries: &[UninstallEntry]) -> Vec<InstalledGame> {
    entries
        .iter()
        .filter(|e| e.publisher.to_lowercase().contains("blizzard"))
        .filter(|e| !e.name.to_lowercase().starts_with("battle.net"))
        .filter_map(|e| {
            let dir = Path::new(&e.install_location);
            let launcher = dir.join(format!("{} Launcher.exe", e.name));
            let launch = match battlenet_code(&e.name) {
                Some(code) => Launch::Url {
                    url: format!("battlenet://{code}"),
                },
                None if launcher.is_file() => Launch::Exe {
                    path: launcher.to_string_lossy().into_owned(),
                    args: String::new(),
                },
                None => return None,
            };
            Some(InstalledGame {
                platform: Platform::Battlenet,
                platform_id: battlenet_code(&e.name)
                    .map(str::to_string)
                    .unwrap_or_else(|| normalize_name(&e.name)),
                name: e.name.clone(),
                install_dir: Some(e.install_location.clone()),
                launch,
                process_exe: None,
            })
        })
        .collect()
}

// ---- Riot ---------------------------------------------------------------------

/// `ProgramData\Riot Games`: `Metadata\<product>.live\<product>.live.product_settings.yaml`
/// says where a game lives, `RiotClientInstalls.json` where the Riot Client is.
pub fn riot_games(riot_data: &Path) -> Vec<InstalledGame> {
    let Some(client) = std::fs::read_to_string(riot_data.join("RiotClientInstalls.json"))
        .ok()
        .and_then(|t| riot_client_path(&t))
    else {
        return Vec::new();
    };
    let Ok(dir) = std::fs::read_dir(riot_data.join("Metadata")) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for entry in dir.flatten() {
        let folder = entry.file_name().to_string_lossy().into_owned();
        let Some((product, patchline)) = folder.split_once('.') else {
            continue;
        };
        if product.eq_ignore_ascii_case("riot_client") {
            continue;
        }
        let settings = entry.path().join(format!("{folder}.product_settings.yaml"));
        let Some(install) = std::fs::read_to_string(settings)
            .ok()
            .and_then(|t| riot_install_path(&t))
        else {
            continue;
        };
        if !Path::new(&install).exists() {
            continue;
        }
        out.push(InstalledGame {
            platform: Platform::Riot,
            platform_id: product.to_string(),
            name: riot_name(product),
            install_dir: Some(install),
            process_exe: riot_product_info(product).map(|(_, exe)| exe.to_string()),
            launch: Launch::Exe {
                path: client.clone(),
                args: format!("--launch-product={product} --launch-patchline={patchline}"),
            },
        });
    }
    out
}

/// `"rc_default": "C:/Riot Games/Riot Client/RiotClientServices.exe"`.
pub fn riot_client_path(json: &str) -> Option<String> {
    let v: serde_json::Value = serde_json::from_str(json).ok()?;
    ["rc_default", "rc_live"]
        .iter()
        .find_map(|k| v[*k].as_str())
        .map(native_path)
}

/// Riot writes paths with forward slashes; show them the Windows way.
fn native_path(p: &str) -> String {
    if cfg!(windows) {
        p.replace('/', "\\")
    } else {
        p.to_string()
    }
}

/// `product_install_full_path: "C:/Riot Games/VALORANT/live"`.
pub fn riot_install_path(yaml: &str) -> Option<String> {
    yaml.lines().find_map(|line| {
        let rest = line.trim().strip_prefix("product_install_full_path:")?;
        let p = rest.trim().trim_matches('"').trim_matches('\'');
        (!p.is_empty()).then(|| native_path(p))
    })
}

fn riot_name(product: &str) -> String {
    match product.to_ascii_lowercase().as_str() {
        "valorant" => "VALORANT".into(),
        "league_of_legends" => "League of Legends".into(),
        "bacon" => "Legends of Runeterra".into(),
        other => other
            .split('_')
            .map(|w| {
                let mut c = w.chars();
                c.next()
                    .map(|f| f.to_uppercase().collect::<String>() + c.as_str())
                    .unwrap_or_default()
            })
            .collect::<Vec<_>>()
            .join(" "),
    }
}

// ---- Matching with the library ------------------------------------------------

/// The library game a found game belongs to: same store id, or (for desktop
/// imports without id) same platform and name.
pub fn match_existing<'a>(found: &InstalledGame, games: &'a [Game]) -> Option<&'a Game> {
    games
        .iter()
        .find(|g| {
            g.platform == found.platform
                && g.platform_id
                    .as_deref()
                    .is_some_and(|id| id.eq_ignore_ascii_case(&found.platform_id))
        })
        .or_else(|| {
            let wanted = normalize_name(&found.name);
            games.iter().find(|g| {
                (g.platform == found.platform || g.platform == Platform::Other)
                    && normalize_name(&g.name) == wanted
            })
        })
}

// ---- Windows registry -------------------------------------------------------

#[cfg(windows)]
mod os {
    use super::{InstalledGame, Launch, UninstallEntry};
    use crate::model::Platform;
    use winreg::RegKey;
    use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};

    pub fn uninstall_entries() -> Vec<UninstallEntry> {
        let roots = [
            (
                HKEY_LOCAL_MACHINE,
                r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall",
            ),
            (
                HKEY_LOCAL_MACHINE,
                r"SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall",
            ),
            (
                HKEY_CURRENT_USER,
                r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall",
            ),
        ];
        let mut out = Vec::new();
        for (hive, path) in roots {
            let Ok(key) = RegKey::predef(hive).open_subkey(path) else {
                continue;
            };
            for sub in key.enum_keys().flatten() {
                let Ok(app) = key.open_subkey(&sub) else {
                    continue;
                };
                let get = |v: &str| {
                    app.get_value::<String, _>(v)
                        .map(|s| s.trim().trim_matches('"').to_string())
                        .unwrap_or_default()
                };
                let entry = UninstallEntry {
                    name: get("DisplayName"),
                    publisher: get("Publisher"),
                    install_location: get("InstallLocation"),
                };
                if !entry.name.is_empty() && !entry.install_location.is_empty() {
                    out.push(entry);
                }
            }
        }
        out
    }

    pub fn ubisoft_games() -> Vec<InstalledGame> {
        let Ok(installs) = RegKey::predef(HKEY_LOCAL_MACHINE)
            .open_subkey(r"SOFTWARE\WOW6432Node\Ubisoft\Launcher\Installs")
        else {
            return Vec::new();
        };
        installs
            .enum_keys()
            .flatten()
            .filter_map(|id| {
                let key = installs.open_subkey(&id).ok()?;
                let dir: String = key.get_value("InstallDir").ok()?;
                let dir = dir.replace('/', "\\");
                let dir = dir.trim_end_matches('\\').to_string();
                if !std::path::Path::new(&dir).exists() {
                    return None;
                }
                let name = dir.rsplit('\\').next().unwrap_or(&dir).to_string();
                Some(InstalledGame {
                    platform: Platform::Ubisoft,
                    launch: Launch::Url {
                        url: format!("uplay://launch/{id}/0"),
                    },
                    platform_id: id,
                    name,
                    install_dir: Some(dir),
                    process_exe: None,
                })
            })
            .collect()
    }
}

#[cfg(not(windows))]
mod os {
    use super::{InstalledGame, UninstallEntry};

    pub fn uninstall_entries() -> Vec<UninstallEntry> {
        Vec::new()
    }

    pub fn ubisoft_games() -> Vec<InstalledGame> {
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ACF: &str = r#""AppState"
{
	"appid"		"1086940"
	"Universe"		"1"
	"name"		"Baldur's Gate 3"
	"StateFlags"		"4"
	"installdir"		"Baldurs Gate 3"
}"#;

    #[test]
    fn steam_manifests() {
        let steamapps = Path::new("D:/SteamLibrary/steamapps");
        let g = parse_appmanifest(ACF, steamapps).unwrap();
        assert_eq!(g.platform_id, "1086940");
        assert_eq!(g.name, "Baldur's Gate 3");
        assert_eq!(
            g.launch,
            Launch::Url {
                url: "steam://rungameid/1086940".into()
            }
        );
        assert!(g.install_dir.unwrap().ends_with("Baldurs Gate 3"));

        let downloading = ACF.replace("\"StateFlags\"\t\t\"4\"", "\"StateFlags\"\t\t\"1026\"");
        assert!(parse_appmanifest(&downloading, steamapps).is_none());
        let tool = ACF
            .replace("1086940", "1493710")
            .replace("Baldur's Gate 3", "Proton Experimental");
        assert!(parse_appmanifest(&tool, steamapps).is_none());
    }

    #[test]
    fn steam_libraries_on_disk() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("Steam");
        let lib2 = tmp.path().join("Lib2");
        std::fs::create_dir_all(root.join("steamapps")).unwrap();
        std::fs::create_dir_all(lib2.join("steamapps")).unwrap();
        std::fs::write(
            root.join("steamapps/libraryfolders.vdf"),
            format!(
                "\"libraryfolders\"\n{{\n\t\"0\"\n\t{{\n\t\t\"path\"\t\t\"{}\"\n\t}}\n}}\n",
                // Steam escapes backslashes in its VDF files.
                lib2.display().to_string().replace('\\', "\\\\")
            ),
        )
        .unwrap();
        std::fs::write(lib2.join("steamapps/appmanifest_1086940.acf"), ACF).unwrap();
        let games = steam_games(&[root]);
        assert_eq!(games.len(), 1);
        assert_eq!(games[0].name, "Baldur's Gate 3");
    }

    #[test]
    fn epic_items() {
        let item = serde_json::json!({
            "AppName": "Fortnite",
            "DisplayName": "Fortnite",
            "InstallLocation": "C:\\Program Files\\Epic Games\\Fortnite",
            "LaunchExecutable": "FortniteGame/Binaries/Win64/FortniteLauncher.exe",
            "CatalogNamespace": "fn",
            "CatalogItemId": "4fe75bbc5a674f4f9b356b5c90567da5",
            "AppCategories": ["public", "games", "applications"],
            "bIsIncompleteInstall": false
        });
        let g = parse_epic_item(item.to_string().as_bytes()).unwrap();
        assert_eq!(
            g.platform_id,
            "fn:4fe75bbc5a674f4f9b356b5c90567da5:Fortnite"
        );
        assert_eq!(
            g.launch,
            Launch::Url {
                url: "com.epicgames.launcher://apps/fn%3A4fe75bbc5a674f4f9b356b5c90567da5%3AFortnite?action=launch&silent=true".into()
            }
        );
        assert_eq!(g.process_exe.as_deref(), Some("FortniteLauncher.exe"));

        let mut engine = item.clone();
        engine["AppCategories"] = serde_json::json!(["engines"]);
        assert!(parse_epic_item(engine.to_string().as_bytes()).is_none());
        let mut dlc = item.clone();
        dlc["MainGameAppName"] = serde_json::json!("SomethingElse");
        assert!(parse_epic_item(dlc.to_string().as_bytes()).is_none());
        let mut broken = item;
        broken["bIsIncompleteInstall"] = serde_json::json!(true);
        assert!(parse_epic_item(broken.to_string().as_bytes()).is_none());
    }

    #[test]
    fn ea_installerdata() {
        let xml = r#"<game><contentIDs><contentID>196216</contentID></contentIDs>
            <runtime><launcher><filePath>[HKEY_LOCAL_MACHINE\SOFTWARE\EA Games\Battlefield 6\Install Dir]bf6.exe</filePath></launcher></runtime></game>"#;
        assert_eq!(ea_exe_from_installerdata(xml).as_deref(), Some("bf6.exe"));
        assert_eq!(ea_content_id(xml).as_deref(), Some("196216"));
        assert!(ea_exe_from_installerdata("<game/>").is_none());
    }

    #[test]
    fn battlenet_codes() {
        assert_eq!(battlenet_code("Diablo IV"), Some("Fen"));
        assert_eq!(battlenet_code("World of Warcraft"), Some("WoW"));
        assert_eq!(battlenet_code("Overwatch"), Some("Pro"));
        assert_eq!(battlenet_code("Some Other Game"), None);
        let entries = vec![
            UninstallEntry {
                name: "Diablo IV".into(),
                publisher: "Blizzard Entertainment".into(),
                install_location: "C:\\Program Files (x86)\\Diablo IV".into(),
            },
            UninstallEntry {
                name: "Battle.net".into(),
                publisher: "Blizzard Entertainment".into(),
                install_location: "C:\\Program Files (x86)\\Battle.net".into(),
            },
        ];
        let games = battlenet_games(&entries);
        assert_eq!(games.len(), 1);
        assert_eq!(
            games[0].launch,
            Launch::Url {
                url: "battlenet://Fen".into()
            }
        );
    }

    #[test]
    fn riot_on_disk() {
        let tmp = tempfile::tempdir().unwrap();
        let data = tmp.path();
        let game_dir = data.join("VALORANT/live");
        std::fs::create_dir_all(&game_dir).unwrap();
        std::fs::create_dir_all(data.join("Metadata/valorant.live")).unwrap();
        std::fs::create_dir_all(data.join("Metadata/riot_client.live")).unwrap();
        std::fs::write(
            data.join("RiotClientInstalls.json"),
            r#"{"rc_default": "C:/Riot Games/Riot Client/RiotClientServices.exe"}"#,
        )
        .unwrap();
        std::fs::write(
            data.join("Metadata/valorant.live/valorant.live.product_settings.yaml"),
            format!(
                "product_install_full_path: \"{}\"\nproduct_install_root: \"C:/Riot Games\"\n",
                game_dir.display()
            ),
        )
        .unwrap();
        let games = riot_games(data);
        assert_eq!(games.len(), 1);
        let g = &games[0];
        assert_eq!(g.name, "VALORANT");
        assert_eq!(
            g.process_exe.as_deref(),
            Some("VALORANT-Win64-Shipping.exe")
        );
        assert_eq!(
            g.launch,
            Launch::Exe {
                path: native_path("C:/Riot Games/Riot Client/RiotClientServices.exe"),
                args: "--launch-product=valorant --launch-patchline=live".into()
            }
        );
        assert_eq!(riot_name("2xko_beta"), "2xko Beta");
    }

    fn game(id: i64, name: &str, platform: Platform, pid: Option<&str>) -> Game {
        Game {
            id,
            name: name.into(),
            platform,
            platform_id: pid.map(Into::into),
            shortcut_path: String::new(),
            original_path: None,
            journal_id: None,
            launch_target: None,
            launch_args: None,
            source: crate::model::GameSource::Desktop,
            installed: true,
            install_dir: None,
            process_exe: None,
            cover_path: None,
            hero_path: None,
            added_at: 0,
            last_played: None,
            play_count: 0,
            playtime_secs: 0,
            favorite: false,
            hidden: false,
            genre: None,
            release_year: None,
            developer: None,
            summary: None,
            tags: vec![],
            collections: vec![],
        }
    }

    #[test]
    fn matches_library_games() {
        let found = parse_appmanifest(ACF, Path::new("x")).unwrap();
        let by_id = vec![game(1, "BG3", Platform::Steam, Some("1086940"))];
        assert_eq!(match_existing(&found, &by_id).map(|g| g.id), Some(1));
        let by_name = vec![game(2, "Baldur's Gate 3", Platform::Other, None)];
        assert_eq!(match_existing(&found, &by_name).map(|g| g.id), Some(2));
        let other = vec![game(3, "Baldur's Gate 3", Platform::Epic, Some("x"))];
        assert!(match_existing(&found, &other).is_none());
    }
}
