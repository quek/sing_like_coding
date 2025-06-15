use clap_sys::id::clap_id;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct Point {
    pub module_index: usize,
    pub param_id: clap_id,
    pub value: u8,
    pub delay: u8,
}
