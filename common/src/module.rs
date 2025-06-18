use serde::{Deserialize, Serialize};

pub type ModuleIndex = (usize, usize); // (track_index, module_index)

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
    pub src_module_index: ModuleIndex,
    pub src_port_index: usize,
    pub dst_port_index: usize,
}
