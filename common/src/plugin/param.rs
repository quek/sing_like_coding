use bincode::{Decode, Encode};
use clap_sys::{ext::params::clap_param_info_flags, id::clap_id};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Encode, Decode, Serialize, Deserialize, PartialEq)]
pub struct Param {
    pub id: clap_id,
    pub flags: clap_param_info_flags,
    pub name: String,
    pub module: String,
    pub min_value: f64,
    pub max_value: f64,
    pub default_value: f64,
    pub value: f64,
}
