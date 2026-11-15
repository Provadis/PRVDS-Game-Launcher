//! Self-update through GitHub Releases, signed with the Tauri updater key.
//!
//! Two channels:
//! - `stable`: the latest release (tags `v*` on `main`),
//! - `dev`:    the rolling `dev-latest` pre-release, rebuilt on every push to `dev`.
//!
//! The public key is baked in at build time (`PRVDS_UPDATER_PUBKEY`). Builds
//! without it (local builds, forks) simply report that updates are off.

use std::sync::Mutex;

use serde::Serialize;
use tauri::{AppHandle, State};
use tauri_plugin_updater::{Update, UpdaterExt};

type CmdResult<T> = Result<T, String>;

const REPO: &str = "https://github.com/Provadis/PRVDS-Game-Launcher";

/// Public key of the update signature, set by the release workflow.
pub const PUBKEY: Option<&str> = option_env!("PRVDS_UPDATER_PUBKEY");

pub fn enabled() -> bool {
    PUBKEY.is_some_and(|k| !k.trim().is_empty())
}

pub fn plugin<R: tauri::Runtime>() -> tauri::plugin::TauriPlugin<R, tauri_plugin_updater::Config> {
    let builder = tauri_plugin_updater::Builder::new();
    match PUBKEY.filter(|k| !k.trim().is_empty()) {
        Some(key) => builder.pubkey(key.trim()).build(),
        None => builder.build(),
    }
}

/// The manifest URL of a channel.
pub fn endpoint(channel: &str) -> String {
    match channel {
        "dev" => format!("{REPO}/releases/download/dev-latest/latest.json"),
        _ => format!("{REPO}/releases/latest/download/latest.json"),
    }
}

/// An update found by [`check_update`], kept until it is installed.
#[derive(Default)]
pub struct PendingUpdate(Mutex<Option<Update>>);

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    version: String,
    current_version: String,
    notes: Option<String>,
    /// RFC 3339
    date: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateCheck {
    /// False when this build carries no update key.
    enabled: bool,
    update: Option<UpdateInfo>,
}

#[tauri::command]
pub async fn check_update(
    app: AppHandle,
    pending: State<'_, PendingUpdate>,
    channel: String,
) -> CmdResult<UpdateCheck> {
    if !enabled() {
        return Ok(UpdateCheck {
            enabled: false,
            update: None,
        });
    }
    let url = endpoint(&channel).parse().map_err(|e| format!("{e}"))?;
    let updater = app
        .updater_builder()
        .endpoints(vec![url])
        .map_err(|e| e.to_string())?
        .build()
        .map_err(|e| e.to_string())?;
    let found = updater.check().await.map_err(|e| e.to_string())?;
    let info = found.as_ref().map(|u| UpdateInfo {
        version: u.version.clone(),
        current_version: u.current_version.clone(),
        notes: u.body.clone(),
        date: u.date.and_then(|d| {
            d.format(&time::format_description::well_known::Rfc3339)
                .ok()
        }),
    });
    *pending.0.lock().unwrap() = found;
    Ok(UpdateCheck {
        enabled: true,
        update: info,
    })
}

/// Downloads, verifies and installs the update found last. On Windows the
/// installer closes the app and starts the new version.
#[tauri::command]
pub async fn install_update(app: AppHandle, pending: State<'_, PendingUpdate>) -> CmdResult<()> {
    let update = pending
        .0
        .lock()
        .unwrap()
        .take()
        .ok_or("Kein Update gefunden. Bitte zuerst nach Updates suchen.")?;
    update
        .download_and_install(|_, _| {}, || {})
        .await
        .map_err(|e| e.to_string())?;
    app.restart();
}
