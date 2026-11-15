use std::time::Duration;

use launcher_core::playtime::{Matcher, SystemProcesses, TrackerEvent};
use launcher_core::util::now;
use tauri::{AppHandle, Emitter, Manager};

use crate::state::AppState;

/// While a game runs.
const INTERVAL: Duration = Duration::from_secs(5);
/// Looking out for games started outside the app.
const IDLE_INTERVAL: Duration = Duration::from_secs(10);

/// Polls running processes: for games launched here, and for library games
/// started elsewhere (a launcher, a shortcut, or before the app).
pub fn spawn(app: AppHandle) {
    std::thread::spawn(move || {
        let mut procs = SystemProcesses::new();
        loop {
            let state = app.state::<AppState>();
            let watching = state.tracker.lock().unwrap().is_watching();
            std::thread::sleep(if watching { INTERVAL } else { IDLE_INTERVAL });

            let hints = state
                .lib
                .lock()
                .unwrap()
                .process_hints()
                .unwrap_or_default();
            state
                .tracker
                .lock()
                .unwrap()
                .set_library(hints.into_iter().map(|h| {
                    (
                        h.id,
                        Matcher {
                            install_dir: h.install_dir,
                            exe_name: h.process_exe,
                        },
                    )
                }));
            let snapshot = procs.snapshot();
            let events = state.tracker.lock().unwrap().tick(&snapshot, now());
            for ev in events {
                let lib = state.lib.lock().unwrap();
                let changed = match ev {
                    TrackerEvent::Started {
                        game_id,
                        at,
                        detected: true,
                    } => lib.record_launch(game_id, at).is_ok(),
                    TrackerEvent::Ended {
                        game_id,
                        started_at,
                        ended_at,
                    } => lib.add_session(game_id, started_at, ended_at).is_ok(),
                    _ => false,
                };
                let game_id = match ev {
                    TrackerEvent::Started { game_id, .. }
                    | TrackerEvent::Ended { game_id, .. }
                    | TrackerEvent::NotDetected { game_id } => game_id,
                };
                if changed && let Ok(game) = lib.game(game_id) {
                    let _ = app.emit("game-updated", game);
                }
                drop(lib);
                let _ = app.emit("tracker", ev);
            }
        }
    });
}
