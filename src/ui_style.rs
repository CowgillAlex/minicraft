//! Shared "carved stone" look: square bevelled panels and buttons with hard
//! shadows, plus the angled translucent hotbar slots used by the play HUD.
use crate::{hud::Hud, ui::Rect};

pub type Color = [f32; 4];

const fn rgb(hex: u32) -> Color {
    [
        ((hex >> 16) & 0xff) as f32 / 255.0,
        ((hex >> 8) & 0xff) as f32 / 255.0,
        (hex & 0xff) as f32 / 255.0,
        1.0,
    ]
}

pub const OUTLINE: Color = rgb(0x121418);
pub const TEXT: Color = rgb(0xf3efe2);
pub const MUTED: Color = rgb(0xa9aaa3);
pub const SHADOW: Color = [0.06, 0.065, 0.075, 0.9];
pub const ACCENT: Color = rgb(0x9be36b);
pub const KEY: Color = rgb(0xd8e8c4);

/// Light, base and dark tones of one bevelled surface.
#[derive(Clone, Copy)]
pub struct Stone {
    pub light: Color,
    pub base: Color,
    pub dark: Color,
}
pub const PANEL: Stone = Stone {
    light: rgb(0x5b616b),
    base: rgb(0x3a3e45),
    dark: rgb(0x24272c),
};
pub const BUTTON: Stone = Stone {
    light: rgb(0x9aa1ab),
    base: rgb(0x6c727c),
    dark: rgb(0x464b53),
};
pub const BUTTON_HOVER: Stone = Stone {
    light: rgb(0xb4bbc5),
    base: rgb(0x828995),
    dark: rgb(0x535963),
};
pub const PRIMARY: Stone = Stone {
    light: rgb(0x8cc263),
    base: rgb(0x5d8f3c),
    dark: rgb(0x3c6326),
};
pub const PRIMARY_HOVER: Stone = Stone {
    light: rgb(0xa3d77a),
    base: rgb(0x6ea64a),
    dark: rgb(0x467230),
};
/// Recessed wells: dark on top/left, lighter on bottom/right.
pub const WELL: Stone = Stone {
    light: rgb(0x1b1d21),
    base: rgb(0x26292e),
    dark: rgb(0x4a4f57),
};

pub fn inset(r: Rect, by: f32) -> Rect {
    Rect {
        x: r.x + by,
        y: r.y + by,
        w: (r.w - by * 2.0).max(0.0),
        h: (r.h - by * 2.0).max(0.0),
    }
}

/// A square block face lit from the top left.
pub fn bevel(hud: &mut Hud, r: Rect, stone: Stone, edge: f32) {
    hud.rect(r.x, r.y, r.w, r.h, stone.dark);
    hud.rect(r.x, r.y, r.w - edge, r.h - edge, stone.light);
    let face = inset(r, edge);
    hud.rect(face.x, face.y, face.w, face.h, stone.base);
}

/// Outlined, bevelled block with a hard drop shadow.
pub fn block(hud: &mut Hud, r: Rect, stone: Stone, edge: f32, border: f32, drop: f32) {
    if drop > 0.0 {
        hud.rect(
            r.x - border + drop,
            r.y - border + drop,
            r.w + border * 2.0,
            r.h + border * 2.0,
            [0.0, 0.0, 0.0, 0.4],
        );
    }
    hud.rect(
        r.x - border,
        r.y - border,
        r.w + border * 2.0,
        r.h + border * 2.0,
        OUTLINE,
    );
    bevel(hud, r, stone, edge);
}

pub fn panel(hud: &mut Hud, r: Rect) {
    block(hud, r, PANEL, 4.0, 4.0, 8.0);
}

pub fn well(hud: &mut Hud, r: Rect) {
    bevel(hud, r, WELL, 2.0);
}

pub fn button(hud: &mut Hud, r: Rect, primary: bool, hovered: bool) {
    let stone = match (primary, hovered) {
        (true, false) => PRIMARY,
        (true, true) => PRIMARY_HOVER,
        (false, false) => BUTTON,
        (false, true) => BUTTON_HOVER,
    };
    block(hud, r, stone, 3.0, 3.0, 0.0);
}

fn offset(size: f32) -> f32 {
    (size / 10.0).clamp(1.0, 3.0).round()
}

/// Text with a hard pixel shadow below and to the right.
pub fn text(hud: &mut Hud, s: &str, x: f32, y: f32, size: f32, color: Color, bold: bool) {
    let o = offset(size);
    hud.styled_text(s, x + o, y + o, size, SHADOW, bold);
    hud.styled_text(s, x, y, size, color, bold);
}

pub fn centered(hud: &mut Hud, s: &str, x: f32, y: f32, size: f32, color: Color, bold: bool) {
    text(
        hud,
        s,
        x - Hud::text_width(s, size, bold) / 2.0,
        y,
        size,
        color,
        bold,
    );
}

pub fn right(hud: &mut Hud, s: &str, x: f32, y: f32, size: f32, color: Color, bold: bool) {
    text(
        hud,
        s,
        x - Hud::text_width(s, size, bold),
        y,
        size,
        color,
        bold,
    );
}

/// Width of letter-spaced capitals drawn by [`tracked`].
pub fn tracked_width(s: &str, size: f32, spacing: f32) -> f32 {
    let n = s.chars().count() as f32;
    Hud::text_width(s, size, true) + spacing * (n - 1.0).max(0.0)
}

/// Bold letter-spaced text with a soft shadow, for HUD labels over the world.
pub fn tracked(hud: &mut Hud, s: &str, x: f32, y: f32, size: f32, spacing: f32, color: Color) {
    let mut cx = x;
    let mut buf = [0u8; 4];
    for c in s.chars() {
        let g = c.encode_utf8(&mut buf);
        for (dx, dy, alpha) in [(0.0, 1.5, 0.55), (1.0, 1.0, 0.45)] {
            hud.styled_text(g, cx + dx, y + dy, size, [0.0, 0.0, 0.0, alpha], true);
        }
        hud.styled_text(g, cx, y, size, color, true);
        cx += Hud::text_width(g, size, true) + spacing;
    }
}

pub fn tracked_centered(
    hud: &mut Hud,
    s: &str,
    x: f32,
    y: f32,
    size: f32,
    spacing: f32,
    color: Color,
) {
    tracked(
        hud,
        s,
        x - tracked_width(s, size, spacing) / 2.0,
        y,
        size,
        spacing,
        color,
    );
}

pub fn tracked_right(
    hud: &mut Hud,
    s: &str,
    x: f32,
    y: f32,
    size: f32,
    spacing: f32,
    color: Color,
) {
    tracked(
        hud,
        s,
        x - tracked_width(s, size, spacing),
        y,
        size,
        spacing,
        color,
    );
}

/// A chunky on/off switch: a recessed slot holding a bevelled knob.
pub fn switch(hud: &mut Hud, x: f32, y: f32, on: bool) {
    let slot = Rect {
        x,
        y,
        w: 48.0,
        h: 24.0,
    };
    hud.rect(
        slot.x - 2.0,
        slot.y - 2.0,
        slot.w + 4.0,
        slot.h + 4.0,
        OUTLINE,
    );
    bevel(
        hud,
        slot,
        if on {
            Stone {
                light: rgb(0x2d4a1c),
                base: rgb(0x3c6326),
                dark: rgb(0x5d8f3c),
            }
        } else {
            WELL
        },
        2.0,
    );
    let knob = Rect {
        x: if on { slot.x + 26.0 } else { slot.x + 2.0 },
        y: slot.y + 2.0,
        w: 20.0,
        h: 20.0,
    };
    bevel(hud, knob, if on { PRIMARY_HOVER } else { BUTTON }, 3.0);
}

/// Width of one keycap drawn by [`keycap`].
pub fn keycap_width(label: &str) -> f32 {
    Hud::text_width(label, 12.0, true).max(10.0) + 14.0
}

/// A small raised key, returning its width.
pub fn keycap(hud: &mut Hud, label: &str, x: f32, y: f32) -> f32 {
    let w = keycap_width(label);
    let r = Rect { x, y, w, h: 22.0 };
    hud.rect(r.x - 1.0, r.y - 1.0, r.w + 2.0, r.h + 2.0, OUTLINE);
    hud.rect(r.x, r.y, r.w, r.h, rgb(0x2c2f34));
    hud.rect(r.x, r.y, r.w, r.h - 3.0, rgb(0x6c727c));
    hud.rect(r.x + 1.0, r.y + 1.0, r.w - 2.0, r.h - 5.0, rgb(0x575c65));
    hud.centered(label, r.x + w / 2.0, r.y + 3.0, 12.0, TEXT, true);
    w
}

/// Angled translucent tile used by hotbar slots.
pub fn chamfer(hud: &mut Hud, r: Rect, cut: f32, color: Color) {
    hud.polygon(
        &[
            [r.x, r.y],
            [r.x + r.w - cut, r.y],
            [r.x + r.w, r.y + cut],
            [r.x + r.w, r.y + r.h],
            [r.x + cut, r.y + r.h],
            [r.x, r.y + r.h - cut],
        ],
        color,
    );
}

/// Right-pointing chevron hinting that a button opens another page.
pub fn chevron(hud: &mut Hud, x: f32, y: f32, color: Color) {
    for (dx, c) in [(1.5, SHADOW), (0.0, color)] {
        let (x, y) = (x + dx, y + dx);
        hud.polygon(
            &[[x, y], [x + 3.0, y], [x + 9.0, y + 6.0], [x + 6.0, y + 6.0]],
            c,
        );
        hud.polygon(
            &[
                [x + 6.0, y + 6.0],
                [x + 9.0, y + 6.0],
                [x + 3.0, y + 12.0],
                [x, y + 12.0],
            ],
            c,
        );
    }
}

/// Left-pointing chevron for "previous" controls.
pub fn chevron_left(hud: &mut Hud, x: f32, y: f32, color: Color) {
    for (dx, c) in [(1.5, SHADOW), (0.0, color)] {
        let (x, y) = (x + dx, y + dx);
        hud.polygon(
            &[[x + 6.0, y], [x + 9.0, y], [x + 3.0, y + 6.0], [x, y + 6.0]],
            c,
        );
        hud.polygon(
            &[
                [x, y + 6.0],
                [x + 3.0, y + 6.0],
                [x + 9.0, y + 12.0],
                [x + 6.0, y + 12.0],
            ],
            c,
        );
    }
}

/// Vertical gradient filling a rectangle.
pub fn gradient(hud: &mut Hud, r: Rect, top: Color, bottom: Color) {
    hud.gradient_rect(r, top, bottom);
}
