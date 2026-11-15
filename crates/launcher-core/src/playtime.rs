//! Playtime measurement.
//!
//! After a launch the tracker waits (up to [`START_TIMEOUT`]) for a process
//! that belongs to the game: its exe name matches, or it runs from the
//! install folder. The session lasts until no such process has been seen for
//! [`END_GRACE`] (covers launchers that restart the game exe).
//!
//! The tracker is pure logic fed with process snapshots, so it is testable;
//! [`SystemProcesses`] provides real snapshots via `sysinfo`.

use std::collections::{HashMap, HashSet};

use serde::Serialize;

use crate::scanner::launcher_exe_platform;
use crate::util::{file_name_of, path_starts_with};

pub const START_TIMEOUT: i64 = 180;
pub const END_GRACE: i64 = 20;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcInfo {
    pub pid: u32,
    /// Full exe path if known.
    pub exe: Option<String>,
    pub name: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Matcher {
    pub install_dir: Option<String>,
    pub exe_name: Option<String>,
}

impl Matcher {
    pub fn is_usable(&self) -> bool {
        self.install_dir.is_some() || self.exe_name.is_some()
    }

    pub fn matches(&self, p: &ProcInfo) -> bool {
        let exe_name = p.exe.as_deref().map(file_name_of).unwrap_or(&p.name);
        if launcher_exe_platform(exe_name).is_some() {
            return false;
        }
        // Helpers that live in game folders but are not the game.
        let lower = exe_name.to_ascii_lowercase();
        if [
            "crashreport",
            "crashhandler",
            "unitycrashhandler",
            "easyanticheat",
            "be_service",
            "vc_redist",
            "dxsetup",
        ]
        .iter()
        .any(|h| lower.contains(h))
        {
            return false;
        }
        if let Some(want) = &self.exe_name
            && exe_name.eq_ignore_ascii_case(want)
        {
            return true;
        }
        match (&self.install_dir, &p.exe) {
            (Some(dir), Some(exe)) => path_starts_with(exe, dir),
            _ => false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum TrackerEvent {
    #[serde(rename_all = "camelCase")]
    Started { game_id: i64, at: i64 },
    #[serde(rename_all = "camelCase")]
    Ended {
        game_id: i64,
        started_at: i64,
        ended_at: i64,
    },
    /// No game process appeared after the launch.
    #[serde(rename_all = "camelCase")]
    NotDetected { game_id: i64 },
}

#[derive(Debug)]
struct Watch {
    matcher: Matcher,
    launched_at: i64,
    started_at: Option<i64>,
    last_seen: i64,
}

#[derive(Debug, Default)]
pub struct Tracker {
    watches: HashMap<i64, Watch>,
}

impl Tracker {
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a launch. A second launch of a running game is ignored.
    pub fn launched(&mut self, game_id: i64, matcher: Matcher, now: i64) {
        if !matcher.is_usable() {
            return;
        }
        self.watches.entry(game_id).or_insert(Watch {
            matcher,
            launched_at: now,
            started_at: None,
            last_seen: now,
        });
    }

    pub fn running(&self) -> HashSet<i64> {
        self.watches
            .iter()
            .filter(|(_, w)| w.started_at.is_some())
            .map(|(id, _)| *id)
            .collect()
    }

    pub fn is_watching(&self) -> bool {
        !self.watches.is_empty()
    }

    pub fn tick(&mut self, procs: &[ProcInfo], now: i64) -> Vec<TrackerEvent> {
        let mut events = Vec::new();
        let mut done = Vec::new();
        for (&game_id, w) in self.watches.iter_mut() {
            let seen = procs.iter().any(|p| w.matcher.matches(p));
            match (w.started_at, seen) {
                (None, true) => {
                    w.started_at = Some(now);
                    w.last_seen = now;
                    events.push(TrackerEvent::Started { game_id, at: now });
                }
                (None, false) if now - w.launched_at > START_TIMEOUT => {
                    events.push(TrackerEvent::NotDetected { game_id });
                    done.push(game_id);
                }
                (Some(_), true) => w.last_seen = now,
                (Some(started), false) if now - w.last_seen >= END_GRACE => {
                    events.push(TrackerEvent::Ended {
                        game_id,
                        started_at: started,
                        ended_at: w.last_seen,
                    });
                    done.push(game_id);
                }
                _ => {}
            }
        }
        for id in done {
            self.watches.remove(&id);
        }
        events
    }
}

/// Process snapshots of the running system.
pub struct SystemProcesses {
    sys: sysinfo::System,
}

impl Default for SystemProcesses {
    fn default() -> Self {
        Self::new()
    }
}

impl SystemProcesses {
    pub fn new() -> Self {
        Self {
            sys: sysinfo::System::new(),
        }
    }

    pub fn snapshot(&mut self) -> Vec<ProcInfo> {
        use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, UpdateKind};
        self.sys.refresh_processes_specifics(
            ProcessesToUpdate::All,
            true,
            ProcessRefreshKind::nothing().with_exe(UpdateKind::OnlyIfNotSet),
        );
        self.sys
            .processes()
            .iter()
            .map(|(pid, p)| ProcInfo {
                pid: pid.as_u32(),
                exe: p.exe().map(|e| e.to_string_lossy().into_owned()),
                name: p.name().to_string_lossy().into_owned(),
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn proc(pid: u32, exe: &str) -> ProcInfo {
        ProcInfo {
            pid,
            exe: Some(exe.into()),
            name: file_name_of(exe).into(),
        }
    }

    fn matcher() -> Matcher {
        Matcher {
            install_dir: Some(r"D:\SteamLibrary\steamapps\common\Valheim".into()),
            exe_name: None,
        }
    }

    #[test]
    fn matches_by_dir_and_name() {
        let m = matcher();
        assert!(m.matches(&proc(
            1,
            r"D:\SteamLibrary\steamapps\common\Valheim\valheim.exe"
        )));
        assert!(!m.matches(&proc(
            2,
            r"D:\SteamLibrary\steamapps\common\Valheim\UnityCrashHandler64.exe"
        )));
        assert!(!m.matches(&proc(3, r"C:\Program Files (x86)\Steam\steam.exe")));
        let m = Matcher {
            install_dir: None,
            exe_name: Some("Diablo IV.exe".into()),
        };
        assert!(m.matches(&ProcInfo {
            pid: 4,
            exe: None,
            name: "diablo iv.exe".into()
        }));
    }

    #[test]
    fn full_session() {
        let mut t = Tracker::new();
        t.launched(1, matcher(), 0);
        let game = proc(10, r"D:\SteamLibrary\steamapps\common\Valheim\valheim.exe");
        let steam = proc(5, r"C:\Program Files (x86)\Steam\steam.exe");

        assert!(t.tick(std::slice::from_ref(&steam), 5).is_empty());
        assert_eq!(
            t.tick(&[steam.clone(), game.clone()], 30),
            vec![TrackerEvent::Started { game_id: 1, at: 30 }]
        );
        assert_eq!(t.running(), HashSet::from([1]));
        assert!(t.tick(std::slice::from_ref(&game), 3600).is_empty());
        // Gone briefly (restart): no end yet.
        assert!(t.tick(&[], 3605).is_empty());
        assert!(t.tick(std::slice::from_ref(&game), 3610).is_empty());
        assert!(t.tick(&[], 3620).is_empty());
        assert_eq!(
            t.tick(&[], 3640),
            vec![TrackerEvent::Ended {
                game_id: 1,
                started_at: 30,
                ended_at: 3610
            }]
        );
        assert!(!t.is_watching());
    }

    #[test]
    fn gives_up_when_nothing_starts() {
        let mut t = Tracker::new();
        t.launched(1, matcher(), 0);
        assert!(t.tick(&[], 100).is_empty());
        assert_eq!(
            t.tick(&[], START_TIMEOUT + 1),
            vec![TrackerEvent::NotDetected { game_id: 1 }]
        );
        assert!(!t.is_watching());
    }

    #[test]
    fn ignores_unusable_matchers() {
        let mut t = Tracker::new();
        t.launched(1, Matcher::default(), 0);
        assert!(!t.is_watching());
    }

    #[test]
    fn snapshot_works() {
        let procs = SystemProcesses::new().snapshot();
        assert!(!procs.is_empty());
    }
}
