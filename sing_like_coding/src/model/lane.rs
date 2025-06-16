use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use super::lane_item::LaneItem;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Lane {
    pub items: HashMap<usize, LaneItem>,
}

impl Lane {
    pub fn new() -> Self {
        Self {
            items: Default::default(),
        }
    }

    pub fn item(&self, line: usize) -> Option<&LaneItem> {
        self.items.get(&line)
    }

    pub fn item_mut(&mut self, line: usize) -> Option<&mut LaneItem> {
        self.items.get_mut(&line)
    }
}
