//! Finds where a game is installed and which exe it runs as. Needed to
//! measure playtime: store launchers start the real game process indirectly.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::model::Platform;
use crate::scanner::launcher_exe_platform;
use crate::util::{file_name_of, normalize_name, parent_of};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallInfo {
    pub install_dir: Option<String>,
    pub process_exe: Option<String>,
}

/// Best effort lookup. Never fails, fields stay `None` if nothing is found.
pub fn locate(
    platform: Platform,
    platform_id: Option<&str>,
    launch_target: Option<&str>,
    name: &str,
) -> InstallInfo {
    // A shortcut pointing straight at the game exe is the most reliable source.
    if let Some(target) = launch_target {
        let exe = file_name_of(target);
        if exe.to_ascii_lowercase().ends_with(".exe") && launcher_exe_platform(exe).is_none() {
            let lower = exe.to_ascii_lowercase();
            // "<Game> Launcher.exe" starts another exe: only the folder is useful.
            let process_exe = (!lower.contains("launcher")).then(|| exe.to_string());
            return InstallInfo {
                install_dir: parent_of(target).map(str::to_string),
                process_exe,
            };
        }
    }
    let found = match (platform, platform_id) {
        (Platform::Steam, Some(id)) => steam_roots()
            .iter()
            .find_map(|root| steam_install_dir(root, id))
            .map(dir_only),
        (Platform::Epic, Some(id)) => epic_manifest_dir().and_then(|d| epic_lookup(&d, id, name)),
        (Platform::Ubisoft, Some(id)) => ubisoft_install_dir(id).map(dir_only),
        _ => None,
    };
    found
        .or_else(|| uninstall_lookup(name).map(dir_only))
        .unwrap_or_default()
}

fn dir_only(p: PathBuf) -> InstallInfo {
    InstallInfo {
        install_dir: Some(p.to_string_lossy().into_owned()),
        process_exe: None,
    }
}

// ---- Steam ------------------------------------------------------------------

/// All values of `"key" "value"` pairs in a Valve KeyValues (VDF/ACF) file.
pub fn vdf_values(text: &str, key: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in text.lines() {
        let parts = quoted_tokens(line.trim());
        if parts.len() == 2 && parts[0].eq_ignore_ascii_case(key) {
            out.push(parts[1].clone());
        }
    }
    out
}

fn quoted_tokens(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut chars = line.chars();
    while let Some(c) = chars.next() {
        if c != '"' {
            continue;
        }
        let mut s = String::new();
        while let Some(c) = chars.next() {
            match c {
                '\\' => {
                    if let Some(n) = chars.next() {
                        s.push(n);
                    }
                }
                '"' => break,
                c => s.push(c),
            }
        }
        out.push(s);
    }
    out
}

pub fn steam_roots() -> Vec<PathBuf> {
    #[allow(unused_mut)]
    let mut roots = Vec::new();
    #[cfg(windows)]
    {
        use winreg::RegKey;
        use winreg::enums::HKEY_CURRENT_USER;
        if let Ok(key) = RegKey::predef(HKEY_CURRENT_USER).open_subkey(r"Software\Valve\Steam") {
            if let Ok(p) = key.get_value::<String, _>("SteamPath") {
                roots.push(PathBuf::from(p.replace('/', "\\")));
            }
        }
        roots.push(PathBuf::from(r"C:\Program Files (x86)\Steam"));
    }
    roots
}

/// `<library>/steamapps/common/<installdir>` for an AppID, searching all
/// library folders listed in `libraryfolders.vdf`.
pub fn steam_install_dir(steam_root: &Path, appid: &str) -> Option<PathBuf> {
    let mut libraries = vec![steam_root.to_path_buf()];
    if let Ok(text) =
        std::fs::read_to_string(steam_root.join("steamapps").join("libraryfolders.vdf"))
    {
        libraries.extend(vdf_values(&text, "path").into_iter().map(PathBuf::from));
    }
    libraries.into_iter().find_map(|lib| {
        let steamapps = lib.join("steamapps");
        let text =
            std::fs::read_to_string(steamapps.join(format!("appmanifest_{appid}.acf"))).ok()?;
        let dir = vdf_values(&text, "installdir").into_iter().next()?;
        Some(steamapps.join("common").join(dir))
    })
}

// ---- Epic -------------------------------------------------------------------

fn epic_manifest_dir() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        let base = std::env::var_os("ProgramData")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(r"C:\ProgramData"));
        return Some(base.join(r"Epic\EpicGamesLauncher\Data\Manifests"));
    }
    #[allow(unreachable_code)]
    None
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct EpicManifest {
    #[serde(default)]
    app_name: String,
    #[serde(default)]
    display_name: String,
    #[serde(default)]
    install_location: String,
    #[serde(default)]
    launch_executable: String,
}

/// Looks up an Epic game in the launcher's `.item` manifests. `id` is the
/// `namespace:catalogItem:appName` part of the launch URL.
pub fn epic_lookup(manifest_dir: &Path, id: &str, name: &str) -> Option<InstallInfo> {
    let app_name = id.rsplit(':').next().unwrap_or(id);
    let wanted = normalize_name(name);
    let mut by_name = None;
    for entry in std::fs::read_dir(manifest_dir).ok()?.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("item") {
            continue;
        }
        let Ok(bytes) = std::fs::read(&path) else {
            continue;
        };
        let Ok(m) = serde_json::from_slice::<EpicManifest>(&bytes) else {
            continue;
        };
        if m.install_location.is_empty() {
            continue;
        }
        let info = InstallInfo {
            install_dir: Some(m.install_location.clone()),
            process_exe: (!m.launch_executable.is_empty())
                .then(|| file_name_of(&m.launch_executable).to_string()),
        };
        if m.app_name.eq_ignore_ascii_case(app_name) {
            return Some(info);
        }
        if normalize_name(&m.display_name) == wanted {
            by_name = Some(info);
        }
    }
    by_name
}

// ---- Registry (Windows only) ----------------------------------------------

#[cfg(windows)]
fn ubisoft_install_dir(id: &str) -> Option<PathBuf> {
    use winreg::RegKey;
    use winreg::enums::HKEY_LOCAL_MACHINE;
    let key = RegKey::predef(HKEY_LOCAL_MACHINE)
        .open_subkey(format!(
            r"SOFTWARE\WOW6432Node\Ubisoft\Launcher\Installs\{id}"
        ))
        .ok()?;
    let dir: String = key.get_value("InstallDir").ok()?;
    Some(PathBuf::from(dir.replace('/', "\\")))
}

#[cfg(not(windows))]
fn ubisoft_install_dir(_id: &str) -> Option<PathBuf> {
    None
}

/// Searches the "Programs and Features" registry entries for a matching name.
#[cfg(windows)]
fn uninstall_lookup(name: &str) -> Option<PathBuf> {
    use winreg::RegKey;
    use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};
    let wanted = normalize_name(name);
    if wanted.is_empty() {
        return None;
    }
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
    for (hive, path) in roots {
        let Ok(key) = RegKey::predef(hive).open_subkey(path) else {
            continue;
        };
        for sub in key.enum_keys().flatten() {
            let Ok(app) = key.open_subkey(&sub) else {
                continue;
            };
            let Ok(display) = app.get_value::<String, _>("DisplayName") else {
                continue;
            };
            if normalize_name(&display) != wanted {
                continue;
            }
            if let Ok(loc) = app.get_value::<String, _>("InstallLocation") {
                let loc = loc.trim().trim_matches('"').to_string();
                if !loc.is_empty() {
                    return Some(PathBuf::from(loc));
                }
            }
        }
    }
    None
}

#[cfg(not(windows))]
fn uninstall_lookup(_name: &str) -> Option<PathBuf> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_vdf() {
        let text = r#"
"libraryfolders"
{
	"0"
	{
		"path"		"C:\\Program Files (x86)\\Steam"
		"apps" { "228980" "1" }
	}
	"1"
	{
		"path"		"D:\\SteamLibrary"
	}
}"#;
        assert_eq!(
            vdf_values(text, "path"),
            vec![
                r"C:\Program Files (x86)\Steam".to_string(),
                r"D:\SteamLibrary".to_string()
            ]
        );
    }

    #[test]
    fn finds_steam_game_in_second_library() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("Steam");
        let lib2 = tmp.path().join("SteamLibrary");
        std::fs::create_dir_all(root.join("steamapps")).unwrap();
        std::fs::create_dir_all(lib2.join("steamapps")).unwrap();
        let lib2_escaped = lib2.to_string_lossy().replace('\\', "\\\\");
        std::fs::write(
            root.join("steamapps/libraryfolders.vdf"),
            format!("\"libraryfolders\"\n{{\n\"1\"\n{{\n\"path\" \"{lib2_escaped}\"\n}}\n}}\n"),
        )
        .unwrap();
        std::fs::write(
            lib2.join("steamapps/appmanifest_892970.acf"),
            "\"AppState\"\n{\n\t\"appid\"\t\t\"892970\"\n\t\"name\"\t\t\"Valheim\"\n\t\"installdir\"\t\t\"Valheim\"\n}\n",
        )
        .unwrap();
        assert_eq!(
            steam_install_dir(&root, "892970"),
            Some(lib2.join("steamapps/common/Valheim"))
        );
        assert_eq!(steam_install_dir(&root, "1"), None);
    }

    #[test]
    fn finds_epic_game() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(
            tmp.path().join("ABC.item"),
            r#"{"AppName":"Fortnite","DisplayName":"Fortnite","InstallLocation":"C:\\Program Files\\Epic Games\\Fortnite","LaunchExecutable":"FortniteGame/Binaries/Win64/FortniteLauncher.exe"}"#,
        )
        .unwrap();
        let info = epic_lookup(tmp.path(), "Fortnite:4fe75bbc:Fortnite", "Fortnite").unwrap();
        assert_eq!(
            info.install_dir.as_deref(),
            Some(r"C:\Program Files\Epic Games\Fortnite")
        );
        assert_eq!(info.process_exe.as_deref(), Some("FortniteLauncher.exe"));
        assert!(epic_lookup(tmp.path(), "x:y:Other", "Other").is_none());
    }

    #[test]
    fn direct_exe_shortcuts() {
        let info = locate(
            Platform::Other,
            None,
            Some(r"D:\Games\Witcher3\bin\x64\witcher3.exe"),
            "The Witcher 3",
        );
        assert_eq!(
            info.install_dir.as_deref(),
            Some(r"D:\Games\Witcher3\bin\x64")
        );
        assert_eq!(info.process_exe.as_deref(), Some("witcher3.exe"));
        let info = locate(
            Platform::Other,
            None,
            Some(r"C:\Games\Diablo IV\Diablo IV Launcher.exe"),
            "Diablo IV",
        );
        assert_eq!(info.install_dir.as_deref(), Some(r"C:\Games\Diablo IV"));
        assert_eq!(info.process_exe, None);
    }
}
