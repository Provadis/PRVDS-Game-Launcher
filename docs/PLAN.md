# PRVDS Game Launcher – Plan v1

Ein Game Launcher für Windows 10/11, geschrieben in Rust mit Tauri v2. Ziel: den überfüllten Desktop aufräumen, ohne die Spiele-Verknüpfungen zu verlieren, und alle Spiele in einem schönen „Game Space“ mit Covern zu verwalten.

## Entscheidungen

| Thema | Entscheidung |
|---|---|
| Betriebssystem | Nur Windows 10/11 |
| GUI | Tauri v2 (Rust-Backend, Web-Frontend) |
| Plattformen | Steam, Epic, Ubisoft Connect, EA App, Battle.net (plus lose `.exe`) |
| Öffentlicher Desktop | Wird nicht benötigt (keine Adminrechte nötig) |
| Cover/Metadaten | Steam-CDN, SteamGridDB und IGDB |
| Import aus Launchern | Ja, installierte Spiele auch direkt aus den Launchern (nach v1-Kern) |
| Spielzeit | Wird gemessen (Prozessüberwachung) |
| Optik | Modern, minimalistisch, lebt von den Covern; Mischung aus Steam und Playnite |
| Open Source | Ja, aber später (API-Keys daher von Anfang an nicht im Code) |

## Grundidee

1. Der Launcher liest den Desktop ein und erkennt Spiele-Verknüpfungen.
2. Der Nutzer wählt die Spiele aus.
3. Die Verknüpfungen werden in den Bibliotheksordner verschoben (`%APPDATA%\PRVDS\GameLauncher\Shortcuts`).
4. Gestartet wird später die verschobene Verknüpfung selbst. Argumente, Arbeitsordner und Launcher-Links (`steam://`, `com.epicgames.launcher://` usw.) bleiben dadurch unverändert gültig.

## Architektur

```
src-tauri/
  scanner/    Desktop einlesen, .lnk/.url/.exe parsen, Plattform erkennen
  importer/   Sicher verschieben, Journal, Undo
  library/    SQLite (rusqlite), Spiele, Tags, Sammlungen, Ansichten
  launcher/   Starten über ShellExecute, Spielzeit messen
  artwork/    Cover-Download, Cache, Icon-Fallback
  sources/    (später) Import direkt aus Steam/Epic/Ubisoft/EA/Battle.net
  commands.rs Tauri-Commands für das Frontend
ui/           Frontend (Svelte + TypeScript, Vite)
```

### scanner
- Desktop-Pfad über die Known-Folder-API (`FOLDERID_Desktop`), damit ein von OneDrive umgeleiteter Desktop gefunden wird.
- `.lnk`: Ziel, Argumente, Arbeitsordner und Icon auslesen (`lnk`-Crate oder `IShellLinkW` über die `windows`-Crate).
- `.url`: INI-Format, `URL=` und `IconFile=` auslesen.
- Plattform-Erkennung:
  - Steam: `steam://rungameid/<AppID>` → AppID speichern
  - Epic: `com.epicgames.launcher://apps/<id>?action=launch`
  - Ubisoft: `uplay://launch/<id>`
  - EA App: `origin2://` bzw. `link2ea://`
  - Battle.net: `battlenet://` oder `.lnk` auf `Battle.net Launcher.exe --exec=...`
  - Sonst: `.lnk` auf eine `.exe` → „Sonstige“
- Nicht-Spiele (Browser, Office usw.) werden vorgeschlagen, aber nicht vorausgewählt; der Nutzer entscheidet.

### importer (sicheres Verschieben)
1. Datei in den Bibliotheksordner kopieren.
2. Kopie prüfen (Größe, Hash).
3. Eintrag ins Journal schreiben (`journal.json`: Originalpfad, neuer Pfad, Zeitpunkt).
4. Erst dann das Original löschen.

Funktionen: „Rückgängig“ pro Spiel, „Alles zurück auf den Desktop“, und beim Start ein Abgleich, falls ein Vorgang abgebrochen wurde. Namenskonflikte werden durch Suffixe gelöst.

### library
SQLite-Datenbank mit:
- `games`: Name, Plattform, Plattform-ID, Pfad zur Verknüpfung, Cover-Pfad, hinzugefügt am, zuletzt gespielt, Anzahl Starts, Spielzeit, Favorit, versteckt
- `metadata`: Genre, Erscheinungsjahr, Entwickler, Beschreibung (aus IGDB)
- `tags`, `game_tags`
- `collections`, `collection_games` (manuelle Sammlungen)
- `views` (gespeicherte Ansichten: Filter, Sortierung, Layout als JSON)
- `sessions` (Spielsitzungen: Start, Ende, Dauer)

### launcher
- Start über `ShellExecuteW` auf die gespeicherte Verknüpfung.
- Spielzeit: Nach dem Start den Spielprozess erkennen und überwachen. Weil Launcher wie Steam oder Battle.net den eigentlichen Prozess indirekt starten, wird der Prozess über den Installationsordner bzw. den bekannten Exe-Namen gesucht (Polling über `sysinfo` oder WMI). Fallback: der Nutzer kann die Exe im Spiel-Detail festlegen.
- Optional später: Tray-Icon, damit die Messung auch bei geschlossenem Fenster weiterläuft.

### artwork
Reihenfolge der Quellen:
1. Steam-CDN (`library_600x900.jpg`) für Steam-Spiele, ohne Key.
2. SteamGridDB (API-Key) für alle anderen, große Auswahl an Grids, Heroes und Logos.
3. IGDB (Twitch Client-ID/Secret) für Metadaten und als weitere Cover-Quelle.
4. Fallback: Icon aus der `.exe` bzw. `IconFile`.

Zuordnung über Plattform-ID wo möglich, sonst unscharfe Namenssuche. Der Nutzer kann das Cover jederzeit wechseln oder ein eigenes Bild per Drag & Drop setzen. Bilder werden lokal gecacht.

API-Keys trägt der Nutzer in den Einstellungen ein; sie werden im Windows Credential Manager gespeichert (`keyring`-Crate), nie im Repo.

### Import aus Launchern (nach dem v1-Kern)
- Steam: `libraryfolders.vdf` und `appmanifest_*.acf`
- Epic: Manifeste in `%ProgramData%\Epic\EpicGamesLauncher\Data\Manifests`
- Ubisoft, EA, Battle.net: Registry bzw. Konfigurationsdateien der Launcher

## UI-Konzept

Modern, minimalistisch, dunkles Theme, die Cover tragen die Optik.

- **Bibliothek (Grid)**: Cover im Hochformat (600×900), sanfte Hover-Animation, Play-Button beim Hover. Einstellbare Covergröße.
- **Listenansicht**: kompakt mit kleinem Cover, Plattform, Spielzeit, zuletzt gespielt.
- **Seitenleiste**: Alle Spiele, Favoriten, Zuletzt gespielt, Sammlungen, gespeicherte Ansichten.
- **Toolbar**: Suche, Sortierung, Filter (Plattform, Tags, Genre, gespielt/ungespielt), Layout-Umschalter.
- **Spiel-Detail**: großes Hero-Bild, Spielzeit, Sitzungen, Tags, Cover wechseln.
- **Import-Assistent**: Desktop scannen → Spiele auswählen (Cover-Vorschau) → verschieben → fertig, mit Undo-Hinweis.

### Sortierung
Name, Plattform, hinzugefügt am, zuletzt gespielt, Spielzeit, Anzahl Starts, Erscheinungsjahr, Genre.

### Custom-Ansichten
- **Sammlungen**: manuell zusammengestellt („Mit Freunden“, „Durchgespielt“).
- **Gespeicherte Ansichten**: Kombination aus Filter, Sortierung und Layout, z.B. „Steam, seit 3 Monaten nicht gespielt“.

## Meilensteine

1. **Grundgerüst und Import**: Tauri-Projekt, Desktop scannen, Spiele auswählen, sicher verschieben mit Undo, einfache Liste, Spiel starten.
2. **Bibliothek**: SQLite, Sortieren, Suchen, Filter, Favoriten, Tags.
3. **Cover-Grid**: Steam-CDN, SteamGridDB, IGDB, Cache, Cover wechseln.
4. **Spielzeit**: Prozessüberwachung, Sitzungen, Statistik im Detail.
5. **Ansichten**: Sammlungen und gespeicherte Ansichten.
6. **Launcher-Import**: Steam, Epic, Ubisoft, EA, Battle.net direkt einlesen.
7. **Auslieferung**: Installer (NSIS/MSI), Auto-Update, optional Tray und Autostart.

## Offene Punkte für später
- Lizenz und Veröffentlichung als Open Source.
- Synchronisation zwischen mehreren PCs.
- Controller-Bedienung / Big-Picture-Modus.
