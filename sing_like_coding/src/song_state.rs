use clap_sys::id::clap_id;
use common::process_data::MAX_CHANNELS;

use crate::view::stereo_peak_meter::DB_MIN;

pub const MAX_PATH_LEN: usize = 1024;
pub const MAX_TRACKS: usize = 0xff;

#[repr(C)]
#[derive(Debug)]
pub struct SongState {
    song_file: [u8; MAX_PATH_LEN],
    pub play_p: bool,
    pub line_play: usize,
    pub loop_p: bool,
    pub loop_start: usize,
    pub loop_end: usize,
    pub ms_play: usize,
    pub process_elasped_avg: f64,
    pub cpu_usage: f64,
    pub tracks: [TrackState; MAX_TRACKS],
    pub param_track_index: usize,
    pub param_module_index: usize,
    pub param_id: clap_id,
    pub rec_p: bool,
    pub song_dirty_p: bool,
}

impl SongState {
    pub fn init(&mut self) {
        self.song_file[0] = 0;
        self.play_p = false;
        self.line_play = 0;
        self.loop_p = false;
        self.loop_start = 0;
        self.loop_end = 0x100 * 0x20;
        self.ms_play = 0;
        self.process_elasped_avg = 0.0;
        self.cpu_usage = 0.0;
        for track in self.tracks.iter_mut() {
            for peak in track.peaks.iter_mut() {
                *peak = DB_MIN;
            }
            track.rec_p = false;
        }
        self.param_track_index = usize::MAX;
        self.rec_p = false;
        self.song_dirty_p = false;
    }

    pub fn song_file_get(&self) -> Option<String> {
        let null_pos = self
            .song_file
            .iter()
            .position(|&b| b == 0)
            .unwrap_or(self.song_file.len());
        std::str::from_utf8(&self.song_file[..null_pos])
            .ok()
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
    }

    pub fn song_file_set(&mut self, name: &str) {
        let bytes = name.as_bytes();
        let len = bytes.len().min(self.song_file.len() - 1);
        self.song_file[..len].copy_from_slice(&bytes[..len]);
        self.song_file[len] = 0; // null 終端
    }
}

#[repr(C)]
#[derive(Debug)]
pub struct TrackState {
    pub peaks: [f32; MAX_CHANNELS],
    pub rec_p: bool,
}
