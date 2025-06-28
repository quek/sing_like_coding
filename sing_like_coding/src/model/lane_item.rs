use serde::{Deserialize, Serialize};

use super::{note::Note, point::Point};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum LaneItem {
    Note(Note),
    Point(Point),
    Call(String),
    Label(String),
    Ret,
}

impl LaneItem {
    pub fn delay(&self) -> u8 {
        match self {
            LaneItem::Note(Note { delay, .. }) => *delay,
            LaneItem::Point(Point { delay, .. }) => *delay,
            LaneItem::Call(_) => 0,
            LaneItem::Label(_) => 0,
            LaneItem::Ret => 0,
        }
    }
}

impl Default for LaneItem {
    fn default() -> Self {
        Self::Note(Note::default())
    }
}
