use common::process_data::MAX_CHANNELS;

pub const MAX_PATH_LEN: usize = 1024;
pub const MAX_TRACKS: usize = 0xff;

#[repr(C)]
pub struct SongState {
    song_file: [u8; MAX_PATH_LEN],
    pub play_p: bool,
    pub line_play: usize,
    pub loop_p: bool,
    pub loop_start: usize,
    pub loop_end: usize,
    pub process_elasped_avg: f64,
    pub cpu_usage: f64,
    pub tracks: [TrackState; MAX_TRACKS],
}

impl SongState {
    pub fn get_song_file(&self) -> Option<String> {
        let nul_pos = self
            .song_file
            .iter()
            .position(|&b| b == 0)
            .unwrap_or(self.song_file.len());
        std::str::from_utf8(&self.song_file[..nul_pos])
            .ok()
            .map(|s| s.to_string())
    }

    pub fn set_song_file(&mut self, name: &str) {
        let bytes = name.as_bytes();
        let len = bytes.len().min(self.song_file.len() - 1);
        self.song_file[..len].copy_from_slice(&bytes[..len]);
        self.song_file[len] = 0; // null 終端
    }
}

#[repr(C)]
pub struct TrackState {
    pub peaks: [f32; MAX_CHANNELS],
}
