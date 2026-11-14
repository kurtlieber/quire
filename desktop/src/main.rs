//! Typo: a plain editor for folders of Markdown files.
// On Windows the app opens no console window. `typo mcp` still talks over the pipes its
// client gives it.
#![cfg_attr(windows, windows_subsystem = "windows")]

mod app;
mod cli;
mod commands;
mod folders;
mod format;
mod html2md;
mod images;
mod keys;
mod lock;
mod lock_app;
mod mcp;
mod md;
mod pins;
mod sample;
mod store;
mod sync;
mod theme;
mod ui;
mod update;
mod vault;
mod watch;

use rune::AppSpec;

use app::App;

fn scene_welcome(s: &mut App) {
    s.screen = app::Screen::Welcome;
    s.refresh_counts();
}
fn scene_preview(s: &mut App) {
    s.mode = app::Mode::Preview;
}
fn scene_dark(s: &mut App) {
    s.persist.settings.theme = "dark".into();
}
fn scene_focus(s: &mut App) {
    s.focus = true;
}
fn scene_settings(s: &mut App) {
    s.settings_open = true;
}

/// Text rasterization. `TYPO_SMOOTHING` picks another mode, for comparing renders.
fn smoothing() -> rune::FontSmoothing {
    use rune::FontSmoothing as F;
    match std::env::var("TYPO_SMOOTHING").as_deref() {
        Ok("windfoil-chromium") => F::WindfoilChromium,
        Ok("hinted") => F::Hinted,
        Ok("hinted-gpu") => F::HintedGpu,
        Ok("slug") => F::Slug,
        Ok("windfoil") => F::Windfoil,
        Ok("off") => F::Off,
        Ok("on") => F::On,
        _ => default_smoothing(),
    }
}

fn default_smoothing() -> rune::FontSmoothing {
    // Each platform's browser text, since the design is drawn in a browser.
    if cfg!(target_os = "macos") {
        rune::FontSmoothing::On
    } else if cfg!(target_os = "windows") {
        rune::FontSmoothing::DirectWrite
    } else {
        rune::FontSmoothing::HintedGpu
    }
}

fn main() {
    store::move_from_quire();
    if cli::run_subcommand() {
        return;
    }
    rune::prewarm_gpu();
    rune::set_app_id("typo");
    // The Dock icon on macOS, and the title bar and taskbar icon on Windows. Linux takes the
    // icon from the launcher entry.
    if !cfg!(target_os = "linux") {
        let _ = rune::set_app_icon(&include_bytes!("../packaging/typo-512.png")[..]);
    }
    let spec = AppSpec {
        min_size: Some((520.0, 400.0)),
        fonts: theme::FONTS,
        families: ("Instrument Sans", "IBM Plex Mono"),
        on_key: Some(keys::on_key),
        poll: Some(|s: &mut App, t, tx| s.poll(t, tx)),
        setup: Some(|s: &mut App, waker| s.set_waker(waker)),
        scenes: &[("welcome", scene_welcome), ("preview", scene_preview), ("dark", scene_dark), ("focus", scene_focus), ("settings", scene_settings)],
        ..AppSpec::new("Typo", (1280.0, 800.0), app::init, ui::view)
    };
    rune::App::new(spec)
        .font_smoothing(smoothing())
        .chrome(rune::Chrome { full_size_content: true, traffic_lights: Some((14.0, 17.0)), drag_regions: true })
        .titlebar(rune::Titlebar::new(42.0))
        .on_close(|s: &mut App, _tx| s.flush_all())
        .run();
}

#[cfg(test)]
pub(crate) mod scroll_tests {
    use rune::{Env, Runtime};

    /// Tests that set HOME and the XDG folders hold this lock, because the variables are global.
    pub(crate) static ENV: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[test]
    fn settings_body_scrolls() {
        let _env = ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("typo-scroll-{}", std::process::id()));
        // SAFETY: tests in this module run before any thread reads these variables.
        unsafe {
            std::env::set_var("HOME", &dir);
            std::env::set_var("XDG_CONFIG_HOME", dir.join("cfg"));
            std::env::set_var("XDG_DATA_HOME", dir.join("data"));
        }
        let mut s = crate::app::init();
        s.settings_open = true;
        s.set_sec = crate::app::Sec::Mcp;
        s.persist.settings.mcp.enabled = true;
        let mut rt = Runtime::new(s, crate::ui::view, (1280.0, 800.0), Env { system_dark: false, scale: 1.0 });
        for _ in 0..3 {
            rt.frame(1.0 / 60.0);
        }
        let before = rt.presented("client").map(|p| p.0[1]);
        let (x, y) = rt.find("set-scroll").unwrap();
        rt.pointer_move(x, y);
        rt.wheel(x, y, -300.0);
        for _ in 0..30 {
            rt.frame(1.0 / 60.0);
        }
        let after = rt.presented("client").map(|p| p.0[1]);
        eprintln!("client y before {before:?} after {after:?}");
        assert!(after < before, "the settings body scrolls");
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[cfg(test)]
mod theme_choice_tests {
    /// Each mode uses its own pick, Paper and Ink are the defaults, and the accent recolours
    /// the palette.
    #[test]
    fn each_mode_uses_its_own_theme() {
        let _env = crate::scroll_tests::ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("typo-themes-{}", std::process::id()));
        // SAFETY: the ENV lock keeps other tests that read these variables out.
        unsafe {
            std::env::set_var("HOME", &dir);
            std::env::set_var("XDG_CONFIG_HOME", dir.join("cfg"));
            std::env::set_var("XDG_DATA_HOME", dir.join("data"));
        }
        let mut s = crate::app::init();
        assert_eq!((s.theme_id(false), s.theme_id(true)), ("paper".to_string(), "ink".to_string()));
        s.persist.settings.light_theme = Some("gruvbox-light".into());
        s.persist.settings.dark_theme = Some("nord".into());
        s.persist.settings.theme = "system".into();
        assert!(std::ptr::eq(s.palette(true), crate::theme::theme_pal("nord", true)));
        assert!(std::ptr::eq(s.palette(false), crate::theme::theme_pal("gruvbox-light", false)));
        s.persist.settings.theme = "light".into();
        assert!(!s.palette(true).dark, "Light mode ignores the system");
        s.persist.settings.accent = "#1f8a80".into();
        assert_eq!(format!("{:?}", s.palette(false).button), format!("{:?}", crate::theme::hex("#1f8a80")));
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[cfg(test)]
mod toolbar_tests {
    use rune::{Env, Runtime};

    #[test]
    fn toolbar_actions_edit_the_selection() {
        let _env = crate::scroll_tests::ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("typo-toolbar-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        // SAFETY: the ENV lock keeps other tests that read these variables out.
        unsafe {
            std::env::set_var("HOME", &dir);
            std::env::set_var("XDG_CONFIG_HOME", dir.join("cfg"));
            std::env::set_var("XDG_DATA_HOME", dir.join("data"));
        }
        let mut rt = Runtime::new(crate::app::init(), crate::ui::view, (1280.0, 800.0), Env { system_dark: false, scale: 1.0 });
        rt.frame(1.0 / 60.0);
        let mut out = String::new();
        rt.update(|s, tx| {
            s.mode = crate::app::Mode::Source;
            let text = "Intro — with a dash\nhello world".to_string();
            let a = s.active.clone().unwrap();
            s.set_text(&a, text.clone());
            let start = text.find("hello").unwrap();
            s.cur_edit().set_selection(start, text.len());
            s.format(crate::format::Fmt::Bold, tx);
            s.format(crate::format::Fmt::Bullet, tx);
            out = s.active_text().to_string();
        });
        assert_eq!(out, "Intro — with a dash\n- **hello world**");
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[cfg(test)]
mod lock_tests {
    use rune::{Env, Runtime};

    /// Lock a note, lock it again, unlock it, then remove the password. The folder copy and
    /// the file stay sealed until the password goes.
    #[test]
    fn a_note_locks_and_unlocks() {
        let _env = crate::scroll_tests::ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("typo-lock-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        // SAFETY: the ENV lock keeps other tests that read these variables out.
        unsafe {
            std::env::set_var("HOME", &dir);
            std::env::set_var("XDG_CONFIG_HOME", dir.join("cfg"));
            std::env::set_var("XDG_DATA_HOME", dir.join("data"));
        }
        let mut rt = Runtime::new(crate::app::init(), crate::ui::view, (1280.0, 800.0), Env { system_dark: false, scale: 1.0 });
        rt.frame(1.0 / 60.0);
        let set = |s: &mut crate::app::App, key: &str, v: &str| s.field(key, String::new).replace_text(v.to_string());
        rt.update(|s, tx| {
            s.mode = crate::app::Mode::Source;
            let a = s.active.clone().unwrap();
            s.set_text(&a, "# Secret\n\nThe code is 4711.".into());
            s.ask_lock(tx);
            set(s, "lock-pass", "short");
            set(s, "lock-repeat", "short");
            s.lock_note(tx);
            assert!(s.lock_dialog.as_deref().is_some_and(|e| e.contains("at least 8")), "a short password is refused");
            set(s, "lock-pass", "tiger lily");
            set(s, "lock-repeat", "tiger lily");
            s.lock_note(tx);
            assert!(s.lock_dialog.is_none());
            assert!(crate::lock::is_locked(&s.files()[&a]), "the folder copy is sealed");
            assert_eq!(s.active_text(), "# Secret\n\nThe code is 4711.", "the note stays open");
            // An edit while open is sealed too.
            s.set_text(&a, "# Secret\n\nThe code is 1234.".into());
            assert!(!s.files()[&a].contains("1234"));
            s.relock();
            assert!(s.note_locked(&a) && s.active_text().is_empty());
            set(s, "unlock-pass", "wrong password");
            s.unlock(tx);
            assert_eq!(s.unlock_error.as_deref(), Some("The password does not open this note."));
            set(s, "unlock-pass", "tiger lily");
            s.unlock(tx);
            assert_eq!(s.active_text(), "# Secret\n\nThe code is 1234.");
            s.remove_password(tx);
            assert_eq!(s.files()[&a], "# Secret\n\nThe code is 1234.");
            s.writer.wait(std::time::Duration::from_secs(5));
            let disk = std::fs::read_to_string(s.vault.as_ref().unwrap().path.join(&a)).unwrap();
            assert_eq!(disk, "# Secret\n\nThe code is 1234.");
        });
        let _ = std::fs::remove_dir_all(&dir);
    }
}
