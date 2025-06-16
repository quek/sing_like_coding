use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct Point {
    pub automation_params_index: usize,
    pub value: u8,
    pub delay: u8,
}
