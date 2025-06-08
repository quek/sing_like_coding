use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Module {
    pub id: String,
    pub name: String,
    pub state: Option<Vec<u8>>,
}

impl Module {
    pub fn new(id: String, name: String) -> Self {
        Self {
            id,
            name,
            state: None,
        }
    }
}
