use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Module {
    pub id: String,
    pub name: String,
    pub audio_inputs: Vec<AudioInput>,
    pub state: Option<Vec<u8>>,
}

impl Module {
    pub fn new(id: String, name: String, audio_inputs: Vec<AudioInput>) -> Self {
        Self {
            id,
            name,
            audio_inputs,
            state: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioInput {
    pub track_index: usize,
    pub module_index: usize,
}
