//! Playtime measurement.
//!
//! After a launch the tracker waits (up to [`START_TIMEOUT`]) for a process
//! that belongs to the game: its exe name matches, or it runs from the
//! install folder. The session lasts until no such process has been seen for
//! [`END_GRACE`] (covers launchers that restart the game exe).
//!
//! Games started elsewhere (Steam, a desktop shortcut, or before the app
//! itself) are picked up too: every tick also checks the whole library, see
//! [`Tracker::set_library`].
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
    /// Process start, unix seconds, if known.
    pub started: Option<i64>,
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

    /// For spotting games nobody launched through the app, only clues that
    /// point at one game: an exe name, or an install folder that is not a
    /// drive or a shared library folder like `Program Files`.
    fn for_detection(&self) -> Option<Matcher> {
        let install_dir = self.install_dir.clone().filter(|d| is_specific_dir(d));
        let exe_name = self.exe_name.clone().filter(|e| !e.trim().is_empty());
        let m = Matcher {
            install_dir,
            exe_name,
        };
        m.is_usable().then_some(m)
    }

    /// How well a process fits: exe name beats folder, a deeper folder beats
    /// a shallower one. `None` when it does not match.
    fn score(&self, p: &ProcInfo) -> Option<usize> {
        if !self.matches(p) {
            return None;
        }
        let exe_name = p.exe.as_deref().map(file_name_of).unwrap_or(&p.name);
        if self
            .exe_name
            .as_deref()
            .is_some_and(|w| exe_name.eq_ignore_ascii_case(w))
        {
            return Some(usize::MAX);
        }
        Some(self.install_dir.as_deref().map_or(0, str::len))
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

/// Whether a folder names one game rather than a drive or a folder many
/// games share.
fn is_specific_dir(dir: &str) -> bool {
    let norm = dir.replace('/', "\\").to_lowercase();
    let parts: Vec<&str> = norm
        .split('\\')
        .filter(|s| !s.is_empty() && !s.ends_with(':'))
        .collect();
    let Some(last) = parts.last() else {
        return false;
    };
    const SHARED: [&str; 14] = [
        "program files",
        "program files (x86)",
        "programdata",
        "windows",
        "system32",
        "users",
        "appdata",
        "local",
        "roaming",
        "games",
        "common",
        "steamapps",
        "steamlibrary",
        "epic games",
    ];
    !SHARED.contains(last)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum TrackerEvent {
    /// `detected`: the game was not started through the app.
    #[serde(rename_all = "camelCase")]
    Started {
        game_id: i64,
        at: i64,
        detected: bool,
    },
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
    /// Every game in the library, for spotting games started elsewhere.
    library: Vec<(i64, Matcher)>,
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

    /// The games to look out for when they are started outside the app.
    pub fn set_library(&mut self, games: impl IntoIterator<Item = (i64, Matcher)>) {
        self.library = games
            .into_iter()
            .filter_map(|(id, m)| Some((id, m.for_detection()?)))
            .collect();
    }

    pub fn tick(&mut self, procs: &[ProcInfo], now: i64) -> Vec<TrackerEvent> {
        let mut events = self.detect(procs, now);
        let mut done = Vec::new();
        for (&game_id, w) in self.watches.iter_mut() {
            let seen = procs.iter().any(|p| w.matcher.matches(p));
            match (w.started_at, seen) {
                (None, true) => {
                    w.started_at = Some(now);
                    w.last_seen = now;
                    events.push(TrackerEvent::Started {
                        game_id,
                        at: now,
                        detected: false,
                    });
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

    /// Starts watching library games that run without having been launched
    /// here. Each process counts for the one game it fits best, so a game
    /// in a sub-folder of another game's folder is not counted twice.
    fn detect(&mut self, procs: &[ProcInfo], now: i64) -> Vec<TrackerEvent> {
        let mut found: HashMap<i64, i64> = HashMap::new();
        for p in procs {
            let best = self
                .library
                .iter()
                .filter_map(|(id, m)| Some((m.score(p)?, *id)))
                .max_by_key(|&(score, id)| (score, std::cmp::Reverse(id)));
            if let Some((_, id)) = best {
                // Count from when the game really started (it may have been
                // running before the app), but never from the future.
                let at = p.started.filter(|&s| s > 0 && s <= now).unwrap_or(now);
                let e = found.entry(id).or_insert(at);
                *e = (*e).min(at);
            }
        }
        let mut events = Vec::new();
        for (game_id, at) in found {
            if self.watches.contains_key(&game_id) {
                continue;
            }
            let Some((_, matcher)) = self.library.iter().find(|(id, _)| *id == game_id) else {
                continue;
            };
            self.watches.insert(
                game_id,
                Watch {
                    matcher: matcher.clone(),
                    launched_at: at,
                    started_at: Some(at),
                    last_seen: now,
                },
            );
            events.push(TrackerEvent::Started {
                game_id,
                at,
                detected: true,
            });
        }
        events.sort_by_key(|e| match e {
            TrackerEvent::Started { game_id, .. } => *game_id,
            _ => 0,
        });
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
                started: i64::try_from(p.start_time()).ok(),
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
            started: None,
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
            name: "diablo iv.exe".into(),
            started: None,
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
            vec![TrackerEvent::Started {
                game_id: 1,
                at: 30,
                detected: false
            }]
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
    fn detects_games_started_elsewhere() {
        let mut t = Tracker::new();
        t.set_library([
            (1, matcher()),
            // A folder many games share is no clue.
            (
                2,
                Matcher {
                    install_dir: Some(r"D:\SteamLibrary\steamapps\common".into()),
                    exe_name: None,
                },
            ),
            (3, Matcher::default()),
        ]);
        let mut game = proc(10, r"D:\SteamLibrary\steamapps\common\Valheim\valheim.exe");
        // Already running for an hour when the app looked.
        game.started = Some(400);
        let steam = proc(5, r"C:\Program Files (x86)\Steam\steam.exe");
        assert!(t.tick(std::slice::from_ref(&steam), 4000).is_empty());
        assert_eq!(
            t.tick(&[steam, game.clone()], 4005),
            vec![TrackerEvent::Started {
                game_id: 1,
                at: 400,
                detected: true
            }]
        );
        assert_eq!(t.running(), HashSet::from([1]));
        assert!(t.tick(std::slice::from_ref(&game), 4010).is_empty());
        assert!(t.tick(&[], 4015).is_empty());
        assert_eq!(
            t.tick(&[], 4030),
            vec![TrackerEvent::Ended {
                game_id: 1,
                started_at: 400,
                ended_at: 4010
            }]
        );
        assert!(!t.is_watching());
    }

    #[test]
    fn a_launch_is_not_detected_twice() {
        let mut t = Tracker::new();
        t.set_library([(1, matcher())]);
        t.launched(1, matcher(), 0);
        let game = proc(10, r"D:\SteamLibrary\steamapps\common\Valheim\valheim.exe");
        assert_eq!(
            t.tick(std::slice::from_ref(&game), 10),
            vec![TrackerEvent::Started {
                game_id: 1,
                at: 10,
                detected: false
            }]
        );
    }

    #[test]
    fn exe_name_beats_folder() {
        let mut t = Tracker::new();
        t.set_library([
            (
                1,
                Matcher {
                    install_dir: Some(r"C:\Games\Riot Games".into()),
                    exe_name: None,
                },
            ),
            (
                2,
                Matcher {
                    install_dir: None,
                    exe_name: Some("VALORANT-Win64-Shipping.exe".into()),
                },
            ),
        ]);
        let p = proc(
            1,
            r"C:\Games\Riot Games\VALORANT\live\ShooterGame\Binaries\Win64\VALORANT-Win64-Shipping.exe",
        );
        let ev = t.tick(&[p], 100);
        assert_eq!(
            ev,
            vec![TrackerEvent::Started {
                game_id: 2,
                at: 100,
                detected: true
            }]
        );
    }

    #[test]
    fn shared_folders_are_no_clue() {
        assert!(!is_specific_dir(r"C:\"));
        assert!(!is_specific_dir(r"C:\Program Files (x86)\"));
        assert!(!is_specific_dir(r"D:\SteamLibrary\steamapps\common"));
        assert!(is_specific_dir(r"C:\Program Files\Epic Games\Fortnite"));
    }

    #[test]
    fn snapshot_works() {
        let procs = SystemProcesses::new().snapshot();
        assert!(!procs.is_empty());
    }
}
