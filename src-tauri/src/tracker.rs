use std::time::Duration;

use launcher_core::playtime::{SystemProcesses, TrackerEvent};
use launcher_core::util::now;
use tauri::{AppHandle, Emitter, Manager};

use crate::state::AppState;

const INTERVAL: Duration = Duration::from_secs(5);

/// Polls running processes while a launched game is being watched.
pub fn spawn(app: AppHandle) {
    std::thread::spawn(move || {
        let mut procs = SystemProcesses::new();
        loop {
            std::thread::sleep(INTERVAL);
            let state = app.state::<AppState>();
            if !state.tracker.lock().unwrap().is_watching() {
                continue;
            }
            let snapshot = procs.snapshot();
            let events = state.tracker.lock().unwrap().tick(&snapshot, now());
            for ev in events {
                if let TrackerEvent::Ended {
                    game_id,
                    started_at,
                    ended_at,
                } = ev
                {
                    let lib = state.lib.lock().unwrap();
                    if lib.add_session(game_id, started_at, ended_at).is_ok()
                        && let Ok(game) = lib.game(game_id)
                    {
                        let _ = app.emit("game-updated", game);
                    }
                }
                let _ = app.emit("tracker", ev);
            }
        }
    });
}
