use std::collections::{HashMap, HashSet, VecDeque};

use chrono::Local;
use common::module::{Module, ModuleId, ModuleIndex};
use serde::{Deserialize, Serialize};

use crate::app_state::CursorTrack;

use super::{lane_item::LaneItem, track::Track};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Song {
    pub name: String,
    pub bpm: f64,
    pub sample_rate: f64,
    pub lpb: u16,
    pub tracks: Vec<Track>,
}

impl Song {
    pub fn new() -> Self {
        Self {
            name: Local::now().format("%Y%m%d.json").to_string(),
            bpm: 128.0,
            sample_rate: 48000.0,
            lpb: 4,
            tracks: vec![],
        }
    }

    pub fn module_by_id_mut(&mut self, id: ModuleId) -> Option<&mut Module> {
        self.tracks
            .iter_mut()
            .find_map(|track| track.modules.iter_mut().find(|module| module.id == id))
    }

    pub fn track_add(&mut self) {
        let mut track = Track::new();
        track.name = if self.tracks.is_empty() {
            "Main".to_string()
        } else {
            format!("T{:02X}", self.tracks.len())
        };
        self.tracks.push(track);
    }

    #[allow(dead_code)]
    pub fn track_at(&self, track_index: usize) -> Option<&Track> {
        self.tracks.get(track_index)
    }

    pub fn track_at_mut(&mut self, track_index: usize) -> Option<&mut Track> {
        self.tracks.get_mut(track_index)
    }

    pub fn track_delete(&mut self, track_index: usize) {
        self.tracks.remove(track_index);

        for track in &mut self.tracks {
            for module in &mut track.modules {
                module
                    .audio_inputs
                    .retain(|input| input.src_module_index.0 != track_index);
                for audio_input in &mut module.audio_inputs {
                    let src_index = &mut audio_input.src_module_index.0;
                    if *src_index > track_index {
                        *src_index -= 1;
                    }
                }
            }
        }
    }

    pub fn track_insert(&mut self, track_index: usize, track: Track) {
        self.tracks.insert(track_index, track);

        for track in &mut self.tracks {
            for module in &mut track.modules {
                for audio_input in &mut module.audio_inputs {
                    let src_index = &mut audio_input.src_module_index.0;
                    if *src_index >= track_index {
                        *src_index += 1;
                    }
                }
            }
        }
    }

    pub fn track_move(&mut self, track_index: usize, delta: isize) {
        let track_index_new = track_index.saturating_add_signed(delta);
        let track = self.tracks.remove(track_index);
        self.tracks.insert(track_index_new, track);

        let direction = delta.signum();
        let range = track_index.min(track_index_new)..(track_index.max(track_index_new) + 1);

        for track in self.tracks.iter_mut() {
            for module in &mut track.modules {
                for audio_input in &mut module.audio_inputs {
                    let src_index = &mut audio_input.src_module_index.0;
                    if *src_index == track_index {
                        *src_index = track_index_new;
                    } else if range.contains(src_index) {
                        *src_index = src_index.saturating_add_signed(direction);
                    }
                }
            }
        }
    }

    pub fn lane_item(&self, cursor: &CursorTrack) -> Option<&LaneItem> {
        self.tracks
            .get(cursor.track)
            .and_then(|x| x.lanes.get(cursor.lane))
            .and_then(|x| x.item(cursor.line))
    }

    // pub fn lane_item_mut(&mut self, cursor: &CursorTrack) -> Option<&mut LaneItem> {
    //     self.tracks
    //         .get_mut(cursor.track)
    //         .and_then(|x| x.lanes.get_mut(cursor.lane))
    //         .and_then(|x| x.item_mut(cursor.line))
    // }

    pub fn module_at(&self, module_index: ModuleIndex) -> Option<&Module> {
        self.track_at(module_index.0)
            .and_then(|track| track.modules.get(module_index.1))
    }

    pub fn module_at_mut(&mut self, module_index: ModuleIndex) -> Option<&mut Module> {
        self.track_at_mut(module_index.0)
            .and_then(|track| track.modules.get_mut(module_index.1))
    }
}

/// トポロジカル順にモジュールを依存レベルごとに分けて返す。
/// Track 0:
///     Module 0
///     Module 1 ← depends on Track 1, Module 0
///
/// Track 1:
///     Module 0 ← depends on Track 0, Module 0
///     Module 1
/// こういう依存関係でも処理できるように作ってもらった

pub fn topological_levels(song: &Song) -> anyhow::Result<Vec<Vec<ModuleIndex>>> {
    let mut graph: HashMap<ModuleIndex, HashSet<ModuleIndex>> = HashMap::new(); // node -> deps
    let mut reverse_graph: HashMap<ModuleIndex, HashSet<ModuleIndex>> = HashMap::new(); // dep -> users
    let mut in_degree: HashMap<ModuleIndex, usize> = HashMap::new();

    // グラフ構築
    for (track_index, track) in song.tracks.iter().enumerate() {
        if track_index == 0 {
            continue;
        }
        for (module_index, module) in track.modules.iter().enumerate() {
            let id = (track_index, module_index);
            let deps = module
                .audio_inputs
                .iter()
                .map(|input| input.src_module_index);

            for dep in deps {
                graph.entry(id).or_default().insert(dep);
                reverse_graph.entry(dep).or_default().insert(id);
            }

            in_degree.insert(id, graph.get(&id).map_or(0, |s| s.len()));
        }
    }

    // レベルごとに分割
    let mut levels: Vec<Vec<ModuleIndex>> = Vec::new();
    let mut queue: VecDeque<ModuleIndex> = in_degree
        .iter()
        .filter_map(|(id, &deg)| if deg == 0 { Some(*id) } else { None })
        .collect();

    while !queue.is_empty() {
        let mut current_level = Vec::new();
        let mut next_queue = VecDeque::new();

        for id in queue {
            current_level.push(id);
            if let Some(children) = reverse_graph.get(&id) {
                for &child in children {
                    if let Some(deg) = in_degree.get_mut(&child) {
                        *deg -= 1;
                        if *deg == 0 {
                            next_queue.push_back(child);
                        }
                    }
                }
            }
        }

        levels.push(current_level);
        queue = next_queue;
    }

    // サイクル検出
    let total_processed: usize = levels.iter().map(|level| level.len()).sum();
    if total_processed != in_degree.len() {
        anyhow::bail!("循環依存があります（例：サイドチェインの相互依存）");
    }

    Ok(levels)
}
