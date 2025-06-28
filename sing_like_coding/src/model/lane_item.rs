use serde::{Deserialize, Serialize};

use super::{note::Note, point::Point};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum LaneItem {
    Note(Note),
    Point(Point),
    Label(String),
}

impl LaneItem {
    pub fn delay(&self) -> u8 {
        match self {
            LaneItem::Note(Note { delay, .. }) => *delay,
            LaneItem::Point(Point { delay, .. }) => *delay,
            LaneItem::Label(_) => 0,
        }
    }
}

impl Default for LaneItem {
    fn default() -> Self {
        Self::Note(Note::default())
    }
}
