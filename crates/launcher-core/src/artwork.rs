//! Cover art and metadata.
//!
//! Official store art comes first, so a game looks the way its store shows it:
//! 1. Steam CDN and Steam store API for Steam games (no key needed),
//! 2. the Epic launcher's own catalog cache for Epic games (local, no key),
//! 3. Steam again for games from other launchers that are also on Steam,
//!    found by exact name (no key),
//! 4. IGDB (Twitch client id + secret): official box art, plus metadata,
//! 5. SteamGridDB (API key): community art for everything else.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, RwLock};
use std::time::Duration;
use std::time::SystemTime;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::{Error, IoContext, Result};
use crate::model::{Game, Platform};
use crate::util::{normalize_name, now};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtworkKeys {
    pub steamgriddb: Option<String>,
    pub igdb_client_id: Option<String>,
    pub igdb_client_secret: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ArtKind {
    Cover,
    Hero,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtOption {
    pub url: String,
    pub thumb: String,
    pub source: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Metadata {
    pub genre: Option<String>,
    pub release_year: Option<i32>,
    pub developer: Option<String>,
    pub summary: Option<String>,
}

pub struct ArtClient {
    http: reqwest::blocking::Client,
    keys: RwLock<ArtworkKeys>,
    igdb_token: Mutex<Option<(String, i64)>>,
    /// Epic catalog images by catalog item id, with the cache file's mtime.
    epic_catalog: Mutex<Option<(SystemTime, EpicCatalog)>>,
}

/// Key images per catalog item id, and per namespace as a fallback.
#[derive(Debug, Default, Clone)]
pub struct EpicCatalog {
    by_id: HashMap<String, Vec<(String, String)>>,
    by_namespace: HashMap<String, Vec<(String, String)>>,
}

const SGDB: &str = "https://www.steamgriddb.com/api/v2";

impl ArtClient {
    pub fn new(keys: ArtworkKeys) -> Self {
        let http = reqwest::blocking::Client::builder()
            .user_agent(concat!("PRVDS-Game-Launcher/", env!("CARGO_PKG_VERSION")))
            .timeout(Duration::from_secs(20))
            .build()
            .expect("http client");
        Self {
            http,
            keys: RwLock::new(keys),
            igdb_token: Mutex::new(None),
            epic_catalog: Mutex::new(None),
        }
    }

    pub fn set_keys(&self, keys: ArtworkKeys) {
        *self.keys.write().unwrap() = keys;
        *self.igdb_token.lock().unwrap() = None;
    }

    fn keys(&self) -> ArtworkKeys {
        self.keys.read().unwrap().clone()
    }

    fn steam_id(game: &Game) -> Option<&str> {
        (game.platform == Platform::Steam)
            .then_some(game.platform_id.as_deref())
            .flatten()
    }

    /// Official store art: Steam for Steam games, the Epic catalog for Epic
    /// games, and Steam by exact name for games from other launchers.
    /// `all` = false stops at the first hit.
    fn official(&self, game: &Game, kind: ArtKind, all: bool) -> Vec<ArtOption> {
        let mut out = Vec::new();
        let steam = |appid: &str, out: &mut Vec<ArtOption>| {
            if let Some(url) = self
                .steam_urls(appid, kind)
                .into_iter()
                .find(|u| self.exists(u))
            {
                out.push(ArtOption {
                    thumb: url.clone(),
                    url,
                    source: "Steam".into(),
                });
            }
        };
        if let Some(appid) = Self::steam_id(game) {
            steam(appid, &mut out);
            return out;
        }
        if game.platform == Platform::Epic {
            out.extend(self.epic_art(game, kind));
            if !all && !out.is_empty() {
                out.truncate(1);
                return out;
            }
        }
        if let Some(appid) = self.steam_appid_by_name(&game.name) {
            steam(&appid, &mut out);
        }
        out
    }

    /// All cover or hero options for the picker, best first.
    pub fn options(&self, game: &Game, kind: ArtKind) -> Vec<ArtOption> {
        let mut out = self.official(game, kind, true);
        out.extend(self.igdb_art(game, kind).unwrap_or_default());
        out.extend(self.sgdb_options(game, kind).unwrap_or_default());
        let mut seen = std::collections::HashSet::new();
        out.retain(|o| seen.insert(o.url.clone()));
        out
    }

    pub fn best(&self, game: &Game, kind: ArtKind) -> Option<ArtOption> {
        if let Some(o) = self.official(game, kind, false).into_iter().next() {
            return Some(o);
        }
        // IGDB has the publisher's box art; its hero images are often
        // screenshots, so for heroes SteamGridDB goes first.
        let igdb = || {
            self.igdb_art(game, kind)
                .ok()
                .and_then(|v| v.into_iter().next())
        };
        let sgdb = || {
            self.sgdb_options(game, kind)
                .ok()
                .and_then(|v| v.into_iter().next())
        };
        match kind {
            ArtKind::Cover => igdb().or_else(sgdb),
            ArtKind::Hero => sgdb().or_else(igdb),
        }
    }

    // ---- Epic -----------------------------------------------------------------

    fn epic_art(&self, game: &Game, kind: ArtKind) -> Vec<ArtOption> {
        let Some(id) = game.platform_id.as_deref() else {
            return vec![];
        };
        let Some(catalog) = self.epic_catalog() else {
            return vec![];
        };
        epic_images(&catalog, id, kind)
            .into_iter()
            .map(|url| ArtOption {
                thumb: url.clone(),
                url,
                source: "Epic".into(),
            })
            .collect()
    }

    /// The Epic launcher's catalog cache, re-read when the file changes.
    fn epic_catalog(&self) -> Option<EpicCatalog> {
        let path = epic_catalog_path()?;
        let mtime = std::fs::metadata(&path).and_then(|m| m.modified()).ok()?;
        let mut guard = self.epic_catalog.lock().unwrap();
        if let Some((t, c)) = guard.as_ref()
            && *t == mtime
        {
            return Some(c.clone());
        }
        let catalog = parse_epic_catalog(&std::fs::read(&path).ok()?)?;
        *guard = Some((mtime, catalog.clone()));
        Some(catalog)
    }

    // ---- Steam ------------------------------------------------------------------

    /// The Steam app with exactly this name, if the game is also on Steam.
    fn steam_appid_by_name(&self, name: &str) -> Option<String> {
        let v: Value = self
            .http
            .get("https://store.steampowered.com/api/storesearch/")
            .query(&[("term", name), ("l", "german"), ("cc", "DE")])
            .send()
            .ok()?
            .error_for_status()
            .ok()?
            .json()
            .ok()?;
        pick_steam_search(&v, name)
    }

    /// Candidate Steam image URLs, best first: the real asset paths from the
    /// store API (newer games keep their art under a hashed folder), then the
    /// classic fixed paths.
    fn steam_urls(&self, appid: &str, kind: ArtKind) -> Vec<String> {
        let mut urls = self
            .steam_store_item(appid)
            .map(|item| store_asset_urls(&item, kind))
            .unwrap_or_default();
        for u in steam_cdn_urls(appid, kind) {
            if !urls.contains(&u) {
                urls.push(u);
            }
        }
        urls
    }

    /// The store item (with asset paths) from Steam's public store API.
    fn steam_store_item(&self, appid: &str) -> Option<Value> {
        let id: u64 = appid.parse().ok()?;
        let input = serde_json::json!({
            "ids": [{ "appid": id }],
            "context": { "language": "german", "country_code": "DE" },
            "data_request": { "include_assets": true }
        });
        let v: Value = self
            .http
            .get("https://api.steampowered.com/IStoreBrowseService/GetItems/v1/")
            .query(&[("input_json", input.to_string())])
            .send()
            .ok()?
            .error_for_status()
            .ok()?
            .json()
            .ok()?;
        v["response"]["store_items"]
            .as_array()?
            .iter()
            .find(|i| i["appid"].as_u64() == Some(id))
            .cloned()
    }

    fn exists(&self, url: &str) -> bool {
        self.http
            .head(url)
            .send()
            .map(|r| r.status().is_success())
            .unwrap_or(false)
    }

    // ---- SteamGridDB --------------------------------------------------------

    fn sgdb_get(&self, key: &str, path: &str) -> Result<Value> {
        let v: Value = self
            .http
            .get(format!("{SGDB}{path}"))
            .bearer_auth(key)
            .send()?
            .error_for_status()?
            .json()?;
        Ok(v)
    }

    fn sgdb_game_id(&self, key: &str, game: &Game) -> Result<Option<i64>> {
        if let Some(appid) = Self::steam_id(game)
            && let Ok(v) = self.sgdb_get(key, &format!("/games/steam/{appid}"))
            && let Some(id) = v["data"]["id"].as_i64()
        {
            return Ok(Some(id));
        }
        let term =
            percent_encoding::utf8_percent_encode(&game.name, percent_encoding::NON_ALPHANUMERIC)
                .to_string();
        let v = self.sgdb_get(key, &format!("/search/autocomplete/{term}"))?;
        Ok(pick_sgdb_game(&v, &game.name))
    }

    fn sgdb_options(&self, game: &Game, kind: ArtKind) -> Result<Vec<ArtOption>> {
        let Some(key) = self.keys().steamgriddb.filter(|k| !k.trim().is_empty()) else {
            return Ok(vec![]);
        };
        let Some(id) = self.sgdb_game_id(&key, game)? else {
            return Ok(vec![]);
        };
        let path = match kind {
            ArtKind::Cover => format!(
                "/grids/game/{id}?dimensions=600x900,342x482,660x930&types=static&nsfw=false&humor=false"
            ),
            ArtKind::Hero => format!("/heroes/game/{id}?types=static&nsfw=false&humor=false"),
        };
        let v = self.sgdb_get(&key, &path)?;
        Ok(parse_sgdb_images(&v))
    }

    // ---- IGDB ---------------------------------------------------------------

    fn igdb_token(&self, id: &str, secret: &str) -> Result<String> {
        let mut guard = self.igdb_token.lock().unwrap();
        if let Some((tok, exp)) = guard.as_ref()
            && *exp > now() + 60
        {
            return Ok(tok.clone());
        }
        let v: Value = self
            .http
            .post("https://id.twitch.tv/oauth2/token")
            .query(&[
                ("client_id", id),
                ("client_secret", secret),
                ("grant_type", "client_credentials"),
            ])
            .send()?
            .error_for_status()?
            .json()?;
        let tok = v["access_token"]
            .as_str()
            .ok_or_else(|| Error::Other("IGDB: kein Token".into()))?
            .to_string();
        let exp = now() + v["expires_in"].as_i64().unwrap_or(3600);
        *guard = Some((tok.clone(), exp));
        Ok(tok)
    }

    fn igdb_game(&self, game: &Game) -> Result<Option<Value>> {
        let keys = self.keys();
        let (Some(id), Some(secret)) = (
            keys.igdb_client_id.filter(|s| !s.trim().is_empty()),
            keys.igdb_client_secret.filter(|s| !s.trim().is_empty()),
        ) else {
            return Ok(None);
        };
        let token = self.igdb_token(&id, &secret)?;
        let name = game.name.replace('"', "");
        let body = format!(
            "search \"{name}\"; fields name,first_release_date,genres.name,cover.image_id,artworks.image_id,\
             involved_companies.company.name,involved_companies.developer,summary; limit 10;"
        );
        let v: Value = self
            .http
            .post("https://api.igdb.com/v4/games")
            .header("Client-ID", id)
            .bearer_auth(token)
            .body(body)
            .send()?
            .error_for_status()?
            .json()?;
        Ok(pick_igdb_game(&v, &game.name))
    }

    fn igdb_art(&self, game: &Game, kind: ArtKind) -> Result<Vec<ArtOption>> {
        let Some(g) = self.igdb_game(game)? else {
            return Ok(vec![]);
        };
        Ok(igdb_images(&g, kind))
    }

    // ---- Metadata -------------------------------------------------------------

    pub fn metadata(&self, game: &Game) -> Option<Metadata> {
        let appid = Self::steam_id(game)
            .map(str::to_string)
            .or_else(|| self.steam_appid_by_name(&game.name));
        if let Some(appid) = appid.as_deref() {
            let url =
                format!("https://store.steampowered.com/api/appdetails?appids={appid}&l=german");
            if let Ok(v) = self.http.get(url).send().and_then(|r| r.json::<Value>())
                && let Some(m) = parse_steam_appdetails(&v, appid)
            {
                return Some(m);
            }
        }
        self.igdb_game(game)
            .ok()
            .flatten()
            .map(|g| igdb_metadata(&g))
    }

    /// Downloads an image to `dest`, scaled down to a sensible size for
    /// `kind` (see [`fit_image`]).
    pub fn download(&self, url: &str, dest: &Path, kind: ArtKind) -> Result<()> {
        let resp = self.http.get(url).send()?.error_for_status()?;
        let is_image = resp
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .is_none_or(|ct| ct.starts_with("image/") || ct == "application/octet-stream");
        if !is_image {
            return Err(Error::Other(format!("Kein Bild: {url}")));
        }
        let bytes = resp.bytes()?;
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent).ctx(parent)?;
        }
        let bytes = fit_image(&bytes, dest, kind).unwrap_or_else(|| bytes.to_vec());
        std::fs::write(dest, &bytes).ctx(dest)?;
        Ok(())
    }
}

/// Scales down an already stored image in place if it is larger than
/// needed (covers saved before downscaling existed). Returns whether it
/// changed.
pub fn shrink_file(path: &Path, kind: ArtKind) -> bool {
    let (max_w, max_h) = max_size(kind);
    match image::image_dimensions(path) {
        Ok((w, h)) if w > max_w || h > max_h => {}
        _ => return false,
    }
    let Ok(bytes) = std::fs::read(path) else {
        return false;
    };
    match fit_image(&bytes, path, kind) {
        Some(out) => std::fs::write(path, out).is_ok(),
        None => false,
    }
}

/// Largest stored size: covers are shown at most ~300 px wide, so twice
/// that stays sharp on high-DPI screens. Heroes are the big background of
/// a game's page and keep their full resolution up to 4K.
fn max_size(kind: ArtKind) -> (u32, u32) {
    match kind {
        ArtKind::Cover => (600, 900),
        ArtKind::Hero => (3840, 2160),
    }
}

/// Scales an image down when it is larger than needed. The webview shrinks
/// very large images (Epic's box art is 1200×1600 and more) poorly, and
/// they look grainy at cover size; a clean downscale here avoids that.
/// Returns the re-encoded bytes in the format `dest` asks for, or `None`
/// to keep the original (small enough, or not decodable, like GIFs).
pub fn fit_image(bytes: &[u8], dest: &Path, kind: ArtKind) -> Option<Vec<u8>> {
    use image::ImageFormat;
    let (max_w, max_h) = max_size(kind);
    let img = image::load_from_memory(bytes).ok()?;
    if img.width() <= max_w && img.height() <= max_h {
        return None;
    }
    let img = img.resize(max_w, max_h, image::imageops::FilterType::Lanczos3);
    let format = ImageFormat::from_path(dest).ok()?;
    let mut out = std::io::Cursor::new(Vec::new());
    match format {
        ImageFormat::Jpeg => {
            let enc = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, 90);
            img.to_rgb8().write_with_encoder(enc).ok()?;
        }
        ImageFormat::Png | ImageFormat::WebP => img.write_to(&mut out, format).ok()?,
        _ => return None,
    }
    Some(out.into_inner())
}

fn epic_catalog_path() -> Option<PathBuf> {
    let base = std::env::var_os("ProgramData").map(PathBuf::from)?;
    Some(base.join(r"Epic\EpicGamesLauncher\Data\Catalog\catcache.bin"))
}

/// Reads `catcache.bin`: base64 of a JSON array of catalog items, each with
/// `id`, `namespace` and `keyImages: [{type, url}]`.
pub fn parse_epic_catalog(bytes: &[u8]) -> Option<EpicCatalog> {
    use base64::Engine;
    let text: Vec<u8> = bytes
        .iter()
        .copied()
        .filter(|b| !b.is_ascii_whitespace())
        .collect();
    let json = base64::engine::general_purpose::STANDARD
        .decode(&text)
        .ok()
        .unwrap_or_else(|| bytes.to_vec());
    let items: Vec<Value> = serde_json::from_slice(&json).ok()?;
    let mut catalog = EpicCatalog::default();
    for item in items {
        let images: Vec<(String, String)> = item["keyImages"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|i| {
                        Some((
                            i["type"].as_str()?.to_string(),
                            i["url"].as_str()?.to_string(),
                        ))
                    })
                    .collect()
            })
            .unwrap_or_default();
        if images.is_empty() {
            continue;
        }
        if let Some(ns) = item["namespace"].as_str() {
            catalog
                .by_namespace
                .entry(ns.to_string())
                .or_insert_with(|| images.clone());
        }
        if let Some(id) = item["id"].as_str() {
            catalog.by_id.insert(id.to_string(), images);
        }
    }
    Some(catalog)
}

/// Image URLs for an Epic game (`namespace:catalogItemId:appName`), best first.
pub fn epic_images(catalog: &EpicCatalog, platform_id: &str, kind: ArtKind) -> Vec<String> {
    let mut parts = platform_id.split(':');
    let (ns, id) = (parts.next().unwrap_or(""), parts.next().unwrap_or(""));
    let Some(images) = catalog
        .by_id
        .get(id)
        .or_else(|| catalog.by_namespace.get(ns))
    else {
        return vec![];
    };
    let wanted: &[&str] = match kind {
        ArtKind::Cover => &["DieselGameBoxTall", "OfferImageTall", "Thumbnail"],
        ArtKind::Hero => &["DieselGameBox", "OfferImageWide", "DieselStoreFrontWide"],
    };
    wanted
        .iter()
        .filter_map(|w| images.iter().find(|(t, _)| t == w))
        .map(|(_, u)| u.clone())
        .collect()
}

/// The app id of the store search hit whose name matches exactly.
pub fn pick_steam_search(v: &Value, name: &str) -> Option<String> {
    let want = normalize_name(name);
    v["items"]
        .as_array()?
        .iter()
        .find(|i| {
            i["name"]
                .as_str()
                .is_some_and(|n| normalize_name(n) == want)
        })
        .and_then(|i| i["id"].as_u64())
        .map(|id| id.to_string())
}

const STEAM_ASSET_BASE: &str = "https://shared.steamstatic.com/store_item_assets/";

/// Image URLs from a `IStoreBrowseService/GetItems` store item, best first.
/// `assets.asset_url_format` looks like `steam/apps/<id>/<hash>/${FILENAME}?t=…`.
pub fn store_asset_urls(item: &Value, kind: ArtKind) -> Vec<String> {
    let assets = &item["assets"];
    let Some(format) = assets["asset_url_format"].as_str() else {
        return vec![];
    };
    let fields: &[&str] = match kind {
        ArtKind::Cover => &["library_capsule_2x", "library_capsule"],
        ArtKind::Hero => &["library_hero_2x", "library_hero"],
    };
    fields
        .iter()
        .filter_map(|f| assets[*f].as_str())
        .filter(|file| !file.is_empty())
        .map(|file| format!("{STEAM_ASSET_BASE}{}", format.replace("${FILENAME}", file)))
        .collect()
}

pub fn steam_cdn_urls(appid: &str, kind: ArtKind) -> Vec<String> {
    let file = match kind {
        ArtKind::Cover => "library_600x900_2x.jpg",
        ArtKind::Hero => "library_hero.jpg",
    };
    vec![
        format!("https://shared.steamstatic.com/store_item_assets/steam/apps/{appid}/{file}"),
        format!("https://cdn.cloudflare.steamstatic.com/steam/apps/{appid}/{file}"),
    ]
}

fn pick_sgdb_game(v: &Value, name: &str) -> Option<i64> {
    let list = v["data"].as_array()?;
    let wanted = normalize_name(name);
    list.iter()
        .find(|g| {
            g["name"]
                .as_str()
                .is_some_and(|n| normalize_name(n) == wanted)
        })
        .or_else(|| {
            list.iter()
                .map(|g| {
                    (
                        g,
                        strsim::jaro_winkler(
                            &wanted,
                            &normalize_name(g["name"].as_str().unwrap_or("")),
                        ),
                    )
                })
                .filter(|(_, s)| *s > 0.85)
                .max_by(|a, b| a.1.total_cmp(&b.1))
                .map(|(g, _)| g)
        })
        .and_then(|g| g["id"].as_i64())
}

fn parse_sgdb_images(v: &Value) -> Vec<ArtOption> {
    v["data"]
        .as_array()
        .map(|list| {
            list.iter()
                .filter_map(|i| {
                    Some(ArtOption {
                        url: i["url"].as_str()?.to_string(),
                        thumb: i["thumb"].as_str().or(i["url"].as_str())?.to_string(),
                        source: "SteamGridDB".into(),
                    })
                })
                .take(24)
                .collect()
        })
        .unwrap_or_default()
}

fn pick_igdb_game(v: &Value, name: &str) -> Option<Value> {
    let list = v.as_array()?;
    let wanted = normalize_name(name);
    list.iter()
        .find(|g| {
            g["name"]
                .as_str()
                .is_some_and(|n| normalize_name(n) == wanted)
        })
        .or_else(|| {
            list.iter()
                .map(|g| {
                    (
                        g,
                        strsim::jaro_winkler(
                            &wanted,
                            &normalize_name(g["name"].as_str().unwrap_or("")),
                        ),
                    )
                })
                .filter(|(_, s)| *s > 0.85)
                .max_by(|a, b| a.1.total_cmp(&b.1))
                .map(|(g, _)| g)
        })
        .cloned()
}

fn igdb_images(g: &Value, kind: ArtKind) -> Vec<ArtOption> {
    let img = |size: &str, id: &str| {
        format!("https://images.igdb.com/igdb/image/upload/t_{size}/{id}.jpg")
    };
    match kind {
        ArtKind::Cover => g["cover"]["image_id"]
            .as_str()
            .map(|id| ArtOption {
                url: img("cover_big_2x", id),
                thumb: img("cover_big", id),
                source: "IGDB".into(),
            })
            .into_iter()
            .collect(),
        ArtKind::Hero => g["artworks"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|x| x["image_id"].as_str())
                    .map(|id| ArtOption {
                        url: img("1080p", id),
                        thumb: img("screenshot_med", id),
                        source: "IGDB".into(),
                    })
                    .collect()
            })
            .unwrap_or_default(),
    }
}

fn year_from_unix(ts: i64) -> i32 {
    // Days since epoch → civil year (Howard Hinnant's algorithm).
    let z = ts.div_euclid(86_400) + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }) as i32
}

fn igdb_metadata(g: &Value) -> Metadata {
    let genres: Vec<&str> = g["genres"]
        .as_array()
        .map(|a| a.iter().filter_map(|x| x["name"].as_str()).collect())
        .unwrap_or_default();
    let developer = g["involved_companies"].as_array().and_then(|a| {
        a.iter()
            .find(|c| c["developer"].as_bool() == Some(true))
            .and_then(|c| c["company"]["name"].as_str())
            .map(str::to_string)
    });
    Metadata {
        genre: (!genres.is_empty()).then(|| genres.join(", ")),
        release_year: g["first_release_date"].as_i64().map(year_from_unix),
        developer,
        summary: g["summary"].as_str().map(str::to_string),
    }
}

fn parse_steam_appdetails(v: &Value, appid: &str) -> Option<Metadata> {
    let d = &v[appid];
    if d["success"].as_bool() != Some(true) {
        return None;
    }
    let d = &d["data"];
    let genres: Vec<&str> = d["genres"]
        .as_array()
        .map(|a| a.iter().filter_map(|x| x["description"].as_str()).collect())
        .unwrap_or_default();
    let release_year = d["release_date"]["date"].as_str().and_then(|s| {
        s.split(|c: char| !c.is_ascii_digit())
            .filter(|p| p.len() == 4)
            .find_map(|p| p.parse().ok())
    });
    Some(Metadata {
        genre: (!genres.is_empty()).then(|| genres.join(", ")),
        release_year,
        developer: d["developers"]
            .as_array()
            .and_then(|a| a.first())
            .and_then(|x| x.as_str())
            .map(str::to_string),
        summary: d["short_description"]
            .as_str()
            .map(str::to_string)
            .filter(|s| !s.is_empty()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn picks_sgdb_game() {
        let v = json!({"success": true, "data": [
            {"id": 1, "name": "Elden Ring Nightreign"},
            {"id": 2, "name": "ELDEN RING"}
        ]});
        assert_eq!(pick_sgdb_game(&v, "Elden Ring"), Some(2));
        assert_eq!(pick_sgdb_game(&v, "Completely Different"), None);
    }

    #[test]
    fn parses_sgdb_images() {
        let v = json!({"success": true, "data": [{"url": "https://a/full.png", "thumb": "https://a/t.png"}, {"nope": 1}]});
        let r = parse_sgdb_images(&v);
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].thumb, "https://a/t.png");
    }

    #[test]
    fn igdb_parsing() {
        let v = json!([{
            "name": "Diablo IV",
            "first_release_date": 1685998800,
            "genres": [{"name": "Role-playing (RPG)"}, {"name": "Hack and slash/Beat 'em up"}],
            "cover": {"image_id": "co69sm"},
            "artworks": [{"image_id": "ar1"}],
            "involved_companies": [{"developer": false, "company": {"name": "Activision"}}, {"developer": true, "company": {"name": "Blizzard Entertainment"}}],
            "summary": "Hell is back."
        }]);
        let g = pick_igdb_game(&v, "Diablo® IV").unwrap();
        let m = igdb_metadata(&g);
        assert_eq!(m.release_year, Some(2023));
        assert_eq!(m.developer.as_deref(), Some("Blizzard Entertainment"));
        assert_eq!(
            m.genre.as_deref(),
            Some("Role-playing (RPG), Hack and slash/Beat 'em up")
        );
        assert_eq!(
            igdb_images(&g, ArtKind::Cover)[0].url,
            "https://images.igdb.com/igdb/image/upload/t_cover_big_2x/co69sm.jpg"
        );
        assert_eq!(igdb_images(&g, ArtKind::Hero).len(), 1);
    }

    #[test]
    fn steam_store_assets() {
        let item = json!({
            "appid": 2807960,
            "assets": {
                "asset_url_format": "steam/apps/2807960/5a1d4c7e0f2b/${FILENAME}?t=1730000000",
                "library_capsule": "library_600x900.jpg",
                "library_capsule_2x": "library_600x900_2x.jpg",
                "library_hero": "library_hero.jpg"
            }
        });
        assert_eq!(
            store_asset_urls(&item, ArtKind::Cover),
            vec![
                "https://shared.steamstatic.com/store_item_assets/steam/apps/2807960/5a1d4c7e0f2b/library_600x900_2x.jpg?t=1730000000",
                "https://shared.steamstatic.com/store_item_assets/steam/apps/2807960/5a1d4c7e0f2b/library_600x900.jpg?t=1730000000",
            ]
        );
        assert_eq!(store_asset_urls(&item, ArtKind::Hero).len(), 1);
        assert!(store_asset_urls(&json!({"appid": 1}), ArtKind::Cover).is_empty());
    }

    #[test]
    fn epic_catalog_images() {
        use base64::Engine;
        let json = serde_json::json!([
            {
                "id": "4fe75bbc5a674f4f9b356b5c90567da5",
                "namespace": "fn",
                "keyImages": [
                    { "type": "DieselGameBox", "url": "https://cdn1.epicgames.com/wide.jpg" },
                    { "type": "DieselGameBoxTall", "url": "https://cdn1.epicgames.com/tall.jpg" }
                ]
            },
            { "id": "dlc", "namespace": "fn", "keyImages": [] }
        ]);
        let bin = base64::engine::general_purpose::STANDARD.encode(json.to_string());
        let c = parse_epic_catalog(bin.as_bytes()).unwrap();
        let id = "fn:4fe75bbc5a674f4f9b356b5c90567da5:Fortnite";
        assert_eq!(
            epic_images(&c, id, ArtKind::Cover),
            vec!["https://cdn1.epicgames.com/tall.jpg"]
        );
        assert_eq!(
            epic_images(&c, id, ArtKind::Hero),
            vec!["https://cdn1.epicgames.com/wide.jpg"]
        );
        // Unknown item id: falls back to the namespace.
        assert_eq!(epic_images(&c, "fn:other:X", ArtKind::Cover).len(), 1);
        assert!(epic_images(&c, "zz:none:Y", ArtKind::Cover).is_empty());
    }

    #[test]
    fn large_images_are_scaled_down() {
        let big = image::RgbImage::from_pixel(1200, 1600, image::Rgb([200, 40, 90]));
        let mut png = std::io::Cursor::new(Vec::new());
        big.write_to(&mut png, image::ImageFormat::Png).unwrap();
        let out = fit_image(png.get_ref(), Path::new("c.jpg"), ArtKind::Cover).unwrap();
        let img = image::load_from_memory(&out).unwrap();
        assert_eq!((img.width(), img.height()), (600, 800));
        // Small enough already: kept as it is.
        let small = image::RgbImage::from_pixel(600, 900, image::Rgb([0, 0, 0]));
        let mut png = std::io::Cursor::new(Vec::new());
        small.write_to(&mut png, image::ImageFormat::Png).unwrap();
        assert!(fit_image(png.get_ref(), Path::new("c.png"), ArtKind::Cover).is_none());
        assert!(fit_image(b"not an image", Path::new("c.jpg"), ArtKind::Cover).is_none());
    }

    #[test]
    fn steam_search_needs_exact_name() {
        let v = serde_json::json!({ "items": [
            { "type": "app", "name": "A Way Out Soundtrack", "id": 1 },
            { "type": "app", "name": "A Way Out™", "id": 1222700 }
        ]});
        assert_eq!(
            pick_steam_search(&v, "A Way Out").as_deref(),
            Some("1222700")
        );
        assert_eq!(pick_steam_search(&v, "Honkai: Star Rail"), None);
    }

    #[test]
    fn steam_parsing() {
        let v = json!({"1086940": {"success": true, "data": {
            "genres": [{"description": "Abenteuer"}, {"description": "Rollenspiel"}],
            "release_date": {"date": "3. Aug. 2023"},
            "developers": ["Larian Studios"],
            "short_description": "Sammle deine Gruppe."
        }}});
        let m = parse_steam_appdetails(&v, "1086940").unwrap();
        assert_eq!(m.genre.as_deref(), Some("Abenteuer, Rollenspiel"));
        assert_eq!(m.release_year, Some(2023));
        assert_eq!(m.developer.as_deref(), Some("Larian Studios"));
        assert!(parse_steam_appdetails(&json!({"1": {"success": false}}), "1").is_none());
    }

    #[test]
    fn years() {
        assert_eq!(year_from_unix(0), 1970);
        assert_eq!(year_from_unix(1_704_067_199), 2023);
        assert_eq!(year_from_unix(1_704_067_200), 2024);
    }
}
