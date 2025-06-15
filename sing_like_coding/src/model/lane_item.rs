use serde::{Deserialize, Serialize};

use super::{note::Note, point::Point};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum LaneItem {
    Note(Note),
    Point(Point),
}

impl Default for LaneItem {
    fn default() -> Self {
        Self::Note(Note::default())
    }
}
