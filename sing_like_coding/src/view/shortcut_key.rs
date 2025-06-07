use eframe::egui::{InputState, Key};

#[allow(dead_code)]
pub enum Modifier {
    None,
    C,
    A,
    S,
    CA,
    CS,
    AS,
    CAS,
}

pub trait ShortcutKey {
    fn is(&self, modifiers: Modifier, key: Key) -> bool;
}

impl ShortcutKey for InputState {
    fn is(&self, modifiers: Modifier, key: Key) -> bool {
        self.key_pressed(key)
            && match modifiers {
                Modifier::None => {
                    !self.modifiers.ctrl && !self.modifiers.alt && !self.modifiers.shift
                }
                Modifier::C => self.modifiers.ctrl && !self.modifiers.alt && !self.modifiers.shift,

                Modifier::A => !self.modifiers.ctrl && self.modifiers.alt && !self.modifiers.shift,
                Modifier::S => !self.modifiers.ctrl && !self.modifiers.alt && self.modifiers.shift,
                Modifier::CA => self.modifiers.ctrl && self.modifiers.alt && !self.modifiers.shift,
                Modifier::CS => self.modifiers.ctrl && !self.modifiers.alt && self.modifiers.shift,
                Modifier::AS => !self.modifiers.ctrl && self.modifiers.alt && self.modifiers.shift,
                Modifier::CAS => self.modifiers.ctrl && self.modifiers.alt && self.modifiers.shift,
            }
    }
}
