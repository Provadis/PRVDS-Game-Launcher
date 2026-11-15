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
