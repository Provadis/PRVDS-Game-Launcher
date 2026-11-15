//! Cover art and metadata.
//!
//! Sources, in this order:
//! 1. Steam CDN and Steam store API for Steam games (no key needed),
//! 2. SteamGridDB (API key) for covers and hero images of every game,
//! 3. IGDB (Twitch client id + secret) for metadata and as a further cover source.

use std::path::Path;
use std::sync::{Mutex, RwLock};
use std::time::Duration;

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
}

const SGDB: &str = "https://www.steamgriddb.com/api/v2";

impl ArtClient {
    pub fn new(keys: ArtworkKeys) -> Self {
        let http = reqwest::blocking::Client::builder()
            .user_agent("PRVDS-Game-Launcher/0.1")
            .timeout(Duration::from_secs(20))
            .build()
            .expect("http client");
        Self {
            http,
            keys: RwLock::new(keys),
            igdb_token: Mutex::new(None),
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

    /// All cover or hero options for the picker, best first.
    pub fn options(&self, game: &Game, kind: ArtKind) -> Vec<ArtOption> {
        let mut out = Vec::new();
        if let Some(appid) = Self::steam_id(game) {
            out.extend(
                steam_cdn_urls(appid, kind)
                    .into_iter()
                    .filter(|u| self.exists(u))
                    .take(1)
                    .map(|url| ArtOption {
                        thumb: url.clone(),
                        url,
                        source: "Steam".into(),
                    }),
            );
        }
        out.extend(self.sgdb_options(game, kind).unwrap_or_default());
        out.extend(self.igdb_art(game, kind).unwrap_or_default());
        out.dedup_by(|a, b| a.url == b.url);
        out
    }

    pub fn best(&self, game: &Game, kind: ArtKind) -> Option<ArtOption> {
        if let Some(appid) = Self::steam_id(game)
            && let Some(url) = steam_cdn_urls(appid, kind)
                .into_iter()
                .find(|u| self.exists(u))
        {
            return Some(ArtOption {
                thumb: url.clone(),
                url,
                source: "Steam".into(),
            });
        }
        if let Some(o) = self
            .sgdb_options(game, kind)
            .ok()
            .and_then(|v| v.into_iter().next())
        {
            return Some(o);
        }
        self.igdb_art(game, kind)
            .ok()
            .and_then(|v| v.into_iter().next())
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
        if let Some(appid) = Self::steam_id(game) {
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

    /// Downloads an image to `dest`.
    pub fn download(&self, url: &str, dest: &Path) -> Result<()> {
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
        std::fs::write(dest, &bytes).ctx(dest)?;
        Ok(())
    }
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
