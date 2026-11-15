//! Reads shortcut files (`.lnk`, `.url`, `.exe`) from a folder and guesses
//! which store they belong to and whether they are games.

use std::path::{Path, PathBuf};

use percent_encoding::percent_decode_str;
use serde::{Deserialize, Serialize};

use crate::model::Platform;
use crate::util::{file_name_of, title_from_file_stem};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ShortcutKind {
    Lnk,
    Url,
    Exe,
}

/// Raw data read from a shortcut file.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShortcutInfo {
    /// `.lnk`: target path. `.exe`: the file itself. `.url`: the URL.
    pub target: Option<String>,
    pub arguments: Option<String>,
    pub working_dir: Option<String>,
    pub icon: Option<String>,
}

/// A shortcut found on the desktop, ready to be shown in the import wizard.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Candidate {
    pub path: String,
    pub file_name: String,
    pub title: String,
    pub kind: ShortcutKind,
    pub info: ShortcutInfo,
    pub platform: Platform,
    pub platform_id: Option<String>,
    /// Pre-selected in the wizard.
    pub likely_game: bool,
}

/// Scans the given folders (non-recursive) for shortcuts.
pub fn scan_dirs(dirs: &[PathBuf]) -> Vec<Candidate> {
    let mut out = Vec::new();
    for dir in dirs {
        let Ok(entries) = std::fs::read_dir(dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            if let Some(c) = inspect(&path) {
                out.push(c);
            }
        }
    }
    out.sort_by(|a, b| {
        b.likely_game
            .cmp(&a.likely_game)
            .then_with(|| a.title.to_lowercase().cmp(&b.title.to_lowercase()))
    });
    out
}

/// Reads one file and classifies it. Returns `None` for unsupported files.
pub fn inspect(path: &Path) -> Option<Candidate> {
    let ext = path.extension()?.to_str()?.to_ascii_lowercase();
    let kind = match ext.as_str() {
        "lnk" => ShortcutKind::Lnk,
        "url" => ShortcutKind::Url,
        "exe" => ShortcutKind::Exe,
        _ => return None,
    };
    let info = match kind {
        ShortcutKind::Lnk => read_lnk(path).unwrap_or_default(),
        ShortcutKind::Url => read_url(path).unwrap_or_default(),
        ShortcutKind::Exe => ShortcutInfo {
            target: Some(path.to_string_lossy().into_owned()),
            ..Default::default()
        },
    };
    let stem = path.file_stem()?.to_string_lossy();
    let title = title_from_file_stem(&stem);
    let (platform, platform_id) = detect_platform(kind, &info);
    let likely_game = is_likely_game(kind, &info, platform, platform_id.as_deref(), &title);
    Some(Candidate {
        path: path.to_string_lossy().into_owned(),
        file_name: path.file_name()?.to_string_lossy().into_owned(),
        title,
        kind,
        info,
        platform,
        platform_id,
        likely_game,
    })
}

/// Parses a Windows shell link. The `lnk` crate may panic on unusual files,
/// so the call is guarded.
pub fn read_lnk(path: &Path) -> Option<ShortcutInfo> {
    let path = path.to_path_buf();
    std::panic::catch_unwind(move || {
        let link = lnk::ShellLink::open(&path, lnk::encoding::WINDOWS_1252).ok()?;
        let sd = link.string_data();
        let mut target = link.link_target();
        if target.is_none() {
            // Many shortcuts only carry the shell item id list.
            if let Some(list) = link.linktarget_id_list() {
                let items: Vec<&[u8]> =
                    list.id_list().iter().map(|i| i.data().as_slice()).collect();
                target = idlist_path(&items);
            }
        }
        if target.is_none() {
            // Fall back to working dir + relative path.
            if let (Some(wd), Some(rel)) = (sd.working_dir(), sd.relative_path()) {
                let rel = rel.trim_start_matches(".\\").trim_start_matches("./");
                let file = file_name_of(rel);
                target = Some(format!("{}\\{}", wd.trim_end_matches('\\'), file));
            }
        }
        Some(ShortcutInfo {
            target,
            arguments: sd
                .command_line_arguments()
                .clone()
                .filter(|s| !s.is_empty()),
            working_dir: sd.working_dir().clone().filter(|s| !s.is_empty()),
            icon: sd.icon_location().clone().filter(|s| !s.is_empty()),
        })
    })
    .ok()
    .flatten()
}

/// Rebuilds a file system path from shell item ids (drive item followed by
/// file entry items). See the "Windows Shell Item format" documentation.
pub fn idlist_path(items: &[&[u8]]) -> Option<String> {
    let mut parts: Vec<String> = Vec::new();
    for data in items {
        let Some(&t) = data.first() else { continue };
        match t & 0x70 {
            0x20 => {
                let drive: String = data[1..]
                    .iter()
                    .take_while(|b| **b != 0)
                    .map(|b| *b as char)
                    .collect();
                if drive.len() >= 2 && drive.as_bytes()[1] == b':' {
                    parts.clear();
                    parts.push(drive.trim_end_matches('\\').to_string());
                }
            }
            0x30 if !parts.is_empty() => parts.push(file_entry_name(data)?),
            _ => {}
        }
    }
    (parts.len() > 1).then(|| parts.join("\\"))
}

fn file_entry_name(data: &[u8]) -> Option<String> {
    let unicode = data[0] & 0x04 != 0;
    let short = if unicode {
        utf16_until_nul(data.get(12..)?)
    } else {
        data.get(12..)?
            .iter()
            .take_while(|b| **b != 0)
            .map(|b| *b as char)
            .collect()
    };
    // Extension block 0xBEEF0004 holds the long name.
    let sig = data.windows(4).position(|w| w == [0x04, 0x00, 0xEF, 0xBE]);
    // The long name is the last UTF-16 string of the block, right before the
    // trailing 2-byte offset. Walk back from there to the previous NUL unit.
    let long = sig.and_then(|p| {
        let block_start = p.checked_sub(4)?;
        let size = u16::from_le_bytes([data[block_start], data[block_start + 1]]) as usize;
        let block_end = (block_start + size).min(data.len());
        let name_end = block_end.checked_sub(4)?; // NUL terminator position
        if name_end <= p + 4 || data.get(name_end..name_end + 2)? != [0, 0] {
            return None;
        }
        let mut start = name_end;
        while start >= p + 6 {
            let unit = u16::from_le_bytes([data[start - 2], data[start - 1]]);
            if unit == 0 {
                break;
            }
            start -= 2;
        }
        let name = utf16_until_nul(&data[start..name_end + 2]);
        (!name.is_empty() && !name.chars().any(char::is_control)).then_some(name)
    });
    long.or((!short.is_empty()).then_some(short))
}

fn utf16_until_nul(bytes: &[u8]) -> String {
    let units: Vec<u16> = bytes
        .as_chunks::<2>()
        .0
        .iter()
        .map(|c| u16::from_le_bytes(*c))
        .take_while(|u| *u != 0)
        .collect();
    String::from_utf16_lossy(&units)
}

/// Parses an Internet shortcut (`.url`, INI format).
pub fn read_url(path: &Path) -> Option<ShortcutInfo> {
    let bytes = std::fs::read(path).ok()?;
    let text = String::from_utf8_lossy(&bytes);
    Some(parse_url_file(&text))
}

pub fn parse_url_file(text: &str) -> ShortcutInfo {
    let mut info = ShortcutInfo::default();
    let mut in_section = false;
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_section = line.eq_ignore_ascii_case("[InternetShortcut]");
            continue;
        }
        if !in_section {
            continue;
        }
        let Some((k, v)) = line.split_once('=') else {
            continue;
        };
        let v = v.trim().to_string();
        match k.trim().to_ascii_lowercase().as_str() {
            "url" => info.target = Some(v),
            "iconfile" => info.icon = Some(v),
            _ => {}
        }
    }
    info
}

/// Executables of the store launchers themselves.
const LAUNCHER_EXES: &[(&str, Platform)] = &[
    ("steam.exe", Platform::Steam),
    ("epicgameslauncher.exe", Platform::Epic),
    ("upc.exe", Platform::Ubisoft),
    ("ubisoftconnect.exe", Platform::Ubisoft),
    ("ubisoftgamelauncher.exe", Platform::Ubisoft),
    ("eadesktop.exe", Platform::Ea),
    ("ealauncher.exe", Platform::Ea),
    ("origin.exe", Platform::Ea),
    ("battle.net launcher.exe", Platform::Battlenet),
    ("battle.net.exe", Platform::Battlenet),
];

pub fn launcher_exe_platform(exe_name: &str) -> Option<Platform> {
    let lower = exe_name.to_ascii_lowercase();
    LAUNCHER_EXES
        .iter()
        .find(|(n, _)| *n == lower)
        .map(|(_, p)| *p)
}

/// Determines the store and the store specific id of a shortcut.
pub fn detect_platform(kind: ShortcutKind, info: &ShortcutInfo) -> (Platform, Option<String>) {
    let Some(target) = info.target.as_deref() else {
        return (Platform::Other, None);
    };
    if kind == ShortcutKind::Url {
        return detect_from_url(target);
    }
    let exe = file_name_of(target).to_ascii_lowercase();
    let args = info.arguments.as_deref().unwrap_or("");
    if let Some(p) = launcher_exe_platform(&exe) {
        let id = match p {
            Platform::Steam => arg_after(args, "-applaunch"),
            Platform::Battlenet => exec_launch_code(args),
            _ => None,
        };
        // A launcher shortcut with a game id is a game, without one it is the launcher.
        return if id.is_some() {
            (p, id)
        } else {
            (Platform::Other, None)
        };
    }
    let lower = target.replace('/', "\\").to_ascii_lowercase();
    let platform = if lower.contains("\\steamapps\\common\\") {
        Platform::Steam
    } else if lower.contains("\\epic games\\") {
        Platform::Epic
    } else if lower.contains("\\ubisoft game launcher\\games\\") || lower.contains("\\ubisoft\\") {
        Platform::Ubisoft
    } else if lower.contains("\\ea games\\") || lower.contains("\\origin games\\") {
        Platform::Ea
    } else {
        Platform::Other
    };
    (platform, None)
}

fn detect_from_url(url: &str) -> (Platform, Option<String>) {
    let lower = url.to_ascii_lowercase();
    let after = |prefix: &str| -> Option<String> {
        lower
            .starts_with(prefix)
            .then(|| url[prefix.len()..].to_string())
    };
    if let Some(rest) = after("steam://rungameid/").or_else(|| after("steam://run/")) {
        let id: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
        return (Platform::Steam, (!id.is_empty()).then_some(id));
    }
    if let Some(rest) = after("com.epicgames.launcher://apps/") {
        let raw = rest.split(['?', '/']).next().unwrap_or("");
        let decoded = percent_decode_str(raw).decode_utf8_lossy().into_owned();
        return (Platform::Epic, (!decoded.is_empty()).then_some(decoded));
    }
    if let Some(rest) = after("uplay://launch/") {
        let id: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
        return (Platform::Ubisoft, (!id.is_empty()).then_some(id));
    }
    if let Some(rest) = after("link2ea://launchgame/") {
        let id = rest.split(['?', '/']).next().unwrap_or("").to_string();
        return (Platform::Ea, (!id.is_empty()).then_some(id));
    }
    if lower.starts_with("origin2://") || lower.starts_with("origin://") {
        let id = query_param(url, "offerIds")
            .map(|s| percent_decode_str(&s).decode_utf8_lossy().into_owned());
        return (Platform::Ea, id);
    }
    if let Some(rest) = after("battlenet://") {
        let id = rest.trim_end_matches('/').to_string();
        return (Platform::Battlenet, (!id.is_empty()).then_some(id));
    }
    (Platform::Other, None)
}

fn query_param(url: &str, key: &str) -> Option<String> {
    let query = url.split_once('?')?.1;
    query.split('&').find_map(|kv| {
        let (k, v) = kv.split_once('=')?;
        k.eq_ignore_ascii_case(key).then(|| v.to_string())
    })
}

fn arg_after(args: &str, flag: &str) -> Option<String> {
    let mut it = args.split_whitespace();
    while let Some(a) = it.next() {
        if a.eq_ignore_ascii_case(flag) {
            return it.next().map(|s| s.trim_matches('"').to_string());
        }
    }
    None
}

/// Battle.net shortcuts: `--exec="launch Pro"` → `Pro`.
fn exec_launch_code(args: &str) -> Option<String> {
    let lower = args.to_ascii_lowercase();
    let idx = lower.find("launch ")?;
    let rest = &args[idx + "launch ".len()..];
    let code: String = rest
        .chars()
        .take_while(|c| c.is_alphanumeric() || *c == '_')
        .collect();
    (!code.is_empty()).then_some(code)
}

/// Programs that are certainly no games.
const NON_GAME_HINTS: &[&str] = &[
    "chrome",
    "firefox",
    "msedge",
    "edge",
    "opera",
    "brave",
    "discord",
    "spotify",
    "code",
    "visual studio",
    "obs",
    "teams",
    "zoom",
    "slack",
    "skype",
    "whatsapp",
    "telegram",
    "winword",
    "excel",
    "powerpnt",
    "outlook",
    "onenote",
    "acrobat",
    "vlc",
    "notepad",
    "explorer",
    "control",
    "recycle",
    "papierkorb",
    "teamspeak",
    "geforce",
    "nvidia",
    "radeon",
    "afterburner",
    "icue",
    "logitech",
    "razer",
    "7-zip",
    "winrar",
    "putty",
    "filezilla",
    "gimp",
    "photoshop",
    "blender",
    "unity hub",
    "epic games launcher",
    "ubisoft connect",
    "ea app",
    "battle.net",
    "steam",
    "gog galaxy",
    "itunes",
    "anydesk",
    "teamviewer",
    "powershell",
    "cmd",
    "terminal",
    "uninstall",
    "deinstall",
    "readme",
];

const GAME_PATH_HINTS: &[&str] = &[
    "\\games\\",
    "\\spiele\\",
    "steamlibrary",
    "\\steamapps\\",
    "\\gog games\\",
    "\\gog galaxy\\games\\",
    "\\riot games\\",
    "\\xboxgames\\",
    "\\epic games\\",
    "\\ea games\\",
    "\\origin games\\",
    "\\ubisoft\\",
    "\\rockstar games\\",
    "\\blizzard\\",
    "\\battle.net\\",
    "\\bethesda.net launcher\\games\\",
];

fn is_likely_game(
    kind: ShortcutKind,
    info: &ShortcutInfo,
    platform: Platform,
    platform_id: Option<&str>,
    title: &str,
) -> bool {
    if platform != Platform::Other && (platform_id.is_some() || kind != ShortcutKind::Url) {
        return true;
    }
    let target = info.target.as_deref().unwrap_or("");
    if kind == ShortcutKind::Url {
        // Web links are not games.
        return false;
    }
    let title_l = title.to_lowercase();
    let exe_l = file_name_of(target).to_lowercase();
    let hit = |s: &str| {
        NON_GAME_HINTS.iter().any(|h| {
            s == *h
                || s.starts_with(&format!("{h} "))
                || s.starts_with(&format!("{h}."))
                || s.ends_with(&format!(" {h}"))
        })
    };
    if hit(&title_l) || hit(&exe_l) || !exe_l.ends_with(".exe") {
        return false;
    }
    let path_l = format!("\\{}", target.replace('/', "\\").to_lowercase());
    // Many Blizzard/Rockstar/... games start through "<Game> Launcher.exe".
    exe_l.contains("launcher") || GAME_PATH_HINTS.iter().any(|h| path_l.contains(h))
}

/// The user's desktop folder (honours OneDrive redirection via the
/// Known Folder API on Windows).
pub fn desktop_dirs() -> Vec<PathBuf> {
    dirs::desktop_dir().into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn url(u: &str) -> (Platform, Option<String>) {
        detect_platform(
            ShortcutKind::Url,
            &ShortcutInfo {
                target: Some(u.into()),
                ..Default::default()
            },
        )
    }

    fn lnk(target: &str, args: Option<&str>) -> (Platform, Option<String>) {
        detect_platform(
            ShortcutKind::Lnk,
            &ShortcutInfo {
                target: Some(target.into()),
                arguments: args.map(Into::into),
                ..Default::default()
            },
        )
    }

    #[test]
    fn parses_url_files() {
        let text = "[{000214A0-0000-0000-C000-000000000046}]\r\nProp3=19,0\r\n[InternetShortcut]\r\nIDList=\r\nIconIndex=0\r\nURL=steam://rungameid/1086940\r\nIconFile=C:\\Program Files (x86)\\Steam\\steam\\games\\abc.ico\r\n";
        let info = parse_url_file(text);
        assert_eq!(info.target.as_deref(), Some("steam://rungameid/1086940"));
        assert!(info.icon.unwrap().ends_with("abc.ico"));
    }

    #[test]
    fn detects_store_urls() {
        assert_eq!(
            url("steam://rungameid/1086940"),
            (Platform::Steam, Some("1086940".into()))
        );
        assert_eq!(
            url(
                "com.epicgames.launcher://apps/Fortnite%3A4fe75bbc5a674f4f9b356b5c90567da5%3AFortnite?action=launch&silent=true"
            ),
            (
                Platform::Epic,
                Some("Fortnite:4fe75bbc5a674f4f9b356b5c90567da5:Fortnite".into())
            )
        );
        assert_eq!(
            url("uplay://launch/635/0"),
            (Platform::Ubisoft, Some("635".into()))
        );
        assert_eq!(
            url("link2ea://launchgame/Origin.OFR.50.0004657?platform=pcwin"),
            (Platform::Ea, Some("Origin.OFR.50.0004657".into()))
        );
        assert_eq!(
            url("origin2://game/launch?offerIds=OFB-EAST%3A109552316&autoDownload=1"),
            (Platform::Ea, Some("OFB-EAST:109552316".into()))
        );
        assert_eq!(
            url("battlenet://Pro"),
            (Platform::Battlenet, Some("Pro".into()))
        );
        assert_eq!(url("https://example.com"), (Platform::Other, None));
    }

    #[test]
    fn detects_store_lnks() {
        assert_eq!(
            lnk(
                r"C:\Program Files (x86)\Battle.net\Battle.net Launcher.exe",
                Some(r#"--exec="launch Fen""#)
            ),
            (Platform::Battlenet, Some("Fen".into()))
        );
        assert_eq!(
            lnk(
                r"C:\Program Files (x86)\Steam\steam.exe",
                Some("-applaunch 570")
            ),
            (Platform::Steam, Some("570".into()))
        );
        assert_eq!(
            lnk(r"C:\Program Files (x86)\Steam\steam.exe", None),
            (Platform::Other, None)
        );
        assert_eq!(
            lnk(
                r"D:\SteamLibrary\steamapps\common\Valheim\valheim.exe",
                None
            ),
            (Platform::Steam, None)
        );
        assert_eq!(
            lnk(r"C:\Program Files\Epic Games\AlanWake2\AlanWake2.exe", None),
            (Platform::Epic, None)
        );
        assert_eq!(
            lnk(
                r"C:\Program Files\EA Games\Battlefield 2042\BF2042.exe",
                None
            ),
            (Platform::Ea, None)
        );
    }

    #[test]
    fn classifies_games() {
        let mk = |kind, target: &str, title: &str| {
            let info = ShortcutInfo {
                target: Some(target.into()),
                ..Default::default()
            };
            let (p, id) = detect_platform(kind, &info);
            is_likely_game(kind, &info, p, id.as_deref(), title)
        };
        assert!(mk(ShortcutKind::Url, "steam://rungameid/1", "Elden Ring"));
        assert!(!mk(ShortcutKind::Url, "https://youtube.com", "YouTube"));
        assert!(mk(
            ShortcutKind::Lnk,
            r"D:\Games\Witcher3\bin\witcher3.exe",
            "The Witcher 3"
        ));
        assert!(!mk(
            ShortcutKind::Lnk,
            r"C:\Program Files\Google\Chrome\Application\chrome.exe",
            "Google Chrome"
        ));
        assert!(!mk(
            ShortcutKind::Lnk,
            r"C:\Users\a\AppData\Local\Discord\Update.exe",
            "Discord"
        ));
        assert!(!mk(
            ShortcutKind::Lnk,
            r"C:\Program Files (x86)\Steam\steam.exe",
            "Steam"
        ));
        assert!(!mk(ShortcutKind::Lnk, r"C:\Tools\foo.exe", "Foo"));
    }

    #[test]
    fn reads_real_lnk_files() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
        let c = inspect(&dir.join("Hearthstone.lnk")).expect("parsed");
        assert_eq!(c.kind, ShortcutKind::Lnk);
        assert_eq!(c.title, "Hearthstone");
        assert_eq!(
            c.info.target.as_deref(),
            Some(r"E:\Hearthstone\Hearthstone Beta Launcher.exe")
        );
        let c = inspect(&dir.join("iron-heart.lnk")).expect("parsed");
        assert!(c.info.target.unwrap().ends_with("iron-heart.exe"));
    }

    #[test]
    fn scans_directory() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("Elden Ring.url"),
            "[InternetShortcut]\nURL=steam://rungameid/1245620\n",
        )
        .unwrap();
        std::fs::write(
            dir.path().join("YouTube.url"),
            "[InternetShortcut]\nURL=https://youtube.com\n",
        )
        .unwrap();
        std::fs::write(dir.path().join("notes.txt"), "x").unwrap();
        let found = scan_dirs(&[dir.path().to_path_buf()]);
        assert_eq!(found.len(), 2);
        assert_eq!(found[0].title, "Elden Ring");
        assert!(found[0].likely_game);
        assert!(!found[1].likely_game);
    }
}
