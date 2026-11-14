use crate::{
    hud::{Hud, HudVertex},
    ui::{Layout, Rect},
    ui_style::{self as style, ACCENT, MUTED, TEXT},
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

const FORM_W: f32 = 400.0;

fn controls(layout: &Layout) -> [(Action, Rect); 3] {
    let x = (layout.width - FORM_W) / 2.0;
    let y = layout.height / 2.0 - 4.0;
    [
        (
            Action::Seed,
            Rect {
                x,
                y,
                w: FORM_W,
                h: 48.0,
            },
        ),
        (
            Action::Create,
            Rect {
                x,
                y: y + 92.0,
                w: FORM_W,
                h: 52.0,
            },
        ),
        (
            Action::Quit,
            Rect {
                x,
                y: y + 152.0,
                w: FORM_W,
                h: 46.0,
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

fn shade(c: [f32; 4], gain: f32) -> [f32; 4] {
    [c[0] * gain, c[1] * gain, c[2] * gain, c[3]]
}

/// Dusk sky over a side-on slice of blocky hills.
fn backdrop(hud: &mut Hud, layout: &Layout) {
    let (w, h) = (layout.width, layout.height);
    style::gradient(
        hud,
        Rect {
            x: 0.0,
            y: 0.0,
            w,
            h,
        },
        [0.07, 0.09, 0.16, 1.0],
        [0.20, 0.24, 0.33, 1.0],
    );
    // A few square clouds drifting high up.
    for (cx, cy, cw) in [
        (0.12, 0.14, 7.0),
        (0.68, 0.09, 9.0),
        (0.86, 0.24, 5.0),
        (0.2, 0.34, 6.0),
    ] {
        let (x, y) = (cx * w, cy * h);
        hud.rect(x, y, cw * 18.0, 18.0, [0.75, 0.80, 0.90, 0.07]);
        hud.rect(
            x + 36.0,
            y - 18.0,
            (cw - 3.0) * 18.0,
            18.0,
            [0.75, 0.80, 0.90, 0.07],
        );
    }
    let size = 40.0;
    let grass = style::Stone {
        light: [0.55, 0.76, 0.39, 1.0],
        base: [0.36, 0.56, 0.24, 1.0],
        dark: [0.24, 0.39, 0.15, 1.0],
    };
    let dirt = style::Stone {
        light: [0.54, 0.39, 0.25, 1.0],
        base: [0.42, 0.29, 0.17, 1.0],
        dark: [0.29, 0.20, 0.11, 1.0],
    };
    let columns = (w / size).ceil() as i32 + 1;
    for i in 0..columns {
        let t = i as f32;
        let height = (2.8 + 1.4 * (t * 0.37).sin() + 0.9 * (t * 0.91 + 1.3).sin())
            .round()
            .max(1.0);
        let top = h - height * size;
        let x = i as f32 * size;
        let mut y = top;
        let mut depth = 0;
        while y < h {
            let stone = match depth {
                0 => grass,
                1 | 2 => dirt,
                _ => style::BUTTON,
            };
            // Deeper rows fade towards the sky colour so the form stays legible.
            let gain = 0.62 - depth as f32 * 0.06;
            let dim = style::Stone {
                light: shade(stone.light, gain),
                base: shade(stone.base, gain),
                dark: shade(stone.dark, gain),
            };
            style::bevel(
                hud,
                Rect {
                    x,
                    y,
                    w: size,
                    h: size,
                },
                dim,
                3.0,
            );
            if depth == 0 {
                // Dirt shows beneath the grass cap.
                let d = shade(dirt.base, gain);
                hud.rect(x + 3.0, y + 14.0, size - 6.0, size - 17.0, d);
                hud.rect(x + 3.0, y + 14.0, size - 6.0, 3.0, shade(grass.dark, gain));
            }
            y += size;
            depth += 1;
        }
    }
    style::gradient(
        hud,
        Rect {
            x: 0.0,
            y: h * 0.55,
            w,
            h: h * 0.45,
        },
        [0.0, 0.0, 0.0, 0.0],
        [0.0, 0.0, 0.0, 0.55],
    );
}

pub fn vertices(size: (u32, u32), state: &Title) -> Vec<HudVertex> {
    let layout = Layout::new(size);
    let mut hud = Hud::new(layout.width, layout.height);
    backdrop(&mut hud, &layout);
    let [(.., field), (.., create), (.., quit)] = controls(&layout);
    let center = layout.width / 2.0;
    let panel = Rect {
        x: field.x - 28.0,
        y: field.y - 56.0,
        w: FORM_W + 56.0,
        h: quit.y + quit.h + 24.0 - (field.y - 56.0),
    };
    let logo_y = panel.y - 128.0;
    crate::ui_icons::block(
        &mut hud,
        center - 26.0,
        logo_y - 62.0,
        52.0,
        crate::world::GRASS,
    );
    style::centered(
        &mut hud,
        "MiniCraft",
        center + 2.0,
        logo_y + 4.0,
        64.0,
        [0.0, 0.0, 0.0, 0.45],
        true,
    );
    style::centered(&mut hud, "MiniCraft", center, logo_y, 64.0, TEXT, true);
    style::tracked_centered(
        &mut hud,
        "6.25 CM VOXELS",
        center,
        logo_y + 76.0,
        11.0,
        4.0,
        ACCENT,
    );
    style::panel(&mut hud, panel);
    style::tracked(
        &mut hud,
        "WORLD SEED",
        field.x,
        field.y - 26.0,
        11.0,
        3.0,
        TEXT,
    );
    if state.focused {
        hud.rect(
            field.x - 2.0,
            field.y - 2.0,
            field.w + 4.0,
            field.h + 4.0,
            TEXT,
        );
    }
    style::well(&mut hud, field);
    if state.selected && state.focused && !state.seed.is_empty() {
        hud.rect(
            field.x + 12.0,
            field.y + 10.0,
            Hud::text_width(&state.seed, 18.0, true) + 4.0,
            28.0,
            [0.24, 0.39, 0.15, 1.0],
        );
    }
    if state.seed.is_empty() {
        hud.text("Random", field.x + 14.0, field.y + 14.0, 18.0, MUTED);
    } else {
        style::text(
            &mut hud,
            &state.seed,
            field.x + 14.0,
            field.y + 14.0,
            18.0,
            TEXT,
            true,
        );
    }
    if state.focused && !state.selected {
        hud.rect(
            field.x + 15.0 + Hud::text_width(&state.seed, 18.0, true),
            field.y + 11.0,
            3.0,
            26.0,
            ACCENT,
        );
    }
    if state.error.is_some() {
        style::text(
            &mut hud,
            "Seed must be a signed 64-bit whole number.",
            field.x,
            field.y + 60.0,
            12.0,
            [0.97, 0.58, 0.48, 1.0],
            false,
        );
    } else {
        style::text(
            &mut hud,
            "Leave blank for a random seed.",
            field.x,
            field.y + 60.0,
            12.0,
            MUTED,
            false,
        );
    }
    let pointer = state.pointer.map(|v| v / layout.scale);
    for (rect, label, primary) in [(create, "Create world", true), (quit, "Quit game", false)] {
        style::button(
            &mut hud,
            style::inset(rect, 3.0),
            primary,
            rect.contains(pointer),
        );
        style::centered(
            &mut hud,
            label,
            center,
            rect.y + (rect.h - 18.0) / 2.0 - 1.0,
            18.0,
            TEXT,
            true,
        );
    }
    style::text(
        &mut hud,
        &format!("v{}", env!("CARGO_PKG_VERSION")),
        20.0,
        layout.height - 30.0,
        12.0,
        MUTED,
        false,
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
