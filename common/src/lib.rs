pub mod audio_buffer;
pub mod clap_manager;
pub mod dsp;
pub mod event;
pub mod module;
pub mod plugin;
pub mod plugin_ref;
pub mod process_data;
pub mod process_track_context;
pub mod protocol;
pub mod shmem;
pub mod str;
pub mod util;

pub const PIPE_CTRL_NAME: &'static str = r"\\.\pipe\sing_like_coding\ctrl";
pub const PIPE_BUFFER_SIZE: u32 = 8092;
