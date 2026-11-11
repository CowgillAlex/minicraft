use crate::{
    hud::{Hud, HudVertex},
    overlay::{ACCENT, MUTED, TEXT},
    ui::{Layout, Rect},
};
use winit::keyboard::KeyCode;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Action {
    Seed,
    Create,
    Quit,
}

/// A seed entry screen shown before any terrain generation starts.
pub struct Title {
    pub seed: String,
    pub pointer: [f32; 2],
    pub focused: bool,
    pub selected: bool,
    pub error: Option<&'static str>,
}
impl Title {
    pub fn new(seed: Option<i64>) -> Self {
        Self {
            seed: seed.map(|v| v.to_string()).unwrap_or_default(),
            pointer: [0.0; 2],
            focused: true,
            selected: true,
            error: None,
        }
    }
    pub fn seed_value(&self) -> Result<i64, &'static str> {
        if self.seed.is_empty() {
            return Ok(std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos() as i64);
        }
        self.seed.parse().map_err(|_| {
            "Enter a whole number between -9223372036854775808 and 9223372036854775807."
        })
    }
    pub fn insert(&mut self, value: &str) {
        let value = value.trim();
        if value.is_empty() || !value.chars().all(|c| c.is_ascii_digit() || c == '-') {
            return;
        }
        let mut next = if self.selected {
            String::new()
        } else {
            self.seed.clone()
        };
        next.push_str(value);
        if next.len() <= 20 && !next[usize::from(next.starts_with('-'))..].contains('-') {
            self.seed = next;
            self.selected = false;
            self.error = None;
        }
    }
    pub fn key(&mut self, key: KeyCode, text: Option<&str>, control: bool) -> Option<Action> {
        match key {
            KeyCode::Enter | KeyCode::NumpadEnter => return Some(Action::Create),
            KeyCode::Tab => {
                self.focused = !self.focused;
                self.selected = self.focused;
            }
            KeyCode::KeyA if control && self.focused => self.selected = true,
            KeyCode::Backspace | KeyCode::Delete if self.focused => {
                if self.selected || key == KeyCode::Delete {
                    self.seed.clear();
                } else {
                    self.seed.pop();
                }
                self.selected = false;
                self.error = None;
            }
            _ if self.focused && !control => {
                if let Some(text) = text {
                    self.insert(text);
                }
            }
            _ => {}
        }
        None
    }
}

fn controls(layout: &Layout) -> [(Action, Rect); 3] {
    let x = (layout.width - 400.0) / 2.0;
    let y = layout.height / 2.0 - 24.0;
    [
        (
            Action::Seed,
            Rect {
                x,
                y,
                w: 400.0,
                h: 48.0,
            },
        ),
        (
            Action::Create,
            Rect {
                x,
                y: y + 132.0,
                w: 400.0,
                h: 48.0,
            },
        ),
        (
            Action::Quit,
            Rect {
                x,
                y: y + 192.0,
                w: 400.0,
                h: 42.0,
            },
        ),
    ]
}
pub fn action_at(size: (u32, u32), pointer: [f32; 2]) -> Option<Action> {
    let layout = Layout::new(size);
    controls(&layout)
        .into_iter()
        .find(|(_, r)| r.contains(pointer.map(|v| v / layout.scale)))
        .map(|(a, _)| a)
}
pub fn vertices(size: (u32, u32), state: &Title) -> Vec<HudVertex> {
    let layout = Layout::new(size);
    let mut hud = Hud::new(layout.width, layout.height);
    let [(.., field), (.., create), (.., quit)] = controls(&layout);
    let center = layout.width / 2.0;
    crate::ui_icons::block(
        &mut hud,
        center - 24.0,
        field.y - 160.0,
        48.0,
        crate::world::GRASS,
    );
    hud.centered("MiniCraft", center, field.y - 90.0, 40.0, TEXT, true);
    hud.centered("New world", center, field.y - 42.0, 14.0, MUTED, false);
    hud.text("World seed", field.x, field.y - 22.0, 13.0, TEXT);
    hud.rounded_rect(
        field,
        8.0,
        if state.focused {
            ACCENT
        } else {
            [0.25, 0.30, 0.32, 1.0]
        },
    );
    hud.rounded_rect(
        Rect {
            x: field.x + 1.0,
            y: field.y + 1.0,
            w: field.w - 2.0,
            h: field.h - 2.0,
        },
        7.0,
        [0.035, 0.046, 0.052, 1.0],
    );
    if state.selected && state.focused && !state.seed.is_empty() {
        hud.rect(
            field.x + 12.0,
            field.y + 10.0,
            Hud::text_width(&state.seed, 16.0, false) + 4.0,
            27.0,
            [0.21, 0.34, 0.28, 1.0],
        );
    }
    hud.text(
        if state.seed.is_empty() {
            "Random"
        } else {
            &state.seed
        },
        field.x + 14.0,
        field.y + 16.0,
        16.0,
        if state.seed.is_empty() { MUTED } else { TEXT },
    );
    if state.focused && !state.selected {
        hud.rect(
            field.x + 14.0 + Hud::text_width(&state.seed, 16.0, false),
            field.y + 13.0,
            1.0,
            22.0,
            ACCENT,
        );
    }
    hud.text(
        "Leave blank for a random seed.",
        field.x,
        field.y + 60.0,
        12.0,
        MUTED,
    );
    if state.error.is_some() {
        hud.text(
            "Seed must be a signed 64-bit whole number.",
            field.x,
            field.y + 83.0,
            12.0,
            [0.97, 0.58, 0.48, 1.0],
        );
    }
    let pointer = state.pointer.map(|v| v / layout.scale);
    hud.rounded_rect(
        create,
        8.0,
        if create.contains(pointer) {
            [0.75, 0.92, 0.73, 1.0]
        } else {
            ACCENT
        },
    );
    hud.centered(
        "Create world",
        center,
        create.y + 15.0,
        16.0,
        [0.035, 0.075, 0.045, 1.0],
        true,
    );
    if quit.contains(pointer) {
        hud.rounded_rect(quit, 8.0, [0.10, 0.13, 0.14, 1.0]);
    }
    hud.centered("Quit game", center, quit.y + 13.0, 14.0, MUTED, false);
    hud.text(
        &format!("v{}", env!("CARGO_PKG_VERSION")),
        24.0,
        layout.height - 35.0,
        12.0,
        MUTED,
    );
    hud.text(
        "6.25 cm voxels",
        layout.width - 115.0,
        layout.height - 35.0,
        12.0,
        MUTED,
    );
    hud.vertices
}

#[cfg(target_os = "windows")]
pub fn clipboard() -> Option<String> {
    #[link(name = "user32")]
    extern "system" {
        fn OpenClipboard(window: *mut std::ffi::c_void) -> i32;
        fn CloseClipboard() -> i32;
        fn GetClipboardData(format: u32) -> *mut std::ffi::c_void;
    }
    #[link(name = "kernel32")]
    extern "system" {
        fn GlobalLock(handle: *mut std::ffi::c_void) -> *const u16;
        fn GlobalUnlock(handle: *mut std::ffi::c_void) -> i32;
        fn GlobalSize(handle: *mut std::ffi::c_void) -> usize;
    }
    unsafe {
        if OpenClipboard(std::ptr::null_mut()) == 0 {
            return None;
        }
        let handle = GetClipboardData(13);
        let result = if handle.is_null() {
            None
        } else {
            let count = (GlobalSize(handle) / 2).min(128);
            let data = GlobalLock(handle);
            if data.is_null() {
                None
            } else {
                let chars = std::slice::from_raw_parts(data, count);
                let end = chars.iter().position(|v| *v == 0).unwrap_or(count);
                let text = String::from_utf16(&chars[..end]).ok();
                GlobalUnlock(handle);
                text
            }
        };
        CloseClipboard();
        result
    }
}
#[cfg(not(target_os = "windows"))]
pub fn clipboard() -> Option<String> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn seed_entry_replaces_selection_validates_and_accepts_signed_extremes() {
        let mut state = Title::new(Some(1));
        state.insert("-9223372036854775808");
        assert_eq!(state.seed_value(), Ok(i64::MIN));
        state.key(KeyCode::KeyA, None, true);
        state.insert("9223372036854775807");
        assert_eq!(state.seed_value(), Ok(i64::MAX));
        state.key(KeyCode::Backspace, None, false);
        assert_eq!(state.seed, "922337203685477580");
        state.selected = true;
        state.insert("9223372036854775808");
        assert!(state.seed_value().is_err());
        state.selected = true;
        state.insert("123abc");
        assert_eq!(state.seed, "9223372036854775808");
        state.key(KeyCode::Delete, None, false);
        assert!(state.seed_value().is_ok());
        assert!(state.seed.is_empty());
    }
    #[test]
    fn seed_and_create_hit_targets_follow_window_scaling() {
        for size in [(800, 600), (1440, 900), (3840, 2160)] {
            let layout = Layout::new(size);
            for (action, rect) in controls(&layout) {
                assert_eq!(
                    action_at(
                        size,
                        [
                            (rect.x + rect.w / 2.0) * layout.scale,
                            (rect.y + rect.h / 2.0) * layout.scale
                        ]
                    ),
                    Some(action)
                );
            }
        }
    }
}
