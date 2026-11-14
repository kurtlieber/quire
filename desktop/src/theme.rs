//! Colours and type from the design. Values are the design's CSS values.

use std::sync::OnceLock;

use rune::{Color, Family, Shadow, TextStyle};

pub const SANS: Family = Family::Named("Instrument Sans");
pub const SERIF: Family = Family::Named("Newsreader");
pub const MONO: Family = Family::Named("IBM Plex Mono");


pub const FONTS: &[&[u8]] = &[
    include_bytes!("../../assets/fonts/InstrumentSans-VF.ttf"),
    include_bytes!("../../assets/fonts/InstrumentSans-Italic-VF.ttf"),
    include_bytes!("../../assets/fonts/Newsreader-VF.ttf"),
    include_bytes!("../../assets/fonts/Newsreader-Italic-VF.ttf"),
    include_bytes!("../../assets/fonts/IBMPlexMono-Regular.ttf"),
    include_bytes!("../../assets/fonts/IBMPlexMono-Italic.ttf"),
    include_bytes!("../../assets/fonts/IBMPlexMono-Medium.ttf"),
    include_bytes!("../../assets/fonts/IBMPlexMono-MediumItalic.ttf"),
    include_bytes!("../../assets/fonts/IBMPlexMono-SemiBold.ttf"),
    include_bytes!("../../assets/fonts/IBMPlexMono-SemiBoldItalic.ttf"),
    include_bytes!("../../assets/fonts/IBMPlexMono-Bold.ttf"),
    include_bytes!("../../assets/fonts/CourierPrime-Regular.ttf"),
    include_bytes!("../../assets/fonts/CourierPrime-Italic.ttf"),
    include_bytes!("../../assets/fonts/CourierPrime-Bold.ttf"),
    include_bytes!("../../assets/fonts/CourierPrime-BoldItalic.ttf"),
    include_bytes!("../../assets/fonts/SplineSansMono-VF.ttf"),
    include_bytes!("../../assets/fonts/SplineSansMono-Italic-VF.ttf"),
];

/// An editor font choice: its id in settings, its label and its family.
pub struct EditorFont {
    pub id: &'static str,
    pub label: &'static str,
    pub family: Family,
    pub name: &'static str,
}

pub const EDITOR_FONTS: [EditorFont; 3] = [
    EditorFont { id: "plex", label: "IBM Plex Mono", family: Family::Named("IBM Plex Mono"), name: "IBM Plex Mono" },
    EditorFont { id: "courier", label: "Courier Prime", family: Family::Named("Courier Prime"), name: "Courier Prime" },
    EditorFont { id: "spline", label: "Spline Sans Mono", family: Family::Named("Spline Sans Mono"), name: "Spline Sans Mono" },
];

pub fn editor_font(id: &str) -> &'static EditorFont {
    EDITOR_FONTS.iter().find(|f| f.id == id).unwrap_or(&EDITOR_FONTS[0])
}

/// sRGB hex with a straight alpha, as CSS `rgba()` writes it.
pub fn rgba(hex: &str, a: f32) -> Color {
    rune::color::hex(hex).unwrap_or(Color::BLACK).alpha(a)
}

pub fn hex(h: &str) -> Color {
    rune::color::hex(h).unwrap_or(Color::BLACK)
}

#[derive(Clone, Copy, Debug)]
#[allow(dead_code)]
pub struct Pal {
    pub dark: bool,
    /// The design's backdrop around its mock window; the app window has none.
    pub desk: Color,
    pub bg: Color,
    pub side: Color,
    pub ink: Color,
    pub ink2: Color,
    pub mute: Color,
    pub mark: Color,
    pub line: Color,
    pub hover: Color,
    pub act: Color,
    pub acc: Color,
    pub soft: Color,
    pub code: Color,
    pub err: Color,
    /// The solid accent of primary buttons and switches, the same in both themes.
    pub button: Color,
    pub button_hover: Color,
    pub white: Color,
    /// `rgba(138,95,201,.4)`: the underline of a resolved wiki link.
    pub link_line: Color,
    /// Text and icons on the accent colour (primary buttons, ticked boxes).
    pub on_acc: Color,
    /// The selection in single-line fields, and in the editor.
    pub sel: Color,
    pub sel_editor: Color,
    pub scrim_palette: Color,
    pub scrim_settings: Color,
}

pub fn pal(dark: bool) -> &'static Pal {
    static L: OnceLock<Pal> = OnceLock::new();
    static D: OnceLock<Pal> = OnceLock::new();
    if dark {
        D.get_or_init(|| Pal {
            dark: true,
            desk: hex("#0f0e0d"),
            bg: hex("#1f1d1a"),
            side: hex("#181715"),
            ink: hex("#e9e3d8"),
            ink2: hex("#bdb5a7"),
            mute: hex("#948c7f"),
            mark: hex("#5f584e"),
            line: hex("#322e28"),
            hover: rgba("#fff0d2", 0.05),
            act: rgba("#fff0d2", 0.08),
            acc: hex("#b597e8"),
            soft: hex("#2f2839"),
            code: hex("#27241f"),
            err: hex("#e8907c"),
            button: hex("#7d52bd"),
            button_hover: hex("#6c44a8"),
            white: Color::WHITE,
            link_line: rgba("#8a5fc9", 0.4),
            on_acc: Color::WHITE,
            sel: rgba("#8a5fc9", 0.22),
            sel_editor: rgba("#8a5fc9", 0.26),
            scrim_palette: rgba("#281e0f", 0.16),
            scrim_settings: rgba("#281e0f", 0.22),
        })
    } else {
        L.get_or_init(|| Pal {
            dark: false,
            desk: hex("#dcd6ca"),
            bg: hex("#f8f5ef"),
            side: hex("#efebe3"),
            ink: hex("#2a2723"),
            ink2: hex("#5b554d"),
            mute: hex("#776f64"),
            mark: hex("#b2a999"),
            line: hex("#e1dbcf"),
            hover: rgba("#463214", 0.06),
            act: rgba("#463214", 0.09),
            acc: hex("#7a4fba"),
            soft: hex("#ebe3f5"),
            code: hex("#efe9de"),
            err: hex("#b0432f"),
            button: hex("#7d52bd"),
            button_hover: hex("#6c44a8"),
            white: Color::WHITE,
            link_line: rgba("#8a5fc9", 0.4),
            on_acc: Color::WHITE,
            sel: rgba("#8a5fc9", 0.22),
            sel_editor: rgba("#8a5fc9", 0.26),
            scrim_palette: rgba("#281e0f", 0.16),
            scrim_settings: rgba("#281e0f", 0.22),
        })
    }
}

static SCALE: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0x3f80_0000);

/// The window's backing scale, set by the view before it builds.
pub fn set_scale(s: f32) {
    SCALE.store(s.max(0.5).to_bits(), std::sync::atomic::Ordering::Relaxed);
}

pub fn scale() -> f32 {
    f32::from_bits(SCALE.load(std::sync::atomic::Ordering::Relaxed))
}

/// Rounds to the device pixel grid.
pub fn px(v: f32) -> f32 {
    let s = scale();
    (v * s).round() / s
}

/// CSS `line-height: normal` as Chromium computes it: ascent and descent each rounded to
/// device pixels.
pub fn normal_lh(family: Family, size: f32) -> f32 {
    let (a, d) = metrics(family);
    px(a * size) + px(d * size)
}

/// Ascent and descent of a family, as fractions of the font size.
pub fn metrics(family: Family) -> (f32, f32) {
    match family {
        f if f == SANS => (0.97, 0.25),
        f if f == SERIF => (0.735, 0.265),
        Family::Named("Courier Prime") => (0.781, 0.342),
        Family::Named("Spline Sans Mono") => (0.964, 0.237),
        _ => (1.025, 0.275),
    }
}

/// Instrument Sans at `size` with CSS `line-height: normal`.
pub fn sans(size: f32, weight: f32) -> TextStyle {
    TextStyle::new(size, weight).family(SANS).lh(normal_lh(SANS, size))
}

/// IBM Plex Mono at `size` with CSS `line-height: normal`.
pub fn mono(size: f32, weight: f32) -> TextStyle {
    TextStyle::new(size, weight).family(MONO).lh(normal_lh(MONO, size))
}

pub fn serif(size: f32, weight: f32) -> TextStyle {
    TextStyle::new(size, weight).family(SERIF).lh(normal_lh(SERIF, size))
}

/// `0 1px 2px rgba(40,28,10,.14), 0 0 0 1px rgba(40,28,10,.05)`: the raised segment.
pub fn seg_shadows() -> [Shadow; 2] {
    [
        Shadow { x: 0.0, y: 1.0, blur: 2.0, spread: 0.0, color: rgba("#281c0a", 0.14), inset: false },
        Shadow { x: 0.0, y: 0.0, blur: 0.0, spread: 1.0, color: rgba("#281c0a", 0.05), inset: false },
    ]
}

/// `0 0 0 1px rgba(30,20,5,.10), 0 14px 34px -10px rgba(40,28,10,.35)`: the folder menu.
pub fn menu_shadows() -> [Shadow; 2] {
    [
        Shadow { x: 0.0, y: 0.0, blur: 0.0, spread: 1.0, color: rgba("#1e1405", 0.10), inset: false },
        Shadow { x: 0.0, y: 14.0, blur: 34.0, spread: -10.0, color: rgba("#281c0a", 0.35), inset: false },
    ]
}

/// `0 0 0 1px rgba(30,20,5,.10), 0 24px 60px -16px rgba(40,28,10,.45)`: the palette.
pub fn palette_shadows() -> [Shadow; 2] {
    [
        Shadow { x: 0.0, y: 0.0, blur: 0.0, spread: 1.0, color: rgba("#1e1405", 0.10), inset: false },
        Shadow { x: 0.0, y: 24.0, blur: 60.0, spread: -16.0, color: rgba("#281c0a", 0.45), inset: false },
    ]
}

/// `0 0 0 1px rgba(30,20,5,.08), 0 30px 80px -20px rgba(40,28,10,.45)`: the settings sheet.
pub fn settings_shadows() -> [Shadow; 2] {
    [
        Shadow { x: 0.0, y: 0.0, blur: 0.0, spread: 1.0, color: rgba("#1e1405", 0.08), inset: false },
        Shadow { x: 0.0, y: 30.0, blur: 80.0, spread: -20.0, color: rgba("#281c0a", 0.45), inset: false },
    ]
}

/// `0 1px 2px rgba(0,0,0,.25)`: a switch knob.
pub fn knob_shadow() -> Shadow {
    Shadow { x: 0.0, y: 1.0, blur: 2.0, spread: 0.0, color: rgba("#000000", 0.25), inset: false }
}

// ---------- Omarchy ----------

/// The folder of the current Omarchy theme, when this desktop is Omarchy.
pub fn omarchy_dir() -> Option<std::path::PathBuf> {
    let home = std::path::PathBuf::from(std::env::var_os("HOME")?);
    [".local/state/omarchy/current/theme", ".config/omarchy/current/theme"]
        .iter()
        .map(|d| home.join(d))
        .find(|d| d.join("colors.toml").is_file())
}

/// `key = "#rrggbb"` lines of an Omarchy `colors.toml`.
fn parse_colors(src: &str) -> std::collections::HashMap<String, String> {
    src.lines()
        .filter_map(|l| {
            let (k, v) = l.split_once('=')?;
            Some((k.trim().to_string(), v.trim().trim_matches('"').to_string()))
        })
        .collect()
}

fn contrast(a: Color, b: Color) -> f32 {
    let (la, lb) = (a.luminance() + 0.05, b.luminance() + 0.05);
    la.max(lb) / la.min(lb)
}

/// Typo's palette made from an Omarchy theme: its background is the paper, its foreground
/// the ink, its accent the accent. The rest mixes from those, as the design's tones do.
pub fn omarchy_pal(src: &str) -> Option<Pal> {
    let m = parse_colors(src);
    let c = |k: &str| m.get(k).and_then(|v| rune::color::hex(v));
    let bg = c("background")?;
    let fg = c("foreground")?;
    let acc = c("accent").or_else(|| c("blue"))?;
    let dark = match m.get("mode").map(String::as_str) {
        Some("light") => false,
        Some("dark") => true,
        _ => bg.luminance() < 0.25,
    };
    let toward = if dark { Color::BLACK } else { fg };
    let side = c("dark_background").unwrap_or_else(|| bg.mix(toward, if dark { 0.25 } else { 0.04 }));
    let on_acc = [Color::WHITE, Color::BLACK, bg].into_iter().max_by(|a, b| contrast(*a, acc).total_cmp(&contrast(*b, acc))).unwrap_or(Color::WHITE);
    let hover_a = if dark { 0.05 } else { 0.06 };
    Some(Pal {
        dark,
        desk: c("darker_background").unwrap_or(side),
        bg,
        side,
        ink: fg,
        ink2: c("light_foreground").unwrap_or_else(|| fg.mix(bg, 0.25)),
        mute: c("dark_foreground").unwrap_or_else(|| fg.mix(bg, 0.45)),
        mark: c("muted").unwrap_or_else(|| fg.mix(bg, 0.65)),
        line: bg.mix(fg, 0.12),
        hover: fg.alpha(hover_a),
        act: fg.alpha(hover_a + 0.03),
        acc,
        soft: bg.mix(acc, 0.18),
        code: bg.mix(fg, 0.05),
        err: c("red").unwrap_or(if dark { hex("#e8907c") } else { hex("#b0432f") }),
        button: acc,
        button_hover: acc.mix(bg, 0.15),
        white: Color::WHITE,
        link_line: acc.alpha(0.4),
        on_acc,
        sel: acc.alpha(0.24),
        sel_editor: acc.alpha(0.3),
        scrim_palette: if dark { Color::BLACK.alpha(0.3) } else { rgba("#281e0f", 0.16) },
        scrim_settings: if dark { Color::BLACK.alpha(0.38) } else { rgba("#281e0f", 0.22) },
    })
}

static OMARCHY: std::sync::RwLock<Option<&'static Pal>> = std::sync::RwLock::new(None);

/// Reads the current Omarchy theme again. True when the palette changed.
pub fn reload_omarchy() -> bool {
    let next = omarchy_dir().and_then(|d| std::fs::read_to_string(d.join("colors.toml")).ok()).and_then(|s| omarchy_pal(&s));
    let mut slot = OMARCHY.write().unwrap_or_else(std::sync::PoisonError::into_inner);
    let same = match (&*slot, &next) {
        (Some(a), Some(b)) => format!("{a:?}") == format!("{b:?}"),
        (None, None) => true,
        _ => false,
    };
    if !same {
        // A theme switch leaks one small palette, so views can keep a plain reference.
        *slot = next.map(|p| &*Box::leak(Box::new(p)));
    }
    !same
}

/// The Omarchy palette, if this desktop runs Omarchy.
pub fn omarchy() -> Option<&'static Pal> {
    *OMARCHY.read().unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// A built-in theme. Paper and Ink are the design's own palettes. The others take the main
/// colours of well-known editor themes, and the remaining tones mix from those.
pub struct ThemeDef {
    pub id: &'static str,
    pub name: &'static str,
    pub dark: bool,
    /// Background, sidebar, ink, second ink, muted text, faint marks, accent, error.
    pub colors: [&'static str; 8],
}

pub const THEMES: &[ThemeDef] = &[
    ThemeDef { id: "paper", name: "Paper", dark: false, colors: ["#f8f5ef", "#efebe3", "#2a2723", "#5b554d", "#776f64", "#b2a999", "#7a4fba", "#b0432f"] },
    ThemeDef { id: "white", name: "White", dark: false, colors: ["#ffffff", "#f5f5f3", "#1f1e1c", "#474540", "#6b6863", "#aeaaa3", "#7a4fba", "#b0432f"] },
    ThemeDef { id: "solarized-light", name: "Solarized Light", dark: false, colors: ["#fdf6e3", "#eee8d5", "#073642", "#586e75", "#657b83", "#93a1a1", "#268bd2", "#dc322f"] },
    ThemeDef { id: "gruvbox-light", name: "Gruvbox Light", dark: false, colors: ["#fbf1c7", "#f2e5bc", "#3c3836", "#504945", "#7c6f64", "#a89984", "#af3a03", "#9d0006"] },
    ThemeDef { id: "catppuccin-latte", name: "Catppuccin Latte", dark: false, colors: ["#eff1f5", "#e6e9ef", "#4c4f69", "#5c5f77", "#6c6f85", "#9ca0b0", "#8839ef", "#d20f39"] },
    ThemeDef { id: "rose-pine-dawn", name: "Rosé Pine Dawn", dark: false, colors: ["#faf4ed", "#f2e9e1", "#575279", "#686388", "#797593", "#9893a5", "#907aa9", "#b4637a"] },
    ThemeDef { id: "ink", name: "Ink", dark: true, colors: ["#1f1d1a", "#181715", "#e9e3d8", "#bdb5a7", "#948c7f", "#5f584e", "#b597e8", "#e8907c"] },
    ThemeDef { id: "black", name: "Black", dark: true, colors: ["#000000", "#0a0a0a", "#e8e6e3", "#b8b5b0", "#8c8984", "#4a4845", "#b597e8", "#e8907c"] },
    ThemeDef { id: "nord", name: "Nord", dark: true, colors: ["#2e3440", "#272c36", "#eceff4", "#d8dee9", "#9aa3b5", "#4c566a", "#88c0d0", "#bf616a"] },
    ThemeDef { id: "gruvbox-dark", name: "Gruvbox Dark", dark: true, colors: ["#282828", "#1d2021", "#ebdbb2", "#d5c4a1", "#a89984", "#665c54", "#fe8019", "#fb4934"] },
    ThemeDef { id: "catppuccin-mocha", name: "Catppuccin Mocha", dark: true, colors: ["#1e1e2e", "#181825", "#cdd6f4", "#bac2de", "#9399b2", "#585b70", "#cba6f7", "#f38ba8"] },
    ThemeDef { id: "tokyo-night", name: "Tokyo Night", dark: true, colors: ["#1a1b26", "#16161e", "#c0caf5", "#a9b1d6", "#7a7fa0", "#414868", "#7aa2f7", "#f7768e"] },
    ThemeDef { id: "rose-pine", name: "Rosé Pine", dark: true, colors: ["#1f1d2e", "#191724", "#e0def4", "#bcb9d6", "#908caa", "#6e6a86", "#c4a7e7", "#eb6f92"] },
    ThemeDef { id: "solarized-dark", name: "Solarized Dark", dark: true, colors: ["#002b36", "#00232c", "#eee8d5", "#93a1a1", "#839496", "#586e75", "#268bd2", "#dc322f"] },
];

/// The accents the settings offer, with their names. The first one keeps the theme's own.
pub const ACCENTS: &[(&str, &str)] = &[
    ("", "Theme accent"),
    ("#7a4fba", "Violet"),
    ("#2f6fdf", "Blue"),
    ("#1f8a80", "Teal"),
    ("#3f8f3a", "Green"),
    ("#c27c0e", "Amber"),
    ("#d4622a", "Orange"),
    ("#c8414b", "Red"),
    ("#c2417f", "Pink"),
    ("#6b6b6b", "Graphite"),
];

/// A `#rgb` or `#rrggbb` colour, or None.
pub fn parse_accent(s: &str) -> Option<Color> {
    let t = s.trim();
    let t = if t.starts_with('#') { t.to_string() } else { format!("#{t}") };
    if !matches!(t.len(), 4 | 7) || !t[1..].chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    rune::color::hex(&t)
}

/// `c` moved toward white on a dark background, or toward black on a light one, until text
/// in it reads on `bg` (a contrast of at least `min`).
fn readable(c: Color, bg: Color, dark: bool, min: f32) -> Color {
    let toward = if dark { Color::WHITE } else { Color::BLACK };
    let mut out = c;
    let mut t = 0.0;
    while contrast(out, bg) < min && t < 1.0 {
        t += 0.05;
        out = c.mix(toward, t);
    }
    out
}

/// A palette with another accent: links, tags, buttons, the selection and the soft fills.
pub fn with_accent(base: &Pal, c: Color) -> Pal {
    let mut p = *base;
    p.acc = readable(c, base.bg, base.dark, 3.0);
    p.button = c;
    p.button_hover = c.mix(Color::BLACK, 0.14);
    p.soft = base.bg.mix(c, if base.dark { 0.2 } else { 0.12 });
    p.link_line = c.alpha(0.4);
    p.on_acc = [Color::WHITE, Color::BLACK].into_iter().max_by(|a, b| contrast(*a, c).total_cmp(&contrast(*b, c))).unwrap_or(Color::WHITE);
    p.sel = c.alpha(0.22);
    p.sel_editor = c.alpha(0.26);
    p
}

fn build(t: &ThemeDef) -> Pal {
    let [bg, side, ink, ink2, mute, mark, acc, err] = t.colors.map(hex);
    let dark = t.dark;
    let hover_a = if dark { 0.05 } else { 0.06 };
    let base = Pal {
        dark,
        desk: side,
        bg,
        side,
        ink,
        ink2,
        mute,
        mark,
        line: bg.mix(ink, if dark { 0.1 } else { 0.11 }),
        hover: ink.alpha(hover_a),
        act: ink.alpha(hover_a + 0.03),
        acc,
        soft: bg,
        code: bg.mix(ink, 0.045),
        err,
        button: acc,
        button_hover: acc,
        white: Color::WHITE,
        link_line: acc,
        on_acc: Color::WHITE,
        sel: acc,
        sel_editor: acc,
        scrim_palette: if dark { Color::BLACK.alpha(0.3) } else { rgba("#281e0f", 0.16) },
        scrim_settings: if dark { Color::BLACK.alpha(0.38) } else { rgba("#281e0f", 0.22) },
    };
    with_accent(&base, acc)
}

/// The palette of a theme id. Paper and Ink are the design's palettes, unchanged. `omarchy`
/// is the current Omarchy theme. An unknown id gives Paper or Ink.
pub fn theme_pal(id: &str, dark: bool) -> &'static Pal {
    static BUILT: OnceLock<Vec<Pal>> = OnceLock::new();
    match id {
        "paper" => pal(false),
        "ink" => pal(true),
        "omarchy" => omarchy().unwrap_or_else(|| pal(dark)),
        _ => {
            let built = BUILT.get_or_init(|| THEMES.iter().map(build).collect());
            THEMES.iter().position(|t| t.id == id).map_or_else(|| pal(dark), |i| &built[i])
        }
    }
}

/// `theme_pal` with the accent from the settings, if one is set. Each new pair is made once.
pub fn themed(id: &str, dark: bool, accent: &str) -> &'static Pal {
    let base = theme_pal(id, dark);
    let Some(c) = parse_accent(accent).filter(|_| !accent.trim().is_empty()) else { return base };
    type Cache = std::collections::HashMap<(String, String, bool), &'static Pal>;
    static CACHE: std::sync::Mutex<Option<Cache>> = std::sync::Mutex::new(None);
    let mut g = CACHE.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    let map = g.get_or_insert_with(Default::default);
    // The Omarchy palette can change, so its entry keys on its colours too.
    let key = (format!("{id}{}", if id == "omarchy" { format!("{:?}", base.bg) } else { String::new() }), accent.trim().to_lowercase(), dark);
    map.entry(key).or_insert_with(|| Box::leak(Box::new(with_accent(base, c))))
}

#[cfg(test)]
mod theme_tests {
    use super::*;

    #[test]
    fn every_theme_reads() {
        for t in THEMES {
            let p = theme_pal(t.id, t.dark);
            assert_eq!(p.dark, t.dark, "{}", t.id);
            eprintln!("{:18} ink {:5.2} ink2 {:5.2} mute {:5.2} accent {:5.2}", t.id, contrast(p.ink, p.bg), contrast(p.ink2, p.bg), contrast(p.mute, p.bg), contrast(p.acc, p.bg));
            // WCAG AA: 4.5 for body text. Muted text is for labels and hints.
            assert!(contrast(p.ink, p.bg) >= 4.5, "{} ink {}", t.id, contrast(p.ink, p.bg));
            assert!(contrast(p.ink2, p.bg) >= 4.5, "{} ink2 {}", t.id, contrast(p.ink2, p.bg));
            assert!(contrast(p.mute, p.bg) >= 3.0, "{} mute {}", t.id, contrast(p.mute, p.bg));
            assert!(contrast(p.acc, p.bg) >= 3.0, "{} accent {}", t.id, contrast(p.acc, p.bg));
        }
    }

    #[test]
    fn a_dark_accent_gets_lighter_on_a_dark_theme() {
        let p = themed("ink", true, "#3a1f6e");
        assert!(contrast(p.acc, p.bg) >= 3.0);
        assert_eq!(format!("{:?}", p.button), format!("{:?}", hex("#3a1f6e")));
        assert!(parse_accent("#abc").is_some() && parse_accent("12ab9f").is_some());
        assert!(parse_accent("#12ab9").is_none() && parse_accent("blue").is_none());
        assert!(std::ptr::eq(themed("nord", true, ""), theme_pal("nord", true)));
    }
}
