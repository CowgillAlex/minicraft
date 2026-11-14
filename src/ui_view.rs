use crate::{
    hud::Hud,
    ui::{Action, Button, Layout, Panel, Rect},
    ui_style::{self as style, MUTED, TEXT},
    view_settings::{Change, ViewSettings},
};

pub fn buttons(layout: &Layout) -> Vec<Button> {
    let p = layout.panel(Panel::View);
    let mut buttons = Vec::new();
    for (y, left, right) in [
        (104.0, Change::Distance(-1), Change::Distance(1)),
        (302.0, Change::Start(-1), Change::Start(1)),
        (386.0, Change::Density(-1), Change::Density(1)),
    ] {
        for (x, label, change) in [(p.x + 24.0, "-", left), (p.x + p.w - 68.0, "+", right)] {
            buttons.push(Button {
                rect: Rect {
                    x,
                    y: p.y + y,
                    w: 44.0,
                    h: 40.0,
                },
                label,
                action: Action::ViewChange(change),
                primary: false,
            });
        }
    }
    for (y, label, action) in [
        (
            166.0,
            "Adaptive terrain detail",
            Action::ViewChange(Change::Lod),
        ),
        (220.0, "Fog", Action::ViewChange(Change::Fog)),
        (466.0, "Restore defaults", Action::ViewChange(Change::Reset)),
        (520.0, "Back", Action::BackToSettings),
    ] {
        buttons.push(Button {
            rect: Rect {
                x: p.x + 24.0,
                y: p.y + y,
                w: p.w - 48.0,
                h: 44.0,
            },
            label,
            action,
            primary: false,
        });
    }
    buttons
}

pub fn draw(hud: &mut Hud, layout: &Layout, view: ViewSettings) {
    let p = layout.panel(Panel::View);
    for (y, label, value) in [
        (80.0, "Render distance", format!("{:.0} m", view.distance())),
        (278.0, "Fog starts at", format!("{:.0} m", view.fog_start())),
        (362.0, "Fog strength", format!("{}%", view.density())),
    ] {
        style::text(hud, label, p.x + 26.0, p.y + y, 13.0, MUTED, false);
        let well = Rect {
            x: p.x + 76.0,
            y: p.y + y + 24.0,
            w: p.w - 152.0,
            h: 40.0,
        };
        style::well(hud, well);
        style::centered(
            hud,
            &value,
            p.x + p.w / 2.0,
            well.y + 11.0,
            16.0,
            TEXT,
            true,
        );
    }
    style::text(
        hud,
        "Longer distances need more GPU memory.",
        p.x + 26.0,
        p.y + 442.0,
        12.0,
        MUTED,
        false,
    );
}
