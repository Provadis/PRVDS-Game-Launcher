//! Filtering, sorting and saved views. Libraries are small (hundreds of
//! games), so this runs in memory on the list returned by the database.

use serde::{Deserialize, Serialize};

use crate::model::{Game, Platform};
use crate::util::normalize_name;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SortField {
    #[default]
    Name,
    Platform,
    AddedAt,
    LastPlayed,
    Playtime,
    PlayCount,
    ReleaseYear,
    Genre,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PlayedFilter {
    #[default]
    Any,
    Played,
    NeverPlayed,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct GameQuery {
    pub search: String,
    /// Empty = all platforms.
    pub platforms: Vec<Platform>,
    /// Game must have all of these tags.
    pub tags: Vec<String>,
    pub genre: Option<String>,
    pub collection: Option<i64>,
    pub favorites_only: bool,
    pub include_hidden: bool,
    pub played: PlayedFilter,
    /// Only games not played for at least this many days (never played counts too).
    pub not_played_for_days: Option<u32>,
    /// Only games played within the last N days.
    pub played_within_days: Option<u32>,
    pub sort: SortField,
    pub descending: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum LayoutMode {
    #[default]
    Grid,
    List,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Layout {
    pub mode: LayoutMode,
    /// Cover width in px for the grid.
    pub cover_size: u32,
}

impl Default for Layout {
    fn default() -> Self {
        Self {
            mode: LayoutMode::Grid,
            cover_size: 200,
        }
    }
}

/// What a saved view stores: the filters, the sort order and the layout.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ViewDefinition {
    pub query: GameQuery,
    pub layout: Layout,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SavedView {
    pub id: i64,
    pub name: String,
    pub definition: ViewDefinition,
}

const DAY: i64 = 86_400;

impl GameQuery {
    pub fn matches(&self, g: &Game, now: i64) -> bool {
        if g.hidden && !self.include_hidden {
            return false;
        }
        if self.favorites_only && !g.favorite {
            return false;
        }
        if !self.platforms.is_empty() && !self.platforms.contains(&g.platform) {
            return false;
        }
        if let Some(c) = self.collection
            && !g.collections.contains(&c)
        {
            return false;
        }
        if !self
            .tags
            .iter()
            .all(|t| g.tags.iter().any(|gt| gt.eq_ignore_ascii_case(t)))
        {
            return false;
        }
        if let Some(genre) = &self.genre {
            let has = g.genre.as_deref().is_some_and(|gg| {
                gg.split(',')
                    .any(|part| part.trim().eq_ignore_ascii_case(genre.trim()))
            });
            if !has {
                return false;
            }
        }
        let played = g.last_played.is_some() || g.playtime_secs > 0;
        match self.played {
            PlayedFilter::Any => {}
            PlayedFilter::Played if !played => return false,
            PlayedFilter::NeverPlayed if played => return false,
            _ => {}
        }
        if let Some(days) = self.not_played_for_days
            && let Some(lp) = g.last_played
            && now - lp < days as i64 * DAY
        {
            return false;
        }
        if let Some(days) = self.played_within_days {
            match g.last_played {
                Some(lp) if now - lp <= days as i64 * DAY => {}
                _ => return false,
            }
        }
        let search = self.search.trim();
        if !search.is_empty() {
            let needle = normalize_name(search);
            let hay = normalize_name(&g.name);
            let in_meta = |s: &Option<String>| {
                s.as_deref()
                    .is_some_and(|v| normalize_name(v).contains(&needle))
            };
            let in_tags = g.tags.iter().any(|t| normalize_name(t).contains(&needle));
            if !(hay.contains(&needle) || in_meta(&g.genre) || in_meta(&g.developer) || in_tags) {
                return false;
            }
        }
        true
    }

    /// Filters and sorts. Missing values (never played, no year, ...) always
    /// go to the end, whatever the direction.
    pub fn apply(&self, games: &[Game], now: i64) -> Vec<Game> {
        let mut out: Vec<Game> = games
            .iter()
            .filter(|g| self.matches(g, now))
            .cloned()
            .collect();
        let name_key = |g: &Game| g.name.to_lowercase();
        out.sort_by(|a, b| {
            use std::cmp::Ordering;
            fn opt<T: Ord>(a: Option<T>, b: Option<T>, desc: bool) -> Ordering {
                match (a, b) {
                    (Some(x), Some(y)) => {
                        if desc {
                            y.cmp(&x)
                        } else {
                            x.cmp(&y)
                        }
                    }
                    (Some(_), None) => Ordering::Less,
                    (None, Some(_)) => Ordering::Greater,
                    (None, None) => Ordering::Equal,
                }
            }
            let d = self.descending;
            let primary = match self.sort {
                SortField::Name => opt(Some(name_key(a)), Some(name_key(b)), d),
                SortField::Platform => opt(Some(a.platform), Some(b.platform), d),
                SortField::AddedAt => opt(Some(a.added_at), Some(b.added_at), d),
                SortField::LastPlayed => opt(a.last_played, b.last_played, d),
                SortField::Playtime => opt(Some(a.playtime_secs), Some(b.playtime_secs), d),
                SortField::PlayCount => opt(Some(a.play_count), Some(b.play_count), d),
                SortField::ReleaseYear => opt(a.release_year, b.release_year, d),
                SortField::Genre => opt(
                    a.genre.as_ref().map(|s| s.to_lowercase()),
                    b.genre.as_ref().map(|s| s.to_lowercase()),
                    d,
                ),
            };
            primary.then_with(|| name_key(a).cmp(&name_key(b)))
        });
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn game(id: i64, name: &str, platform: Platform) -> Game {
        Game {
            id,
            name: name.into(),
            platform,
            platform_id: None,
            shortcut_path: String::new(),
            original_path: None,
            journal_id: None,
            launch_target: None,
            launch_args: None,
            source: crate::model::GameSource::Desktop,
            installed: true,
            install_dir: None,
            process_exe: None,
            cover_path: None,
            hero_path: None,
            added_at: id,
            last_played: None,
            play_count: 0,
            playtime_secs: 0,
            favorite: false,
            hidden: false,
            genre: None,
            release_year: None,
            developer: None,
            summary: None,
            tags: vec![],
            collections: vec![],
        }
    }

    fn sample() -> Vec<Game> {
        let now = 100 * DAY;
        let mut a = game(1, "Elden Ring", Platform::Steam);
        a.last_played = Some(now - DAY);
        a.playtime_secs = 5000;
        a.genre = Some("Rollenspiel, Action".into());
        a.tags = vec!["Souls".into()];
        a.release_year = Some(2022);
        let mut b = game(2, "Diablo IV", Platform::Battlenet);
        b.last_played = Some(now - 40 * DAY);
        b.playtime_secs = 9000;
        b.favorite = true;
        b.collections = vec![7];
        let mut c = game(3, "Alan Wake 2", Platform::Epic);
        c.release_year = Some(2023);
        let mut d = game(4, "Hidden Game", Platform::Steam);
        d.hidden = true;
        vec![a, b, c, d]
    }

    fn names(v: &[Game]) -> Vec<&str> {
        v.iter().map(|g| g.name.as_str()).collect()
    }

    const NOW: i64 = 100 * DAY;

    #[test]
    fn default_sorts_by_name_and_hides_hidden() {
        let r = GameQuery::default().apply(&sample(), NOW);
        assert_eq!(names(&r), ["Alan Wake 2", "Diablo IV", "Elden Ring"]);
        let q = GameQuery {
            include_hidden: true,
            ..Default::default()
        };
        assert_eq!(q.apply(&sample(), NOW).len(), 4);
    }

    #[test]
    fn sorts_with_missing_values_last() {
        let q = GameQuery {
            sort: SortField::LastPlayed,
            descending: true,
            ..Default::default()
        };
        assert_eq!(
            names(&q.apply(&sample(), NOW)),
            ["Elden Ring", "Diablo IV", "Alan Wake 2"]
        );
        let q = GameQuery {
            sort: SortField::ReleaseYear,
            ..Default::default()
        };
        assert_eq!(
            names(&q.apply(&sample(), NOW)),
            ["Elden Ring", "Alan Wake 2", "Diablo IV"]
        );
        let q = GameQuery {
            sort: SortField::Playtime,
            descending: true,
            ..Default::default()
        };
        assert_eq!(names(&q.apply(&sample(), NOW))[0], "Diablo IV");
    }

    #[test]
    fn filters() {
        let s = sample();
        let q = GameQuery {
            platforms: vec![Platform::Steam, Platform::Epic],
            ..Default::default()
        };
        assert_eq!(names(&q.apply(&s, NOW)), ["Alan Wake 2", "Elden Ring"]);
        let q = GameQuery {
            favorites_only: true,
            ..Default::default()
        };
        assert_eq!(names(&q.apply(&s, NOW)), ["Diablo IV"]);
        let q = GameQuery {
            collection: Some(7),
            ..Default::default()
        };
        assert_eq!(names(&q.apply(&s, NOW)), ["Diablo IV"]);
        let q = GameQuery {
            tags: vec!["souls".into()],
            ..Default::default()
        };
        assert_eq!(names(&q.apply(&s, NOW)), ["Elden Ring"]);
        let q = GameQuery {
            genre: Some("Action".into()),
            ..Default::default()
        };
        assert_eq!(names(&q.apply(&s, NOW)), ["Elden Ring"]);
        let q = GameQuery {
            played: PlayedFilter::NeverPlayed,
            ..Default::default()
        };
        assert_eq!(names(&q.apply(&s, NOW)), ["Alan Wake 2"]);
        let q = GameQuery {
            not_played_for_days: Some(30),
            ..Default::default()
        };
        assert_eq!(names(&q.apply(&s, NOW)), ["Alan Wake 2", "Diablo IV"]);
        let q = GameQuery {
            played_within_days: Some(7),
            ..Default::default()
        };
        assert_eq!(names(&q.apply(&s, NOW)), ["Elden Ring"]);
        let q = GameQuery {
            search: "elden".into(),
            ..Default::default()
        };
        assert_eq!(names(&q.apply(&s, NOW)), ["Elden Ring"]);
        let q = GameQuery {
            search: "rollen".into(),
            ..Default::default()
        };
        assert_eq!(names(&q.apply(&s, NOW)), ["Elden Ring"]);
    }

    #[test]
    fn view_definition_tolerates_missing_fields() {
        let def: ViewDefinition = serde_json::from_str(r#"{"query":{"sort":"playtime"}}"#).unwrap();
        assert_eq!(def.query.sort, SortField::Playtime);
        assert_eq!(def.layout.cover_size, 200);
    }
}
