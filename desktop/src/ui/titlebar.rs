//! The vault window's top band: window dots, sidebar toggle, tabs and view toggles.

use rune::{Color, El, PointerButton, el, icon, row, text};

use super::V;
use super::widgets::{CLOSE, CODE, EYE, FOCUS, PLUS, SIDEBAR_LEFT, SIDEBAR_RIGHT, dots, icon_button, label};
use crate::app::{App, Mode};
use crate::md;
use crate::theme::{mono, sans};

pub fn titlebar(v: V) -> El {
    if v.focus() { focus_bar(v) } else { bar(v) }
}

fn bar(v: V) -> El {
    let p = v.p;
    let s = v.s;
    let left_visible = v.left_visible();
    // The 38-point tabs sit on the bar's bottom edge, one point low, so their labels are centred
    // 24 points down, near the middle of the 42-point bar. The icons on both sides centre on the
    // same line.
    let mut left = row().key("left-zone").h(42.0).pt(6.5).pl(14.0).pr(8.0).gap(10.0).center_y();
    if left_visible {
        left = left.w(248.0);
    }
    left = left
        .child(dots(p))
        .child(el().grow(1.0))
        .child(icon_button(p, "toggle-left", &SIDEBAR_LEFT, 16.0, "Toggle sidebar").on_press(|s: &mut App| s.toggle_left()));

    // With the sidebar open, the first tab's left border sits on the line where the content starts.
    let mut tabs = row().key("tabs").grow(1.0).min_w(0.0).h(42.0).items_end().gap(2.0).pl(if left_visible { 0.0 } else { 6.0 }).pr(4.0).clip_rounded(0.0).offset(0.0, 1.0);
    for p_ in s.tabs.iter().filter(|t| s.files().contains_key(*t)) {
        tabs = tabs.child(tab(v, p_));
    }
    tabs = tabs.child(icon_button(p, "new-tab", &PLUS, 12.0, "New note").mb(4.75).ml(2.0).on_press_tx(|s: &mut App, tx| s.new_note(None, tx)));

    let mut right = row().key("right-zone").h(42.0).pt(6.5).pl(6.0).pr(10.0).gap(6.0).center_y();
    if let Some(rel) = &s.update.available {
        right = right.child(update_button(v, &rel.version));
    }
    if v.has_doc() {
        // One button flips between source and preview. Its icon shows where it goes.
        let (glyph, help) = if s.mode == Mode::Source {
            (&EYE, format!("Show preview ({})", crate::keys::k(&["Mod", "E"])))
        } else {
            (&CODE, format!("Show source ({})", crate::keys::k(&["Mod", "E"])))
        };
        right = right.child(icon_button(p, "mode", glyph, 16.0, &help).role("switch").checked(s.mode == Mode::Preview).on_press_tx(|s: &mut App, tx| s.toggle_mode(tx)));
        let help = format!("Focus mode ({})", crate::keys::k(&["Mod", "."]));
        right = right.child(icon_button(p, "focus", &FOCUS, 16.0, &help).on_press_tx(|s: &mut App, tx| s.toggle_focus(tx)));
    }
    right = right.child(icon_button(p, "toggle-right", &SIDEBAR_RIGHT, 16.0, "Toggle outline").on_press(|s: &mut App| s.toggle_right()));

    row()
        .key("titlebar")
        .h(42.0)
        .w_full()
        .window_drag()
        .paint_order(3)
        .child(left)
        .child(tabs)
        .child(right)
        .child(el().w(v.trailing))
}

/// The Update button: a newer release is out. A press installs it and restarts Typo.
fn update_button(v: V, version: &str) -> El {
    let p = v.p;
    let busy = v.s.update.applying();
    let help = format!("Typo {version} is out. Select to update.");
    let mut b = row()
        .key("update")
        .h(24.0)
        .px(10.0)
        .mr(4.0)
        .center_y()
        .radius(12.0)
        .fill(p.button)
        .role("button")
        .label(help.clone())
        .help(help)
        .child(label(if busy { "Updating…" } else { "Update" }, sans(12.5, 500.0), p.on_acc));
    if !busy {
        b = b.hover(p.button_hover).on_press_tx(|s: &mut App, tx| s.start_update(tx));
    }
    b
}

fn tab(v: V, path: &str) -> El {
    let p = v.p;
    let on = v.s.active.as_deref() == Some(path);
    let open_path = path.to_string();
    let close_path = path.to_string();
    let key = format!("tab:{path}");
    let mut t = row()
        .key(key)
        .h(38.0)
        .w(190.0)
        .shrink()
        .min_w(72.0)
        .pl(14.0)
        .pr(6.0)
        .gap(6.0)
        .center_y()
        .corners([9.0, 9.0, 0.0, 0.0])
        .fill(if on { p.bg } else { Color::TRANSPARENT })
        .hover_group()
        .role("tab")
        .label(md::base(path).to_string())
        .help(path.to_string())
        .pointer_buttons([PointerButton::Primary, PointerButton::Middle])
        .no_window_drag()
        .on_press_tx(move |s: &mut App, tx| {
            if tx.button() == PointerButton::Middle {
                s.close_tab(&open_path);
            } else if s.active.as_deref() != Some(open_path.as_str()) {
                s.open_file(&open_path, None, tx);
            }
        });
    // Every tab has a 1 px border that takes room, transparent unless active, as in CSS.
    t = t.border_sides([1.0, 1.0, 0.0, 1.0], if on { p.line } else { Color::TRANSPARENT }).border_layout();
    let name = text(md::base(path)).font(sans(13.0, 400.0)).color(if on { p.ink } else { p.mute }).group_color(p.ink).truncate().grow(1.0).shrink().min_w(0.0);
    let close = row()
        .key("x")
        .size(20.0, 20.0)
        .center()
        .radius(5.0)
        .hover(p.hover)
        .hover_group()
        .label("Close")
        .help("Close")
        .on_press(move |s: &mut App| s.close_tab(&close_path))
        .child(icon(&CLOSE, 8.0).color(p.mute).group_opacity(if on { 1.0 } else { 0.55 }, 1.0));
    t.child(name).child(close)
}

fn focus_bar(v: V) -> El {
    let p = v.p;
    let inner = row()
        .key("focus-inner")
        .h(42.0)
        .w_full()
        .pl(14.0)
        .gap(8.0)
        .center_y()
        .fill(p.bg)
        .group_opacity(0.0, 1.0)
        .child(dots(p))
        .child(el().grow(1.0))
        .child(
            row()
                .key("leave-focus")
                .h(26.0)
                .px(10.0)
                .mr(10.0)
                .center()
                .radius(6.0)
                .hover(p.hover)
                .hover_group()
                .role("button")
                .on_press_tx(|s: &mut App, tx| s.toggle_focus(tx))
                .child(label("leave focus · esc", mono(11.5, 400.0), p.mute).group_color(p.ink)),
        )
        .child(el().w(v.trailing));
    el().key("titlebar").h(42.0).w_full().window_drag().hover_group().hover_motion(rune::Spring::new(0.25, 0.0), rune::Spring::new(0.25, 0.0)).child(inner)
}
