use std::path::PathBuf;
use std::sync::Mutex;

use launcher_core::artwork::ArtClient;
use launcher_core::importer::Importer;
use launcher_core::library::Library;
use launcher_core::playtime::Tracker;
use launcher_core::secrets::SecretStore;

pub struct AppState {
    pub lib: Mutex<Library>,
    pub importer: Importer,
    pub art: ArtClient,
    pub secrets: SecretStore,
    pub tracker: Mutex<Tracker>,
    pub data_dir: PathBuf,
}

impl AppState {
    pub fn init() -> Result<Self, Box<dyn std::error::Error>> {
        // %APPDATA%\PRVDS\GameLauncher on Windows.
        let data_dir = dirs::data_dir()
            .ok_or("Kein Datenordner gefunden")?
            .join("PRVDS")
            .join("GameLauncher");
        std::fs::create_dir_all(data_dir.join("covers"))?;
        let importer = Importer::new(data_dir.join("Shortcuts"), data_dir.join("journal.json"))?;
        let report = importer.recover()?;
        if !report.completed.is_empty() || !report.rolled_back.is_empty() {
            eprintln!("Import-Journal repariert: {report:?}");
        }
        let lib = Library::open(&data_dir.join("library.db"))?;
        let secrets = SecretStore::new(&data_dir);
        let art = ArtClient::new(secrets.load());
        Ok(Self {
            lib: Mutex::new(lib),
            importer,
            art,
            secrets,
            tracker: Mutex::new(Tracker::new()),
            data_dir,
        })
    }

    pub fn covers_dir(&self) -> PathBuf {
        self.data_dir.join("covers")
    }
}
