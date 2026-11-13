//! The command palette and quick switcher: which items show, and what they do.

use rune::Tx;

use crate::app::{App, Mode, PalMode};
use crate::keys::k;
use crate::md;

pub type Run = fn(&mut App, &mut Tx);

pub struct Cmd {
    pub label: String,
    pub keys: &'static [&'static str],
    pub run: Run,
}

fn cmd(label: impl Into<String>, keys: &'static [&'static str], run: Run) -> Cmd {
    Cmd { label: label.into(), keys, run }
}

pub fn commands(s: &App, dark: bool) -> Vec<Cmd> {
    let has = s.active.is_some();
    let mut v = vec![
        cmd("New note", &["Mod", "N"], |s, tx| s.new_note(None, tx)),
        cmd("New folder", &[], |s, tx| s.start_new_folder("", tx)),
        cmd("Open today's journal entry", &[], |s, tx| s.open_today(tx)),
        cmd("Open quick switcher", &["Mod", "O"], |s, tx| s.open_palette(PalMode::Files, tx)),
    ];
    if has {
        let label = if s.mode == Mode::Source { "Show preview" } else { "Show source" };
        v.push(cmd(label, &["Mod", "E"], |s, tx| s.toggle_mode(tx)));
    }
    v.push(cmd("Search all notes", &["Mod", "Shift", "F"], |s, tx| s.open_search(tx)));
    if has {
        let label = if s.focus { "Leave focus mode" } else { "Enter focus mode" };
        v.push(cmd(label, &["Mod", "."], |s, tx| s.toggle_focus(tx)));
    }
    let left = if s.persist.left_open { "Hide sidebar" } else { "Show sidebar" };
    v.push(cmd(left, &["Mod", "\\"], |s, _| s.toggle_left()));
    let right = if s.persist.right_open { "Hide outline" } else { "Show outline" };
    v.push(cmd(right, &[], |s, _| s.toggle_right()));
    if dark {
        v.push(cmd("Use light theme", &[], |s, _| s.set_theme("light")));
    } else {
        v.push(cmd("Use dark theme", &[], |s, _| s.set_theme("dark")));
    }
    if has {
        v.push(cmd("Rename note", &[], |s, tx| s.start_rename(tx)));
        let label = if s.active.as_deref().is_some_and(|a| s.is_pinned(a)) { "Unpin note" } else { "Pin note" };
        v.push(cmd(label, &[], |s, _| {
            if let Some(a) = s.active.clone() {
                s.toggle_pin(&a);
            }
        }));
        v.push(cmd("Delete note", &[], |s, _| s.delete_note()));
        v.push(cmd("Close tab", &[], |s, _| {
            if let Some(a) = s.active.clone() {
                s.close_tab(&a);
            }
        }));
    }
    if s.sync_ready() {
        v.push(cmd("Sync now", &[], |s, _| s.sync_now()));
    }
    if s.active.is_some() {
        v.push(cmd("Insert image…", &["Mod", "Shift", "I"], |s, tx| s.pick_image(tx)));
    }
    if let Some(a) = s.active.as_deref() {
        if s.note_unlocked(a) {
            v.push(cmd("Lock note now", &[], |s, _| s.relock()));
            v.push(cmd("Remove password", &[], |s, tx| s.remove_password(tx)));
        } else if !s.note_locked(a) {
            v.push(cmd("Lock note with a password…", &[], |s, tx| s.ask_lock(tx)));
        }
    }
    v.push(cmd("Open settings", &["Mod", ","], |s, _| s.open_settings()));
    if !cfg!(target_os = "macos") {
        v.push(cmd("Quit", &["Mod", "Q"], |s, tx| s.quit(tx)));
    }
    v.push(cmd("Switch folder…", &[], |s, tx| s.go_welcome(tx)));
    v.push(cmd("Restore sample notes", &[], |s, tx| s.reset_sample(tx)));
    v
}

#[derive(Clone)]
pub enum Action {
    Open(String),
    Create(String),
    Cmd(Run),
}

pub struct Item {
    /// Label pieces; true marks matched characters.
    pub segs: Vec<(String, bool)>,
    pub sub: String,
    pub action: Action,
}

pub struct PalData {
    pub mode: PalMode,
    pub items: Vec<Item>,
    pub idx: usize,
}

/// Splits `s` into runs of matched and unmatched characters.
pub fn segs(s: &str, idx: &[usize]) -> Vec<(String, bool)> {
    let mut out: Vec<(String, bool)> = Vec::new();
    for (i, c) in s.chars().enumerate() {
        let hit = idx.contains(&i);
        match out.last_mut() {
            Some((t, h)) if *h == hit => t.push(c),
            _ => out.push((c.to_string(), hit)),
        }
    }
    out
}

pub fn pal_data(s: &App, dark: bool) -> Option<PalData> {
    let p = s.palette.as_ref()?;
    let mut mode = p.mode;
    let mut q = p.q.text.as_str();
    if mode == PalMode::Files && q.starts_with('>') {
        mode = PalMode::Commands;
        q = &q[1..];
    }
    let q = q.trim();
    let mut items = Vec::new();
    if mode == PalMode::Files {
        let files = s.files();
        let list: Vec<(String, Vec<usize>)> = if q.is_empty() {
            let rec: Vec<&String> = s.recent.iter().filter(|x| files.contains_key(*x)).collect();
            let mut rest: Vec<&String> = files.keys().filter(|x| !rec.contains(x)).collect();
            rest.sort();
            rec.into_iter().chain(rest).map(|x| (x.clone(), vec![])).collect()
        } else {
            let mut scored: Vec<(f32, String, Vec<usize>)> = files
                .keys()
                .filter_map(|x| {
                    if let Some((sc, idx)) = md::fuzzy(q, md::base(x)) {
                        return Some((sc + 500.0, x.clone(), idx));
                    }
                    md::fuzzy(q, md::strip_md(x)).map(|(sc, _)| (sc, x.clone(), vec![]))
                })
                .collect();
            scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
            scored.into_iter().map(|(_, x, i)| (x, i)).collect()
        };
        for (x, idx) in list.into_iter().take(50) {
            items.push(Item { segs: segs(md::base(&x), &idx), sub: md::dir(&x).to_string(), action: Action::Open(x) });
        }
        if !q.is_empty() && !files.keys().any(|x| md::base(x).to_lowercase() == q.to_lowercase()) {
            items.push(Item { segs: vec![(format!("Create note “{q}”"), false)], sub: "new".into(), action: Action::Create(q.to_string()) });
        }
    } else {
        let cmds = commands(s, dark);
        let mut list: Vec<(f32, Cmd, Vec<usize>)> = if q.is_empty() {
            cmds.into_iter().map(|c| (0.0, c, vec![])).collect()
        } else {
            cmds.into_iter().filter_map(|c| md::fuzzy(q, &c.label).map(|(sc, idx)| (sc, c, idx))).collect()
        };
        if !q.is_empty() {
            list.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        }
        for (_, c, idx) in list {
            let sub = if c.keys.is_empty() { String::new() } else { k(c.keys) };
            items.push(Item { segs: segs(&c.label, &idx), sub, action: Action::Cmd(c.run) });
        }
    }
    let idx = p.idx.min(items.len().saturating_sub(1));
    Some(PalData { mode, items, idx })
}

pub fn run(s: &mut App, action: Action, tx: &mut Tx) {
    match action {
        Action::Open(p) => s.open_file(&p, None, tx),
        Action::Create(q) => {
            s.palette = None;
            s.new_note(Some(&q), tx);
        }
        Action::Cmd(f) => {
            s.palette = None;
            f(s, tx);
        }
    }
}
