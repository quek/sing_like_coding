use serde::{Deserialize, Serialize};

use super::{note::Note, point::Point};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum LaneItem {
    Note(Note),
    Point(Point),
}

impl LaneItem {
    pub fn delay(&self) -> u8 {
        match self {
            LaneItem::Note(Note { delay, .. }) => *delay,
            LaneItem::Point(Point { delay, .. }) => *delay,
        }
    }
}

impl Default for LaneItem {
    fn default() -> Self {
        Self::Note(Note::default())
    }
}
