use common::process_data::MAX_CHANNELS;

use crate::singer::Singer;
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
    pub process_elasped_avg: f64,
    pub cpu_usage: f64,
    pub tracks: [TrackState; MAX_TRACKS],
}

impl SongState {
    pub fn init(&mut self, singer: &Singer) {
        self.song_file_set(&singer.song_file.clone().unwrap_or_default());
        self.play_p = singer.play_p;
        self.line_play = singer.line_play;
        self.loop_p = singer.loop_p;
        self.loop_start = singer.loop_range.start;
        self.loop_end = singer.loop_range.end;
        self.process_elasped_avg = singer.process_elasped_avg;
        self.cpu_usage = singer.cpu_usage;
        for track in self.tracks.iter_mut() {
            for peak in track.peaks.iter_mut() {
                *peak = DB_MIN;
            }
        }
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
}
