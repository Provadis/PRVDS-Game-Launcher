//! Moves shortcuts from the desktop into the library folder, safely and with undo.
//!
//! Steps for every file:
//! 1. copy it into the library folder (unique name),
//! 2. compare the copy byte by byte with the original,
//! 3. write a `pending` entry to the journal,
//! 4. delete the original,
//! 5. mark the entry as `moved`.
//!
//! If the app dies between 3 and 5, [`Importer::recover`] finishes or rolls
//! back the move on the next start. Undo copies the file back the same way.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

use serde::{Deserialize, Serialize};

use crate::desktop_icons::IconPosition;
use crate::error::{Error, IoContext, Result};
use crate::util::now;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EntryState {
    Pending,
    Moved,
    Restored,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JournalEntry {
    pub id: String,
    pub original_path: String,
    pub stored_path: String,
    pub moved_at: i64,
    pub state: EntryState,
    /// Where the icon sat on the desktop before the import, if known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub desktop_position: Option<IconPosition>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct Journal {
    entries: Vec<JournalEntry>,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecoveryReport {
    /// Pending moves that were completed.
    pub completed: Vec<String>,
    /// Pending moves that were rolled back (original still there).
    pub rolled_back: Vec<String>,
}

pub struct Importer {
    shortcuts_dir: PathBuf,
    journal_path: PathBuf,
}

static COUNTER: AtomicU32 = AtomicU32::new(0);

impl Importer {
    pub fn new(
        shortcuts_dir: impl Into<PathBuf>,
        journal_path: impl Into<PathBuf>,
    ) -> Result<Self> {
        let shortcuts_dir = shortcuts_dir.into();
        fs::create_dir_all(&shortcuts_dir).ctx(&shortcuts_dir)?;
        Ok(Self {
            shortcuts_dir,
            journal_path: journal_path.into(),
        })
    }

    pub fn shortcuts_dir(&self) -> &Path {
        &self.shortcuts_dir
    }

    fn load(&self) -> Result<Journal> {
        match fs::read(&self.journal_path) {
            Ok(bytes) => Ok(serde_json::from_slice(&bytes)?),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Journal::default()),
            Err(e) => Err(Error::Io {
                path: self.journal_path.clone(),
                source: e,
            }),
        }
    }

    /// Writes the journal atomically (temp file + rename).
    fn save(&self, journal: &Journal) -> Result<()> {
        let tmp = self.journal_path.with_extension("json.tmp");
        if let Some(parent) = self.journal_path.parent() {
            fs::create_dir_all(parent).ctx(parent)?;
        }
        fs::write(&tmp, serde_json::to_vec_pretty(journal)?).ctx(&tmp)?;
        fs::rename(&tmp, &self.journal_path).ctx(&self.journal_path)?;
        Ok(())
    }

    fn update_state(&self, id: &str, state: EntryState) -> Result<JournalEntry> {
        let mut journal = self.load()?;
        let entry = journal
            .entries
            .iter_mut()
            .find(|e| e.id == id)
            .ok_or_else(|| Error::NotFound(format!("Journal-Eintrag {id}")))?;
        entry.state = state;
        let entry = entry.clone();
        self.save(&journal)?;
        Ok(entry)
    }

    pub fn entries(&self) -> Result<Vec<JournalEntry>> {
        Ok(self.load()?.entries)
    }

    pub fn entry(&self, id: &str) -> Result<JournalEntry> {
        self.load()?
            .entries
            .into_iter()
            .find(|e| e.id == id)
            .ok_or_else(|| Error::NotFound(format!("Journal-Eintrag {id}")))
    }

    /// Moves one shortcut into the library folder.
    pub fn move_in(&self, src: &Path) -> Result<JournalEntry> {
        self.move_in_at(src, None)
    }

    /// Like [`Importer::move_in`], and remembers the icon's desktop position
    /// so a later restore can put it back on the same spot.
    pub fn move_in_at(
        &self,
        src: &Path,
        desktop_position: Option<IconPosition>,
    ) -> Result<JournalEntry> {
        let file_name = src
            .file_name()
            .ok_or_else(|| Error::Other(format!("Ungültiger Pfad: {}", src.display())))?;
        let dest = unique_path(&self.shortcuts_dir, file_name.to_string_lossy().as_ref());
        copy_verified(src, &dest)?;

        let entry = JournalEntry {
            id: new_id(),
            original_path: src.to_string_lossy().into_owned(),
            stored_path: dest.to_string_lossy().into_owned(),
            moved_at: now(),
            state: EntryState::Pending,
            desktop_position,
        };
        let mut journal = self.load()?;
        journal.entries.push(entry.clone());
        if let Err(e) = self.save(&journal) {
            let _ = fs::remove_file(&dest);
            return Err(e);
        }

        if let Err(e) = fs::remove_file(src) {
            // Original could not be removed (e.g. locked): roll back.
            let _ = fs::remove_file(&dest);
            journal.entries.retain(|x| x.id != entry.id);
            let _ = self.save(&journal);
            return Err(Error::Io {
                path: src.to_path_buf(),
                source: e,
            });
        }
        self.update_state(&entry.id, EntryState::Moved)
    }

    /// Moves a shortcut back to where it came from. If a file with the same
    /// name exists there, a numbered name is used. Returns the restored path.
    pub fn restore(&self, id: &str) -> Result<PathBuf> {
        let entry = self.entry(id)?;
        if entry.state == EntryState::Restored {
            return Ok(PathBuf::from(entry.original_path));
        }
        let original = PathBuf::from(&entry.original_path);
        let dir = original
            .parent()
            .map(Path::to_path_buf)
            .ok_or_else(|| Error::Other(format!("Ungültiger Pfad: {}", entry.original_path)))?;
        fs::create_dir_all(&dir).ctx(&dir)?;
        let name = original
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let dest = unique_path(&dir, &name);
        let stored = PathBuf::from(&entry.stored_path);
        copy_verified(&stored, &dest)?;
        fs::remove_file(&stored).ctx(&stored)?;
        self.update_state(id, EntryState::Restored)?;
        Ok(dest)
    }

    /// Repairs entries left `pending` by a crash.
    pub fn recover(&self) -> Result<RecoveryReport> {
        let mut journal = self.load()?;
        let mut report = RecoveryReport::default();
        let mut changed = false;
        journal.entries.retain_mut(|e| {
            if e.state != EntryState::Pending {
                return true;
            }
            changed = true;
            let original = Path::new(&e.original_path);
            let stored = Path::new(&e.stored_path);
            if original.exists() {
                // The original was not deleted: undo the copy.
                let _ = fs::remove_file(stored);
                report.rolled_back.push(e.id.clone());
                false
            } else if stored.exists() {
                e.state = EntryState::Moved;
                report.completed.push(e.id.clone());
                true
            } else {
                false
            }
        });
        if changed {
            self.save(&journal)?;
        }
        Ok(report)
    }
}

fn new_id() -> String {
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("{nanos:x}-{n:x}")
}

/// `dir/name`, or `dir/name (2).ext`, `dir/name (3).ext`, ... if taken.
pub fn unique_path(dir: &Path, name: &str) -> PathBuf {
    let candidate = dir.join(name);
    if !candidate.exists() {
        return candidate;
    }
    let p = Path::new(name);
    let stem = p
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let ext = p
        .extension()
        .map(|e| format!(".{}", e.to_string_lossy()))
        .unwrap_or_default();
    (2..)
        .map(|i| dir.join(format!("{stem} ({i}){ext}")))
        .find(|c| !c.exists())
        .expect("infinite iterator")
}

fn copy_verified(src: &Path, dest: &Path) -> Result<()> {
    let original = fs::read(src).ctx(src)?;
    fs::write(dest, &original).ctx(dest)?;
    let copy = fs::read(dest).ctx(dest)?;
    if copy != original {
        let _ = fs::remove_file(dest);
        return Err(Error::VerifyFailed(src.to_path_buf()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup() -> (tempfile::TempDir, PathBuf, Importer) {
        let tmp = tempfile::tempdir().unwrap();
        let desktop = tmp.path().join("Desktop");
        fs::create_dir_all(&desktop).unwrap();
        let imp = Importer::new(
            tmp.path().join("lib/Shortcuts"),
            tmp.path().join("lib/journal.json"),
        )
        .unwrap();
        (tmp, desktop, imp)
    }

    #[test]
    fn moves_and_restores() {
        let (_tmp, desktop, imp) = setup();
        let src = desktop.join("Elden Ring.url");
        fs::write(&src, b"[InternetShortcut]\nURL=steam://rungameid/1245620\n").unwrap();

        let entry = imp.move_in(&src).unwrap();
        assert!(!src.exists());
        assert_eq!(entry.state, EntryState::Moved);
        assert_eq!(
            fs::read(&entry.stored_path).unwrap(),
            b"[InternetShortcut]\nURL=steam://rungameid/1245620\n"
        );

        let back = imp.restore(&entry.id).unwrap();
        assert_eq!(back, src);
        assert!(src.exists());
        assert!(!Path::new(&entry.stored_path).exists());
        assert_eq!(imp.entry(&entry.id).unwrap().state, EntryState::Restored);
    }

    #[test]
    fn handles_name_clashes() {
        let (_tmp, desktop, imp) = setup();
        let a = desktop.join("Game.lnk");
        fs::write(&a, b"one").unwrap();
        let e1 = imp.move_in(&a).unwrap();
        fs::write(&a, b"two").unwrap();
        let e2 = imp.move_in(&a).unwrap();
        assert_ne!(e1.stored_path, e2.stored_path);
        assert!(e2.stored_path.ends_with("Game (2).lnk"));

        // A new file with the same name appeared on the desktop meanwhile.
        fs::write(&a, b"three").unwrap();
        let restored = imp.restore(&e1.id).unwrap();
        assert!(restored.ends_with("Game (2).lnk"));
        assert_eq!(fs::read(&a).unwrap(), b"three");
        assert_eq!(fs::read(&restored).unwrap(), b"one");
    }

    #[test]
    fn recovers_pending_entries() {
        let (_tmp, desktop, imp) = setup();
        // Simulate a crash after the copy and journal write, before the delete.
        let src = desktop.join("A.url");
        fs::write(&src, b"a").unwrap();
        let stored = imp.shortcuts_dir().join("A.url");
        fs::write(&stored, b"a").unwrap();
        // ... and a crash after the delete, before marking as moved.
        let gone = desktop.join("B.url");
        let stored_b = imp.shortcuts_dir().join("B.url");
        fs::write(&stored_b, b"b").unwrap();
        let journal = Journal {
            entries: vec![
                JournalEntry {
                    id: "a".into(),
                    original_path: src.to_string_lossy().into(),
                    stored_path: stored.to_string_lossy().into(),
                    moved_at: 0,
                    state: EntryState::Pending,
                    desktop_position: None,
                },
                JournalEntry {
                    id: "b".into(),
                    original_path: gone.to_string_lossy().into(),
                    stored_path: stored_b.to_string_lossy().into(),
                    moved_at: 0,
                    state: EntryState::Pending,
                    desktop_position: None,
                },
            ],
        };
        imp.save(&journal).unwrap();

        let report = imp.recover().unwrap();
        assert_eq!(report.rolled_back, vec!["a".to_string()]);
        assert_eq!(report.completed, vec!["b".to_string()]);
        assert!(src.exists() && !stored.exists());
        assert_eq!(imp.entries().unwrap().len(), 1);
        assert_eq!(imp.entry("b").unwrap().state, EntryState::Moved);
    }

    #[test]
    fn keeps_desktop_position() {
        let (_tmp, desktop, imp) = setup();
        let src = desktop.join("Game.lnk");
        fs::write(&src, b"x").unwrap();
        let pos = IconPosition { x: 120, y: 340 };
        let e = imp.move_in_at(&src, Some(pos)).unwrap();
        assert_eq!(imp.entry(&e.id).unwrap().desktop_position, Some(pos));
        imp.restore(&e.id).unwrap();
        assert_eq!(imp.entry(&e.id).unwrap().desktop_position, Some(pos));
    }

    #[test]
    fn reads_old_journal_without_position() {
        let json = r#"{"id":"a","originalPath":"C:/a.lnk","storedPath":"D:/a.lnk","movedAt":1,"state":"moved"}"#;
        let e: JournalEntry = serde_json::from_str(json).unwrap();
        assert_eq!(e.desktop_position, None);
    }

    #[test]
    fn missing_source_fails_cleanly() {
        let (_tmp, desktop, imp) = setup();
        assert!(imp.move_in(&desktop.join("nope.lnk")).is_err());
        assert!(imp.entries().unwrap().is_empty());
    }
}
