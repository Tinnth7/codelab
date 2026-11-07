use egui::Key;
use std::collections::HashSet;

#[derive(Default)]
pub struct CaptureState {
    pub held: HashSet<Key>,
    pub mods: HashSet<&'static str>,
    pub main: Option<String>,
    pub live: String,
    pub result: Option<Result<String, ()>>, // Some(Ok(combo)) = accepted, Some(Err) = cancelled
}

const MOD_ORDER: &[&str] = &["Mod", "Ctrl", "Alt", "Shift"];

impl CaptureState {
    pub fn new() -> Self { Self::default() }

    pub fn on_key(&mut self, key: Key, pressed: bool, modifiers: egui::Modifiers) {
        if self.result.is_some() { return; }

        // Sync current modifier state from egui
        self.mods.clear();
        if modifiers.mac_cmd || modifiers.command { self.mods.insert("Mod"); }
        if modifiers.ctrl && !modifiers.command { self.mods.insert("Ctrl"); }
        if modifiers.alt { self.mods.insert("Alt"); }
        if modifiers.shift { self.mods.insert("Shift"); }

        if pressed {
            if key == Key::Escape && !modifiers.any() {
                self.result = Some(Err(()));
                return;
            }
            if let Some(name) = key_name(key) {
                self.main = Some(name);
            }
            self.held.insert(key);
        } else {
            self.held.remove(&key);
            if self.held.is_empty() {
                if let Some(main) = &self.main {
                    let mut parts: Vec<&str> = MOD_ORDER.iter()
                    .filter(|m| self.mods.contains(*m))
                    .copied().collect();
                    parts.push(main);
                    self.result = Some(Ok(parts.join("+")));
                } else {
                    self.update_live();
                }
            }
        }
        self.update_live();
    }

    fn update_live(&mut self) {
        let mut parts: Vec<&str> = MOD_ORDER.iter()
        .filter(|m| self.mods.contains(*m))
        .copied().collect();
        if let Some(m) = &self.main { parts.push(m); }
        self.live = if parts.is_empty() { "…".to_string() } else { parts.join("+") };
    }
}

fn key_name(k: Key) -> Option<String> {
    Some(match k {
        Key::A => "A".into(), Key::B => "B".into(), Key::C => "C".into(), Key::D => "D".into(),
         Key::E => "E".into(), Key::F => "F".into(), Key::G => "G".into(), Key::H => "H".into(),
         Key::I => "I".into(), Key::J => "J".into(), Key::K => "K".into(), Key::L => "L".into(),
         Key::M => "M".into(), Key::N => "N".into(), Key::O => "O".into(), Key::P => "P".into(),
         Key::Q => "Q".into(), Key::R => "R".into(), Key::S => "S".into(), Key::T => "T".into(),
         Key::U => "U".into(), Key::V => "V".into(), Key::W => "W".into(), Key::X => "X".into(),
         Key::Y => "Y".into(), Key::Z => "Z".into(),
         Key::Num0 => "0".into(), Key::Num1 => "1".into(), Key::Num2 => "2".into(),
         Key::Num3 => "3".into(), Key::Num4 => "4".into(), Key::Num5 => "5".into(),
         Key::Num6 => "6".into(), Key::Num7 => "7".into(), Key::Num8 => "8".into(),
         Key::Num9 => "9".into(),
         Key::F1 => "F1".into(), Key::F2 => "F2".into(), Key::F3 => "F3".into(), Key::F4 => "F4".into(),
         Key::F5 => "F5".into(), Key::F6 => "F6".into(), Key::F7 => "F7".into(), Key::F8 => "F8".into(),
         Key::F9 => "F9".into(), Key::F10 => "F10".into(), Key::F11 => "F11".into(), Key::F12 => "F12".into(),
         Key::Enter => "Return".into(),
         Key::Escape => "Escape".into(),
         Key::Tab => "Tab".into(),
         Key::Space => "Space".into(),
         Key::Backspace => "BackSpace".into(),
         Key::Delete => "Delete".into(),
         Key::Insert => "Insert".into(),
         Key::Home => "Home".into(),
         Key::End => "End".into(),
         Key::PageUp => "Page_Up".into(),
         Key::PageDown => "Page_Down".into(),
         Key::ArrowLeft => "Left".into(),
         Key::ArrowRight => "Right".into(),
         Key::ArrowUp => "Up".into(),
         Key::ArrowDown => "Down".into(),
         Key::Minus => "Minus".into(),
         Key::Equals => "Equal".into(),
         Key::OpenBracket => "BracketLeft".into(),
         Key::CloseBracket => "BracketRight".into(),
         Key::Backslash => "Backslash".into(),
         Key::Semicolon => "Semicolon".into(),
         Key::Quote => "Apostrophe".into(),
         Key::Comma => "Comma".into(),
         Key::Period => "Period".into(),
         Key::Slash => "Slash".into(),
         Key::Backtick => "Grave".into(),
         _ => return None,
    })
}
