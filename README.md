# PRVDS Game Launcher

Ein Game Launcher für Windows 10/11 in Rust und Tauri v2. Er liest die Spiele-Verknüpfungen vom Desktop ein,
verschiebt sie sicher (mit Undo) in einen eigenen Ordner und zeigt die Spiele in einem Cover-Grid mit
Sortierung, Filtern, Sammlungen, gespeicherten Ansichten und Spielzeitmessung.

Der Plan steht in [docs/PLAN.md](docs/PLAN.md).

## Aufbau

| Ordner | Inhalt |
|---|---|
| `crates/launcher-core` | Kernlogik ohne GUI: Desktop-Scan, sicheres Verschieben, SQLite-Bibliothek, Filter/Sortierung, Cover/Metadaten, Spielzeit |
| `src-tauri` | Tauri-App: Commands, App-Zustand, Prozessüberwachung |
| `src` | Oberfläche (SvelteKit + TypeScript) |

Daten liegen unter `%APPDATA%\PRVDS\GameLauncher` (Datenbank, verschobene Verknüpfungen, Journal, Cover).

Beim Import merkt sich die App neben dem Originalpfad auch die Position jedes Icons auf dem Desktop.
„Zurück auf den Desktop“ legt die Verknüpfung wieder an dieselbe Stelle (außer wenn „Automatisch anordnen“ aktiv ist).

API-Keys für SteamGridDB und IGDB werden in der Windows-Anmeldeinformationsverwaltung gespeichert.

## Entwicklung

Voraussetzungen: Rust (stable), Node.js 22, unter Windows WebView2 (bei Windows 10/11 vorhanden).

```bash
npm install
npm run tauri dev      # App im Entwicklungsmodus
cargo test -p launcher-core
npm run check          # Typprüfung der Oberfläche
npm run tauri build    # Installer (NSIS/MSI) unter target/release/bundle
```

## Branches

Git Flow: Entwicklung auf `dev`, Releases über `release/*` nach `main`.

## Versionen und Builds

Die Version steht nur in `package.json`; `node scripts/version.mjs set 0.3.0` überträgt sie in `Cargo.toml`,
die CI prüft, dass beide übereinstimmen. Bis zur ersten stabilen Version zählen wir `0.x` hoch.

| Build | Auslöser | Datei |
| --- | --- | --- |
| Dev | jeder Push auf `dev` (oder manuell) | `PRVDS-Game-Launcher_0.2.0-dev.<Build-Nr.>_x64-setup.exe` |
| Release | Tag `v0.2.0` auf `main` | `PRVDS-Game-Launcher_0.2.0_x64-setup.exe` und `.msi` |

Die Installer liegen als Artefakt am jeweiligen Lauf von „Windows installer“ unter GitHub Actions.
Die installierte Version steht in der App unten in den Einstellungen.
