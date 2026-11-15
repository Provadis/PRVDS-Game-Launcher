//! API keys. On Windows they live in the Windows Credential Manager, never in
//! a file. Elsewhere (development only) they go to a JSON file next to the
//! database.

use std::path::{Path, PathBuf};

use crate::artwork::ArtworkKeys;
use crate::error::Result;

pub struct SecretStore {
    #[allow(dead_code)]
    fallback_file: PathBuf,
}

#[cfg(windows)]
const SERVICE: &str = "PRVDS Game Launcher";

impl SecretStore {
    pub fn new(data_dir: &Path) -> Self {
        Self {
            fallback_file: data_dir.join("dev-secrets.json"),
        }
    }

    #[cfg(windows)]
    pub fn load(&self) -> ArtworkKeys {
        let get = |k: &str| {
            keyring::Entry::new(SERVICE, k)
                .ok()
                .and_then(|e| e.get_password().ok())
                .filter(|s| !s.is_empty())
        };
        ArtworkKeys {
            steamgriddb: get("steamgriddb"),
            igdb_client_id: get("igdb_client_id"),
            igdb_client_secret: get("igdb_client_secret"),
        }
    }

    #[cfg(windows)]
    pub fn save(&self, keys: &ArtworkKeys) -> Result<()> {
        use crate::error::Error;
        let set = |k: &str, v: &Option<String>| -> Result<()> {
            let entry = keyring::Entry::new(SERVICE, k).map_err(|e| Error::Other(e.to_string()))?;
            match v.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
                Some(v) => entry
                    .set_password(v)
                    .map_err(|e| Error::Other(e.to_string())),
                None => match entry.delete_credential() {
                    Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
                    Err(e) => Err(Error::Other(e.to_string())),
                },
            }
        };
        set("steamgriddb", &keys.steamgriddb)?;
        set("igdb_client_id", &keys.igdb_client_id)?;
        set("igdb_client_secret", &keys.igdb_client_secret)?;
        Ok(())
    }

    #[cfg(not(windows))]
    pub fn load(&self) -> ArtworkKeys {
        std::fs::read(&self.fallback_file)
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default()
    }

    #[cfg(not(windows))]
    pub fn save(&self, keys: &ArtworkKeys) -> Result<()> {
        use crate::error::IoContext;
        std::fs::write(&self.fallback_file, serde_json::to_vec_pretty(keys)?)
            .ctx(&self.fallback_file)
    }
}
