use eframe::egui::{Event, Key};

#[allow(dead_code)]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord, Hash)]
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

pub fn shortcut_key(gui_context: &eframe::egui::Context) -> Option<(Modifier, Key)> {
    let input = gui_context.input(|i| i.clone());
    if let Some(key) = input
        .events
        .iter()
        .filter_map(|event| match event {
            Event::Key {
                key, pressed: true, ..
            } => Some(*key),
            // C-c, C-x, C-v は変換されている
            // https://github.com/emilk/egui/blob/main/crates/egui-winit/src/lib.rs#L768
            Event::Copy => Some(Key::C),
            Event::Cut => Some(Key::X),
            Event::Paste(_) => Some(Key::V),
            _ => None,
        })
        .next()
    {
        let modifiers = &input.modifiers;
        let modifier = if modifiers.ctrl && !modifiers.alt && !modifiers.shift {
            Modifier::C
        } else if !modifiers.ctrl && modifiers.alt && !modifiers.shift {
            Modifier::A
        } else if !modifiers.ctrl && !modifiers.alt && modifiers.shift {
            Modifier::S
        } else if modifiers.ctrl && modifiers.alt && !modifiers.shift {
            Modifier::CA
        } else if modifiers.ctrl && !modifiers.alt && modifiers.shift {
            Modifier::CS
        } else if !modifiers.ctrl && modifiers.alt && modifiers.shift {
            Modifier::AS
        } else if modifiers.ctrl && modifiers.alt && modifiers.shift {
            Modifier::CAS
        } else {
            Modifier::None
        };
        // dbg!(&modifier, &key,);
        Some((modifier, key))
    } else {
        None
    }
}
