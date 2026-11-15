//! End-to-end flow without GUI: scan a desktop, import the games, query the
//! library, record a session, undo everything.

use std::fs;
use std::path::Path;

use launcher_core::importer::Importer;
use launcher_core::library::Library;
use launcher_core::locate::locate;
use launcher_core::model::{NewGame, Platform};
use launcher_core::query::{GameQuery, SortField};
use launcher_core::scanner::{ShortcutKind, scan_dirs};

#[test]
fn desktop_to_library_and_back() {
    let tmp = tempfile::tempdir().unwrap();
    let desktop = tmp.path().join("Desktop");
    fs::create_dir_all(&desktop).unwrap();
    fs::write(
        desktop.join("Elden Ring.url"),
        "[InternetShortcut]\r\nURL=steam://rungameid/1245620\r\n",
    )
    .unwrap();
    fs::write(
        desktop.join("Overwatch.url"),
        "[InternetShortcut]\r\nURL=battlenet://Pro\r\n",
    )
    .unwrap();
    fs::write(
        desktop.join("Wiki.url"),
        "[InternetShortcut]\r\nURL=https://example.org\r\n",
    )
    .unwrap();
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/Hearthstone.lnk");
    fs::copy(&fixture, desktop.join("Hearthstone.lnk")).unwrap();

    let found = scan_dirs(std::slice::from_ref(&desktop));
    assert_eq!(found.len(), 4);
    let games: Vec<_> = found.iter().filter(|c| c.likely_game).collect();
    let names: Vec<&str> = games.iter().map(|c| c.title.as_str()).collect();
    assert_eq!(names, ["Elden Ring", "Hearthstone", "Overwatch"]);

    let data = tmp.path().join("data");
    let importer = Importer::new(data.join("Shortcuts"), data.join("journal.json")).unwrap();
    let lib = Library::open(&data.join("library.db")).unwrap();

    for c in &games {
        let entry = importer.move_in(Path::new(&c.path)).unwrap();
        let target = c
            .info
            .target
            .as_deref()
            .filter(|_| c.kind != ShortcutKind::Url);
        let install = locate(c.platform, c.platform_id.as_deref(), target, &c.title);
        lib.add_game(&NewGame {
            name: c.title.clone(),
            platform: Some(c.platform),
            platform_id: c.platform_id.clone(),
            shortcut_path: entry.stored_path,
            original_path: Some(entry.original_path),
            journal_id: Some(entry.id),
            launch_target: c.info.target.clone(),
            install_dir: install.install_dir,
            process_exe: install.process_exe,
        })
        .unwrap();
    }

    // Only the web link is left on the desktop.
    let left: Vec<_> = fs::read_dir(&desktop)
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    assert_eq!(left, ["Wiki.url"]);

    let all = lib.games().unwrap();
    assert_eq!(all.len(), 3);
    let hs = all.iter().find(|g| g.name == "Hearthstone").unwrap();
    assert_eq!(hs.install_dir.as_deref(), Some(r"E:\Hearthstone"));
    let ow = all.iter().find(|g| g.name == "Overwatch").unwrap();
    assert_eq!(ow.platform, Platform::Battlenet);
    assert_eq!(ow.platform_id.as_deref(), Some("Pro"));

    // Play a session, then sort by playtime.
    lib.add_session(ow.id, 1_000, 8_200).unwrap();
    let q = GameQuery {
        sort: SortField::Playtime,
        descending: true,
        ..Default::default()
    };
    assert_eq!(q.apply(&lib.games().unwrap(), 10_000)[0].name, "Overwatch");

    // Undo every import.
    for g in lib.games().unwrap() {
        importer.restore(g.journal_id.as_deref().unwrap()).unwrap();
        lib.delete_game(g.id).unwrap();
    }
    let mut back: Vec<_> = fs::read_dir(&desktop)
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .collect();
    back.sort();
    assert_eq!(
        back,
        [
            "Elden Ring.url",
            "Hearthstone.lnk",
            "Overwatch.url",
            "Wiki.url"
        ]
    );
    assert_eq!(
        fs::read(desktop.join("Hearthstone.lnk")).unwrap(),
        fs::read(&fixture).unwrap()
    );
    assert!(
        fs::read_dir(importer.shortcuts_dir())
            .unwrap()
            .next()
            .is_none()
    );
}
