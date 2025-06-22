use crate::singer::MainToAudio;

#[derive(Clone)]
struct UndoHistoryItem {
    pub undo: MainToAudio,
    pub redo: MainToAudio,
}

pub struct UndoHistory {
    undos: Vec<UndoHistoryItem>,
    redos: Vec<UndoHistoryItem>,
    pub traveling_p: bool,
}

impl UndoHistory {
    pub fn new() -> Self {
        Self {
            undos: vec![],
            redos: vec![],
            traveling_p: false,
        }
    }

    pub fn add(&mut self, undo: MainToAudio, redo: MainToAudio) {
        if self.traveling_p {
            return;
        }
        self.undos.push(UndoHistoryItem { undo, redo });
        self.redos.clear();
    }

    pub fn undo(&mut self) -> Option<MainToAudio> {
        self.traveling_p = true;
        if let Some(item) = self.undos.pop() {
            self.redos.push(item.clone());
            Some(item.undo)
        } else {
            None
        }
    }

    pub fn redo(&mut self) -> Option<MainToAudio> {
        self.traveling_p = true;
        if let Some(item) = self.redos.pop() {
            self.undos.push(item.clone());
            Some(item.redo)
        } else {
            None
        }
    }
}
