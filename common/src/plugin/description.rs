use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Description {
    pub id: String,
    pub path: String,
    pub modified: u64,
    pub index: u32,
    pub name: String,
    pub vender: String,
    pub version: String,
    pub description: String,
    pub features: Vec<String>,
}
