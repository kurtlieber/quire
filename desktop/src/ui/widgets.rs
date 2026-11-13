//! Small pieces the screens share: the design's icons, segmented controls and switches.

use rune::{Color, El, Icon, Shadow, TextStyle, Tx, el, icon, row, text};

use crate::app::App;
use crate::theme::{self, Pal, mono, sans};

macro_rules! svg {
    ($name:ident, $view:expr, $body:expr) => {
        pub static $name: Icon = Icon { name: stringify!($name), view: $view, body: $body };
    };
}

svg!(SIDEBAR_LEFT, 16.0, r#"<rect x="2" y="3" width="12" height="10" rx="2" fill="none" stroke="currentColor" stroke-width="1.3"/><path d="M6 3v10" fill="none" stroke="currentColor" stroke-width="1.3"/>"#);
svg!(SIDEBAR_RIGHT, 16.0, r#"<rect x="2" y="3" width="12" height="10" rx="2" fill="none" stroke="currentColor" stroke-width="1.3"/><path d="M10 3v10" fill="none" stroke="currentColor" stroke-width="1.3"/>"#);
svg!(CLOSE, 8.0, r#"<path d="M1 1l6 6M7 1L1 7" fill="none" stroke="currentColor" stroke-width="1.3" stroke-linecap="round"/>"#);
svg!(CLOSE_THIN, 8.0, r#"<path d="M1 1l6 6M7 1L1 7" fill="none" stroke="currentColor" stroke-width="1.2" stroke-linecap="round"/>"#);
svg!(PLUS, 12.0, r#"<path d="M6 1.5v9M1.5 6h9" fill="none" stroke="currentColor" stroke-width="1.3" stroke-linecap="round"/>"#);
svg!(CHEVRON_DOWN, 10.0, r#"<path d="M2 3.5l3 3 3-3" fill="none" stroke="currentColor" stroke-width="1.3" stroke-linecap="round" stroke-linejoin="round"/>"#);
svg!(CHEVRON_RIGHT, 8.0, r#"<path d="M2.5 1l3 3-3 3" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round"/>"#);
svg!(FOLDER, 16.0, r#"<path d="M2 4.5A1.5 1.5 0 0 1 3.5 3h2.6l1.5 1.6h4.9A1.5 1.5 0 0 1 14 6.1v5.4a1.5 1.5 0 0 1-1.5 1.5h-9A1.5 1.5 0 0 1 2 11.5z" fill="none" stroke="currentColor" stroke-width="1.25" stroke-linejoin="round"/>"#);
svg!(FOLDER_OPEN, 16.0, r#"<path d="M2 11.5v-7A1.5 1.5 0 0 1 3.5 3h2.6l1.5 1.6h3.9A1.5 1.5 0 0 1 13 6.1V7" fill="none" stroke="currentColor" stroke-width="1.25" stroke-linejoin="round"/><path d="M2.3 12.3l1.5-4.1A1.5 1.5 0 0 1 5.2 7.2h8.2a.9.9 0 0 1 .85 1.2l-1.3 3.6a1.5 1.5 0 0 1-1.4 1H3.4a1.2 1.2 0 0 1-1.1-.7z" fill="none" stroke="currentColor" stroke-width="1.25" stroke-linejoin="round"/>"#);
svg!(PIN, 16.0, r#"<path d="M5.5 2.5h5M6.5 2.5v3.9L4.5 9h7l-2-2.6V2.5M8 9v4.5" fill="none" stroke="currentColor" stroke-width="1.25" stroke-linecap="round" stroke-linejoin="round"/>"#);
svg!(FOLDER_PLUS, 16.0, r#"<path d="M2 4.5A1.5 1.5 0 0 1 3.5 3h2.6l1.5 1.6h4.9A1.5 1.5 0 0 1 14 6.1v5.4a1.5 1.5 0 0 1-1.5 1.5h-9A1.5 1.5 0 0 1 2 11.5z" fill="none" stroke="currentColor" stroke-width="1.25" stroke-linejoin="round"/><path d="M8 7v4M6 9h4" fill="none" stroke="currentColor" stroke-width="1.25" stroke-linecap="round"/>"#);
svg!(SEARCH, 16.0, r#"<circle cx="7" cy="7" r="4.5" fill="none" stroke="currentColor" stroke-width="1.5"/><path d="M10.5 10.5L14 14" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"/>"#);
svg!(EYE, 16.0, r#"<path d="M1.5 8s2.4-4.5 6.5-4.5S14.5 8 14.5 8s-2.4 4.5-6.5 4.5S1.5 8 1.5 8z" fill="none" stroke="currentColor" stroke-width="1.3" stroke-linejoin="round"/><circle cx="8" cy="8" r="2" fill="none" stroke="currentColor" stroke-width="1.3"/>"#);
// The formatting toolbar.
svg!(BOLD, 16.0, r#"<path d="M4.8 2.8h3.9a2.6 2.6 0 0 1 0 5.2H4.8zM4.8 8h4.6a2.6 2.6 0 0 1 0 5.2H4.8z" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linejoin="round"/>"#);
svg!(ITALIC, 16.0, r#"<path d="M7 2.8h5.2M3.8 13.2H9M9.6 2.8L6.4 13.2" fill="none" stroke="currentColor" stroke-width="1.3" stroke-linecap="round"/>"#);
svg!(STRIKE, 16.0, r#"<path d="M2.8 8h10.4M11 4.4c-.5-.9-1.6-1.6-3-1.6-1.8 0-3 1-3 2.3 0 .9.5 1.5 1.4 1.9M5 11.6c.5.9 1.6 1.6 3 1.6 1.8 0 3.1-1 3.1-2.4 0-.5-.1-.9-.4-1.2" fill="none" stroke="currentColor" stroke-width="1.3" stroke-linecap="round"/>"#);
svg!(HEADING, 16.0, r#"<path d="M4 3v10M12 3v10M4 8h8" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round"/>"#);
svg!(LINK, 16.0, r#"<path d="M6.9 9.1a2.6 2.6 0 0 0 3.7 0l2.1-2.1a2.6 2.6 0 0 0-3.7-3.7l-.7.7M9.1 6.9a2.6 2.6 0 0 0-3.7 0L3.3 9a2.6 2.6 0 0 0 3.7 3.7l.7-.7" fill="none" stroke="currentColor" stroke-width="1.3" stroke-linecap="round"/>"#);
svg!(WIKI, 16.0, r#"<path d="M4.6 3H2.6v10h2M7.2 3H5.4v10h1.8M8.8 3h1.8v10H8.8M11.4 3h2v10h-2" fill="none" stroke="currentColor" stroke-width="1.2" stroke-linecap="round" stroke-linejoin="round"/>"#);
svg!(BULLETS, 16.0, r#"<circle cx="3.2" cy="4" r="1" fill="currentColor"/><circle cx="3.2" cy="8" r="1" fill="currentColor"/><circle cx="3.2" cy="12" r="1" fill="currentColor"/><path d="M6.2 4h7.6M6.2 8h7.6M6.2 12h7.6" fill="none" stroke="currentColor" stroke-width="1.3" stroke-linecap="round"/>"#);
svg!(NUMBERS, 16.0, r#"<path d="M2.5 2.9l1-.6v3.4M2.2 7.5c.2-.5.6-.8 1.1-.8.6 0 1 .4 1 .9 0 .8-2.1 1.5-2.1 2.4h2.2M6.6 4h7.2M6.6 8h7.2M6.6 12h7.2" fill="none" stroke="currentColor" stroke-width="1.2" stroke-linecap="round" stroke-linejoin="round"/>"#);
svg!(TASK, 16.0, r#"<rect x="2.6" y="2.6" width="10.8" height="10.8" rx="2.4" fill="none" stroke="currentColor" stroke-width="1.3"/><path d="M5.3 8.2l1.9 1.9 3.6-4" fill="none" stroke="currentColor" stroke-width="1.3" stroke-linecap="round" stroke-linejoin="round"/>"#);
svg!(QUOTE, 16.0, r#"<path d="M3.2 3v10M6.4 4.6h7M6.4 8h7M6.4 11.4h4.6" fill="none" stroke="currentColor" stroke-width="1.3" stroke-linecap="round"/>"#);
// A padlock: lock a note with a password.
svg!(LOCK, 16.0, r#"<rect x="3.4" y="7" width="9.2" height="6.6" rx="1.6" fill="none" stroke="currentColor" stroke-width="1.3"/><path d="M5.4 7V5.2a2.6 2.6 0 0 1 5.2 0V7" fill="none" stroke="currentColor" stroke-width="1.3" stroke-linecap="round"/>"#);
// A picture with a hill and a sun: insert an image.
svg!(IMAGE, 16.0, r#"<rect x="2" y="3" width="12" height="10" rx="2" fill="none" stroke="currentColor" stroke-width="1.3"/><circle cx="10.5" cy="6.2" r="1.2" fill="none" stroke="currentColor" stroke-width="1.2"/><path d="M2.6 11.6l3.6-3.7 2.6 2.6 1.6-1.5 3 2.8" fill="none" stroke="currentColor" stroke-width="1.3" stroke-linejoin="round" stroke-linecap="round"/>"#);
// Four corners of a frame: focus mode.
svg!(FOCUS, 16.0, r#"<path d="M2.5 6V4a1.5 1.5 0 0 1 1.5-1.5h2M10 2.5h2A1.5 1.5 0 0 1 13.5 4v2M13.5 10v2a1.5 1.5 0 0 1-1.5 1.5h-2M6 13.5H4A1.5 1.5 0 0 1 2.5 12v-2" fill="none" stroke="currentColor" stroke-width="1.3" stroke-linecap="round" stroke-linejoin="round"/>"#);
svg!(CODE, 16.0, r#"<path d="M5.5 4.5L2 8l3.5 3.5M10.5 4.5L14 8l-3.5 3.5" fill="none" stroke="currentColor" stroke-width="1.3" stroke-linecap="round" stroke-linejoin="round"/>"#);
// The logo's mark: three sheets in a stack, the violet one in front (assets/brand).
svg!(MARK, 100.0, r##"<rect x="38" y="10" width="46" height="60" rx="6" fill="#e1dbcf"/><rect x="27" y="20" width="46" height="60" rx="6" fill="#c9bfae"/><rect x="16" y="30" width="46" height="60" rx="6" fill="#7a4fba"/>"##);
svg!(MARK_ON_INK, 100.0, r##"<rect x="38" y="10" width="46" height="60" rx="6" fill="#3a352e"/><rect x="27" y="20" width="46" height="60" rx="6" fill="#5f584e"/><rect x="16" y="30" width="46" height="60" rx="6" fill="#b597e8"/>"##);
svg!(CHECK, 10.0, r##"<path d="M1.5 5.2l2.4 2.4L8.5 2.6" fill="none" stroke="#ffffff" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"/>"##);

/// A one-line text. Monospaced text gets its exact CSS width (0.6 em a character), because
/// Rune rounds text boxes up to device pixels and those fractions add up along a row.
pub fn label(s: impl Into<String>, style: TextStyle, c: Color) -> El {
    let s: String = s.into();
    let mono = matches!(style.family, rune::Family::Named("IBM Plex Mono" | "Courier Prime" | "Spline Sans Mono"));
    let n = s.chars().count();
    let ascii = in_mono_face(&s);
    let e = text(s).font(style).color(c);
    // Only ASCII is sure to come from the mono face; arrows fall back to other fonts.
    if mono && n > 0 && ascii { e.w(n as f32 * 0.6 * style.size) } else { e }
}

/// Whether every character is in the mono face, so it is exactly 0.6 em wide.
fn in_mono_face(s: &str) -> bool {
    s.chars().all(|c| c.is_ascii() || "↑↓←→↵·…“”‘’".contains(c))
}

/// A label with padding inside its exact width (`extra` adds borders that take room).
pub fn padded(s: impl Into<String>, style: TextStyle, c: Color, px: f32, py: f32, extra: f32) -> El {
    let s: String = s.into();
    let mono = matches!(style.family, rune::Family::Named("IBM Plex Mono" | "Courier Prime" | "Spline Sans Mono"));
    let n = s.chars().count();
    let ascii = in_mono_face(&s);
    let e = text(s).font(style).color(c).px(px).py(py);
    if mono && n > 0 && ascii { e.w(n as f32 * 0.6 * style.size + 2.0 * px + extra) } else { e }
}

/// A square icon button: 28 by 28, radius 6, muted, ink on hover.
pub fn icon_button(p: &Pal, key: &str, glyph: &'static Icon, size: f32, help: &str) -> El {
    row()
        .key(key)
        .size(28.0, 28.0)
        .center()
        .radius(6.0)
        .hover(p.hover)
        .hover_group()
        .label(help)
        .help(help)
        .child(icon(glyph, size).color(p.mute).group_color(p.ink))
}

fn seg_shadow(on: bool) -> Vec<Shadow> {
    if on { theme::seg_shadows().to_vec() } else { Vec::new() }
}

/// One button of a segmented control.
pub struct Seg {
    pub label: String,
    pub on: bool,
    pub run: Box<dyn Fn(&mut App, &mut Tx)>,
}

pub fn seg(label: impl Into<String>, on: bool, run: impl Fn(&mut App, &mut Tx) + 'static) -> Seg {
    Seg { label: label.into(), on, run: Box::new(run) }
}

/// The design's segmented control: a tinted track with raised buttons.
pub fn segmented(p: &Pal, key: &str, segs: Vec<Seg>, h: f32, pad_x: f32, radius: f32, style: TextStyle, track: Color, track_radius: f32) -> El {
    let mut r = row().key(key).gap(2.0).padding(2.0).radius(track_radius).fill(track).role("radiogroup");
    for (i, sg) in segs.into_iter().enumerate() {
        let on = sg.on;
        let run = sg.run;
        let mut b = row()
            .key(format!("{key}-{i}"))
            .h(h)
            .px(pad_x)
            .center()
            .radius(radius)
            .fill(if on { p.bg } else { Color::TRANSPARENT })
            .role("radio")
            .checked(on)
            .label(sg.label.clone())
            .on_press_tx(move |s: &mut App, tx| run(s, tx))
            .child(label(sg.label, style, if on { p.ink } else { p.mute }));
        for sh in seg_shadow(on) {
            b = b.box_shadow(sh);
        }
        r = r.child(b);
    }
    r
}

/// The settings segmented control: 26 tall, 12.5 px Instrument Sans, on the side colour.
pub fn settings_seg(p: &Pal, key: &str, segs: Vec<Seg>) -> El {
    segmented(p, key, segs, 26.0, 12.0, 6.0, sans(12.5, 400.0), p.side, 8.0)
}

/// The settings segmented control in mono (MCP transport).
pub fn settings_seg_mono(p: &Pal, key: &str, segs: Vec<Seg>) -> El {
    segmented(p, key, segs, 26.0, 12.0, 6.0, mono(12.0, 400.0), p.side, 8.0)
}

/// A 36 by 20 switch.
pub fn switch(p: &Pal, key: &str, on: bool, label_text: &str, run: impl Fn(&mut App, &mut Tx) + 'static) -> El {
    el().key(key)
        .size(36.0, 20.0)
        .radius(10.0)
        .fill(if on { p.button } else { p.mark })
        .role("switch")
        .checked(on)
        .label(label_text)
        .on_press_tx(move |s: &mut App, tx| run(s, tx))
        .child(el().key("knob").size(16.0, 16.0).radius(8.0).fill(Color::WHITE).box_shadow(theme::knob_shadow()).absolute(if on { 18.0 } else { 2.0 }, 2.0))
}

/// A bordered button such as "Switch folder…" or "Restore".
pub fn outline_button(p: &Pal, key: &str, text_: &str, h: f32, run: impl Fn(&mut App, &mut Tx) + 'static) -> El {
    row()
        .key(key)
        .h(h)
        .px(12.0)
        .center()
        .radius(7.0)
        .border(1.0, p.line)
        .border_layout()
        .hover(p.hover)
        .role("button")
        .label(text_)
        .on_press_tx(move |s: &mut App, tx| run(s, tx))
        .child(label(text_, sans(12.5, 400.0), p.ink))
}

/// The solid purple button.
pub fn primary_button(p: &Pal, key: &str, text_: &str, h: f32, px: f32, size: f32, run: impl Fn(&mut App, &mut Tx) + 'static) -> El {
    row()
        .key(key)
        .h(h)
        .px(px)
        .center()
        .radius(8.0)
        .fill(p.button)
        .hover(p.button_hover)
        .role("button")
        .label(text_)
        .on_press_tx(move |s: &mut App, tx| run(s, tx))
        .child(label(text_, sans(size, 500.0), p.on_acc))
}

/// The design's window dots. On Linux they are the window's close, minimize and maximize
/// buttons. macOS draws its own traffic lights in the same place, so there they leave room.
pub fn dots(p: &Pal) -> El {
    if cfg!(target_os = "macos") {
        return el().key("dots").size(52.0, 12.0);
    }
    // Windows draws its own buttons at the other end of the title bar. On Linux the window
    // manager closes and places the window, and `Ctrl+Q` quits.
    if cfg!(any(target_os = "windows", target_os = "linux")) {
        return el().key("dots");
    }
    let dot = |key: &str, help: &str, run: fn(&mut App, &mut Tx)| {
        el().key(key).size(12.0, 12.0).radius(6.0).fill(p.mark).label(help).help(help).role("button").on_press_tx(move |s: &mut App, tx| run(s, tx))
    };
    row()
        .key("dots")
        .gap(8.0)
        .center_y()
        .child(dot("win-close", "Close", |s, tx| {
            s.flush_all();
            tx.close_window();
        }))
        .child(dot("win-min", "Minimize", |_, tx| tx.minimize()))
        .child(dot("win-max", "Maximize", |_, tx| tx.toggle_maximized()))
}
