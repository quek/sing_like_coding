use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Note {
    time: f64,
    duration: f64,
    channel: i16,
    key: i16,
    velocity: f64,
}
