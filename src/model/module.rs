use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Module {
    pub id: String,
    pub state: Option<String>,
}

impl Module {
    pub fn new(id: String) -> Self {
        Self { id, state: None }
    }
}
