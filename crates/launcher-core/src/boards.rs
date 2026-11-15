//! Boards: hand-arranged views. A board holds titled fields that sit on a
//! grid; each field is a small grid of cover slots, and every slot can hold
//! a game or stay empty (a gap). The same game may sit in several fields and
//! boards. A board is stored as one JSON document and saved as a whole.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

/// Largest field, in cover slots.
/// Wider fields than the window scroll sideways.
pub const MAX_FIELD_COLS: u32 = 60;
pub const MAX_FIELD_ROWS: u32 = 8;
/// How far a field may sit from the board origin, in grid steps.
pub const MAX_BOARD_CELL: u32 = 400;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Board {
    pub id: i64,
    pub name: String,
    pub fields: Vec<BoardField>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct BoardField {
    /// Stable within its board; chosen by the frontend.
    pub id: String,
    pub title: String,
    /// Position on the board, in grid steps (the UI snaps to 24 px).
    pub x: u32,
    pub y: u32,
    /// Size in cover slots.
    pub cols: u32,
    pub rows: u32,
    pub slots: Vec<BoardSlot>,
    /// Set: the field shows this collection's games by itself, in place of
    /// hand-placed slots.
    pub collection_id: Option<i64>,
}

impl Default for BoardField {
    fn default() -> Self {
        Self {
            id: String::new(),
            title: String::new(),
            x: 0,
            y: 0,
            cols: 4,
            rows: 1,
            slots: Vec::new(),
            collection_id: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BoardSlot {
    pub col: u32,
    pub row: u32,
    pub game_id: i64,
}

/// What is stored in the database (everything but id and name).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct BoardLayout {
    pub fields: Vec<BoardField>,
}

/// Cleans up a board coming from the UI or an older save: sizes within
/// bounds, unique field ids, slots inside their field, one game per slot,
/// and only games that still exist.
pub fn normalize_fields(
    fields: Vec<BoardField>,
    game_exists: impl Fn(i64) -> bool,
) -> Vec<BoardField> {
    let mut ids = HashSet::new();
    fields
        .into_iter()
        .enumerate()
        .map(|(i, mut f)| {
            f.title = f.title.trim().to_string();
            f.cols = f.cols.clamp(1, MAX_FIELD_COLS);
            f.rows = f.rows.clamp(1, MAX_FIELD_ROWS);
            f.x = f.x.min(MAX_BOARD_CELL);
            f.y = f.y.min(MAX_BOARD_CELL);
            if f.id.trim().is_empty() || !ids.insert(f.id.clone()) {
                let mut n = i;
                while !ids.insert(format!("f{n}")) {
                    n += 1;
                }
                f.id = format!("f{n}");
            }
            if f.collection_id.is_some() {
                f.slots.clear();
            }
            let mut taken = HashSet::new();
            f.slots.retain(|s| {
                s.col < f.cols
                    && s.row < f.rows
                    && game_exists(s.game_id)
                    && taken.insert((s.col, s.row))
            });
            f.slots.sort_by_key(|s| (s.row, s.col));
            f
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn slot(col: u32, row: u32, game_id: i64) -> BoardSlot {
        BoardSlot { col, row, game_id }
    }

    #[test]
    fn normalizes_fields() {
        let fields = vec![
            BoardField {
                id: "a".into(),
                title: "  Mit Freunden ".into(),
                cols: 3,
                rows: 1,
                slots: vec![
                    slot(0, 0, 1),
                    slot(2, 0, 2),
                    slot(3, 0, 1),
                    slot(0, 1, 1),
                    slot(0, 0, 3),
                    slot(1, 0, 99),
                ],
                ..Default::default()
            },
            BoardField {
                id: "a".into(),
                cols: 0,
                rows: 50,
                x: 9999,
                slots: vec![slot(0, 0, 1)],
                ..Default::default()
            },
        ];
        let out = normalize_fields(fields, |id| id != 99);
        assert_eq!(out[0].title, "Mit Freunden");
        // Outside the field, a second game in the same slot and a deleted game are gone.
        assert_eq!(out[0].slots, vec![slot(0, 0, 1), slot(2, 0, 2)]);
        assert_ne!(out[1].id, "a");
        assert_eq!(
            (out[1].cols, out[1].rows, out[1].x),
            (1, MAX_FIELD_ROWS, MAX_BOARD_CELL)
        );
        // The same game may sit in several fields.
        assert_eq!(out[1].slots, vec![slot(0, 0, 1)]);
    }

    #[test]
    fn collection_fields_have_no_slots() {
        let out = normalize_fields(
            vec![BoardField {
                collection_id: Some(3),
                slots: vec![slot(0, 0, 1)],
                ..Default::default()
            }],
            |_| true,
        );
        assert_eq!(out[0].collection_id, Some(3));
        assert!(out[0].slots.is_empty());
    }

    #[test]
    fn layout_tolerates_missing_fields() {
        let l: BoardLayout =
            serde_json::from_str(r#"{"fields":[{"id":"x","title":"T"}]}"#).unwrap();
        assert_eq!((l.fields[0].cols, l.fields[0].rows), (4, 1));
        assert!(l.fields[0].slots.is_empty());
        let l: BoardLayout = serde_json::from_str("{}").unwrap();
        assert!(l.fields.is_empty());
    }
}
