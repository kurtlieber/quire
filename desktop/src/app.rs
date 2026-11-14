//! Typo's state and the actions that change it.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use rune::{FileDialogOptions, TextEdit, Tx};

use crate::md::{self, Names};
use crate::store::{self, Persist, Session, Settings, VaultRef};
use crate::vault::{self, Op, Vault, Writer};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Screen {
    Welcome,
    Vault,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Source,
    Preview,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Panel {
    Files,
    Search,
    Tags,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PalMode {
    Files,
    Commands,
}

pub struct Palette {
    pub mode: PalMode,
    pub q: TextEdit,
    pub idx: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sec {
    Appearance,
    Editor,
    Files,
    Sync,
    Mcp,
    Keys,
}

pub const SECS: [(Sec, &str); 6] = [
    (Sec::Appearance, "Appearance"),
    (Sec::Editor, "Editor"),
    (Sec::Files, "Files & links"),
    (Sec::Sync, "Sync & backup"),
    (Sec::Mcp, "MCP"),
    (Sec::Keys, "Shortcuts"),
];

#[derive(Clone, Debug, PartialEq)]
pub enum SyncStatus {
    Idle,
    Testing { msg: String },
    Error { msg: String, missing: Vec<String> },
    Ok { msg: String },
}

/// Where to put the caret after opening a note.
#[derive(Clone, Copy, Debug, Default)]
pub struct Sel {
    pub start: usize,
    pub end: usize,
    pub line: Option<usize>,
    pub force_source: bool,
}

pub struct App {
    pub screen: Screen,
    pub persist: Persist,
    pub vault: Option<Vault>,
    pub names: Names,
    pub tabs: Vec<String>,
    pub active: Option<String>,
    pub recent: Vec<String>,
    pub expanded: BTreeSet<String>,
    /// Pinned notes (`pins.rs`), in the order they were pinned.
    pub pinned: Vec<String>,
    /// The Pinned group in the tree is open.
    pub pins_open: bool,
    pub mode: Mode,
    pub left_panel: Panel,
    pub query: TextEdit,
    pub palette: Option<Palette>,
    pub settings_open: bool,
    pub set_sec: Sec,
    pub focus: bool,
    pub vault_menu: bool,
    pub renaming: Option<TextEdit>,
    pub creating: Option<TextEdit>,
    /// The heading at the top of the preview, for the outline's dot.
    pub prev_head: usize,
    /// One editor per open note, so each keeps its caret and undo history.
    pub edits: HashMap<String, TextEdit>,
    /// Single-line fields in settings, by key.
    pub fields: HashMap<String, TextEdit>,
    pub reveal: BTreeSet<String>,
    pub sync_status: SyncStatus,
    pub mcp_copied: bool,
    pub notice: Option<String>,
    /// The update check and the update itself (`update.rs`).
    pub update: crate::update::UpdateState,
    pub writer: Writer,
    pub(crate) dirty: BTreeSet<String>,
    save_at: Option<f64>,
    state_at: Option<f64>,
    /// The app clock in seconds, from `poll` (virtual in headless runs).
    pub(crate) now: std::cell::Cell<f64>,
    /// Note counts for the welcome screen's recent list, read once per visit.
    pub counts: HashMap<PathBuf, usize>,
    /// Set by a view that wants the editor focused once it exists.
    pub want_editor_focus: bool,
    /// The system appearance as the last view saw it, for commands run from keys.
    pub sys_dark: std::cell::Cell<bool>,
    /// The window height as the last view saw it.
    pub view_h: std::cell::Cell<f32>,
    pub mcp_copied_at: Option<Instant>,
    pub mcp_http: Option<crate::mcp::HttpServer>,
    pub mcp_http_error: Option<String>,
    mcp_http_failed_port: Option<u16>,
    /// The editor's caret as the last frame saw it: note, anchor, focus, text length.
    last_caret: Option<(String, usize, usize, usize)>,
    /// A key reached the editor since the last frame.
    pub key_moved: bool,
    pub(crate) waker: Option<rune::Waker>,
    watcher: Option<crate::watch::Watcher>,
    /// Counts name lookups rebuilt, so cached lines redraw when a link's target appears.
    pub names_gen: u64,
    /// Set by the theme watcher when Omarchy switches themes.
    theme_changed: Option<std::sync::Arc<std::sync::atomic::AtomicBool>>,
    pub dragging: Option<crate::folders::DragNote>,
    pub tree_menu: Option<crate::folders::TreeMenu>,
    pub new_folder: Option<crate::folders::NewFolder>,
    pub sync: crate::sync::app::SyncState,
    pub images: crate::images::Images,
    /// Files are dragged over the note.
    pub drop_over: bool,
    /// Locked notes open for reading and editing, by path. The plain text lives only here and
    /// in the note's editor. The folder copy in `vault.files` stays sealed.
    pub unlocked: HashMap<String, Unlocked>,
    /// The "Lock this note" dialog is open, with this error, or an empty string for none.
    pub lock_dialog: Option<String>,
    /// Why the last unlock failed.
    pub unlock_error: Option<String>,
}

/// A locked note while it is open.
pub struct Unlocked {
    pub key: crate::lock::Key,
    pub text: String,
}

pub fn init() -> App {
    let mut persist = store::load();
    let writer = Writer::start(None);
    seed_sample(&mut persist);
    let mut app = App {
        screen: Screen::Welcome,
        persist,
        vault: None,
        names: Names::default(),
        tabs: Vec::new(),
        active: None,
        recent: Vec::new(),
        expanded: BTreeSet::new(),
        pinned: Vec::new(),
        pins_open: true,
        mode: Mode::Source,
        left_panel: Panel::Files,
        query: TextEdit::new(""),
        palette: None,
        settings_open: false,
        set_sec: Sec::Appearance,
        focus: false,
        vault_menu: false,
        renaming: None,
        creating: None,
        prev_head: 0,
        edits: HashMap::new(),
        fields: HashMap::new(),
        reveal: BTreeSet::new(),
        sync_status: SyncStatus::Idle,
        mcp_copied: false,
        notice: None,
        update: Default::default(),
        writer,
        dirty: BTreeSet::new(),
        save_at: None,
        state_at: None,
        now: std::cell::Cell::new(0.0),
        counts: HashMap::new(),
        want_editor_focus: false,
        sys_dark: std::cell::Cell::new(false),
        view_h: std::cell::Cell::new(800.0),
        mcp_copied_at: None,
        mcp_http: None,
        mcp_http_error: None,
        mcp_http_failed_port: None,
        last_caret: None,
        key_moved: false,
        waker: None,
        watcher: None,
        names_gen: 0,
        theme_changed: None,
        dragging: None,
        tree_menu: None,
        new_folder: None,
        sync: Default::default(),
        images: Default::default(),
        drop_over: false,
        unlocked: HashMap::new(),
        lock_dialog: None,
        unlock_error: None,
    };
    crate::theme::reload_omarchy();
    crate::update::clean_up();
    if let Some(from) = app.persist.updated_from.take() {
        if from != crate::update::CURRENT {
            app.notice = Some(format!("Typo is updated to {}.", crate::update::CURRENT));
        }
        app.persist_later();
    }
    // The remote folder used to be one value for every notes folder. Each folder has its own now.
    for c in app.persist.settings.sync.cfg.values_mut() {
        c.remove("prefix");
    }
    app.mode = if app.persist.mode == "preview" { Mode::Preview } else { Mode::Source };
    let target = crate::cli::start_target();
    let note = target.as_ref().and_then(|t| t.1.clone());
    let start = target.map(|t| t.0).or_else(|| app.persist.last.clone());
    if let Some(p) = start {
        if app.load_vault(&p).is_err() {
            app.screen = Screen::Welcome;
        } else if let Some(n) = note.filter(|n| app.files().contains_key(n)) {
            if !app.tabs.contains(&n) {
                app.tabs.push(n.clone());
            }
            app.active = Some(n);
            app.ensure_edit();
        }
    }
    if app.screen == Screen::Welcome {
        app.refresh_counts();
    }
    app
}

/// The sample folder: `~/Documents/Field Notes`, as the design has it. If a folder with that
/// name already exists and is not Typo's sample, the sample goes to Typo's data folder.
pub fn sample_path() -> PathBuf {
    let docs = store::documents_dir().join(crate::sample::NAME);
    let fallback = store::dirs().map(|d| d.data().join(crate::sample::NAME));
    let p = store::load();
    if let Some(known) = p.vaults.iter().find(|v| v.name == crate::sample::NAME && (v.path == docs || Some(&v.path) == fallback.as_ref())) {
        return known.path.clone();
    }
    if !docs.exists() {
        return docs;
    }
    fallback.unwrap_or(docs)
}

fn seed_sample(p: &mut Persist) {
    if p.seeded {
        return;
    }
    let path = sample_path();
    if !path.exists() && vault::write_sample(&path).is_err() {
        return;
    }
    p.seeded = true;
    if !p.vaults.iter().any(|v| v.path == path) {
        p.vaults.push(VaultRef { name: crate::sample::NAME.into(), path: path.clone() });
    }
    let tabs: Vec<String> = crate::sample::TABS.iter().map(|s| s.to_string()).collect();
    p.sessions.insert(
        path.to_string_lossy().into_owned(),
        Session { tabs: tabs.clone(), active: Some(tabs[0].clone()), recent: tabs, expanded: vec!["Drafts".into(), "Journal".into()], ..Default::default() },
    );
    p.last = Some(path);
    let _ = store::save(p);
}

impl App {
    pub fn settings(&self) -> &Settings {
        &self.persist.settings
    }

    pub fn files(&self) -> &BTreeMap<String, String> {
        static EMPTY: BTreeMap<String, String> = BTreeMap::new();
        self.vault.as_ref().map_or(&EMPTY, |v| &v.files)
    }

    /// The active note's text. An unlocked note gives its plain text, and a locked note none.
    pub fn active_text(&self) -> &str {
        let Some(a) = &self.active else { return "" };
        if let Some(u) = self.unlocked.get(a) {
            return &u.text;
        }
        self.files().get(a).map_or("", |t| if crate::lock::is_locked(t) { "" } else { t.as_str() })
    }

    /// The note at `p` is locked and not open: its text needs the password.
    pub fn note_locked(&self, p: &str) -> bool {
        !self.unlocked.contains_key(p) && self.files().get(p).is_some_and(|t| crate::lock::is_locked(t))
    }

    /// The note at `p` is locked and open now.
    pub fn note_unlocked(&self, p: &str) -> bool {
        self.unlocked.contains_key(p)
    }

    pub fn is_dark(&self, system_dark: bool) -> bool {
        self.palette(system_dark).dark
    }

    /// Light or dark mode now. On Omarchy, "Match system" follows the Omarchy theme's mode.
    pub fn dark_mode(&self, system_dark: bool) -> bool {
        match self.settings().theme.as_str() {
            "dark" => true,
            "system" => crate::theme::omarchy().map_or(system_dark, |o| o.dark),
            _ => false,
        }
    }

    /// The theme picked for dark or light mode. Without a pick it is Paper or Ink, or the
    /// Omarchy theme under "Match system" when its mode matches.
    pub fn theme_id(&self, dark: bool) -> String {
        let set = self.settings();
        if let Some(id) = if dark { &set.dark_theme } else { &set.light_theme } {
            return id.clone();
        }
        if set.theme == "system" && crate::theme::omarchy().is_some_and(|o| o.dark == dark) {
            "omarchy".into()
        } else if dark {
            "ink".into()
        } else {
            "paper".into()
        }
    }

    /// The colours to draw with: the theme of the current mode, with the accent from the settings.
    pub fn palette(&self, system_dark: bool) -> &'static crate::theme::Pal {
        let dark = self.dark_mode(system_dark);
        crate::theme::themed(&self.theme_id(dark), dark, &self.settings().accent)
    }

    pub(crate) fn rebuild_names_pub(&mut self) {
        self.rebuild_names();
    }

    pub(crate) fn mark_dirty_pub(&mut self, rel: &str) {
        self.mark_dirty(rel);
    }

    fn rebuild_names(&mut self) {
        self.names = Names::build(self.files());
        self.names_gen += 1;
    }

    pub fn folder_list(&self) -> Vec<String> {
        let Some(v) = &self.vault else { return Vec::new() };
        let mut set: BTreeSet<String> = v.folders.iter().cloned().collect();
        for p in v.files.keys() {
            let d = md::dir(p);
            if !d.is_empty() {
                set.insert(d.to_string());
            }
        }
        set.into_iter().collect()
    }

    // ---------- Persistence ----------

    pub fn persist_later(&mut self) {
        self.state_at = Some(self.now.get() + 0.4);
    }

    fn session(&self) -> Session {
        Session {
            tabs: self.tabs.clone(),
            active: self.active.clone(),
            recent: self.recent.iter().take(30).cloned().collect(),
            expanded: self.expanded.iter().cloned().collect(),
            pinned: self.pinned.clone(),
            pins_closed: !self.pins_open,
        }
    }

    fn store_session(&mut self) {
        if let Some(v) = &self.vault {
            let key = v.path.to_string_lossy().into_owned();
            let s = self.session();
            self.persist.sessions.insert(key, s);
        }
        self.persist.mode = if self.mode == Mode::Preview { "preview" } else { "source" }.into();
    }

    /// Runs before each frame: writes due notes and settings, and picks up writer errors.
    pub fn poll(&mut self, t: f64, tx: &mut Tx) -> bool {
        self.now.set(t);
        let now = Instant::now();
        let mut changed = false;
        if self.save_at.is_some_and(|due| t >= due) {
            self.flush_notes();
        } else if let Some(due) = self.save_at {
            tx.wake_after(Duration::from_secs_f64(due - t));
        }
        if self.state_at.is_some_and(|due| t >= due) {
            self.flush_state();
        } else if let Some(due) = self.state_at {
            tx.wake_after(Duration::from_secs_f64(due - t));
        }
        if self.mcp_copied_at.is_some_and(|t| now >= t + Duration::from_millis(1600)) {
            self.mcp_copied_at = None;
            self.mcp_copied = false;
            changed = true;
        }
        changed |= self.sync_mcp_http();
        changed |= self.check_link_click(tx);
        changed |= self.apply_outside_changes();
        changed |= self.poll_sync(t, tx);
        changed |= self.poll_update(t, tx);
        changed |= self.images.poll();
        if self.theme_changed.as_ref().is_some_and(|f| f.swap(false, std::sync::atomic::Ordering::SeqCst)) {
            changed |= crate::theme::reload_omarchy();
        }
        if let Ok(mut e) = self.writer.errors.lock() {
            if let Some(m) = e.pop() {
                self.notice = Some(m);
                e.clear();
                changed = true;
            }
        }
        changed
    }

    pub fn set_waker(&mut self, w: rune::Waker) {
        self.images.start(w.clone());
        self.theme_changed = Some(crate::watch::watch_theme(w.clone()));
        self.waker = Some(w);
        self.watch_vault();
    }

    fn watch_vault(&mut self) {
        self.watcher = self.vault.as_ref().map(|v| crate::watch::start(v.path.clone(), self.waker.clone()));
    }

    /// Takes changes other apps made to the folder. A note with edits not yet saved keeps
    /// the edits; they overwrite the outside change when they save.
    fn apply_outside_changes(&mut self) -> bool {
        let Some(w) = &self.watcher else { return false };
        let c = match w.inbox.lock() {
            Ok(mut b) => std::mem::take(&mut *b),
            Err(_) => return false,
        };
        if c.changed.is_empty() && c.removed.is_empty() {
            return false;
        }
        let mut any = false;
        for (rel, text) in c.changed {
            if self.dirty.contains(&rel) || self.files().get(&rel) == Some(&text) {
                continue;
            }
            if self.unlocked.contains_key(&rel) {
                self.outside_locked_text(&rel, text);
                any = true;
                continue;
            }
            let new = !self.files().contains_key(&rel);
            if let Some(e) = self.edits.get_mut(&rel) {
                let (a, b) = (e.anchor.min(text.len()), e.focus.min(text.len()));
                e.replace_text(text.clone());
                e.set_selection(a, b);
            }
            if let Some(v) = &mut self.vault {
                if new {
                    v.born.insert(rel.clone(), now_ns());
                }
                v.files.insert(rel, text);
            }
            any = true;
        }
        for rel in c.removed {
            if self.dirty.contains(&rel) {
                continue;
            }
            if let Some(v) = &mut self.vault {
                if v.files.remove(&rel).is_some() {
                    any = true;
                }
            }
            if self.tabs.contains(&rel) {
                self.close_tab(&rel);
            }
            self.recent.retain(|x| x != &rel);
            self.pinned.retain(|x| x != &rel);
        }
        if any {
            self.rebuild_names();
        }
        any
    }

    /// A text field gives no click event, so a click shows as a caret that moved with no key
    /// and no text change. With Ctrl (⌘ on macOS) held, that click follows the link under it.
    fn check_link_click(&mut self, tx: &mut Tx) -> bool {
        let Some(a) = self.active.clone() else {
            self.last_caret = None;
            return false;
        };
        let Some(e) = self.edits.get(&a) else { return false };
        let cur = (a.clone(), e.anchor, e.focus, e.text.len());
        let clicked = self.last_caret.as_ref().is_some_and(|l| l.0 == cur.0 && l.3 == cur.3 && (l.1, l.2) != (cur.1, cur.2))
            && !self.key_moved
            && cur.1 == cur.2
            && e.focused
            && self.mode == Mode::Source
            && tx.mods().cmd;
        self.last_caret = Some(cur);
        self.key_moved = false;
        if clicked {
            crate::ui::follow_link_at_caret(self, tx);
            return true;
        }
        false
    }

    /// Starts, updates or stops the HTTP MCP server to match the settings.
    fn sync_mcp_http(&mut self) -> bool {
        let m = &self.persist.settings.mcp;
        let want = m.enabled && m.transport == "http" && self.vault.is_some();
        let port: Option<u16> = m.port.parse().ok().filter(|p| *p > 0);
        if !want || port.is_none() {
            let had = self.mcp_http.take().is_some() || self.mcp_http_error.is_some();
            if want && port.is_none() {
                let msg = Some("not running · the port must be a number from 1 to 65535".to_string());
                let changed = self.mcp_http_error != msg;
                self.mcp_http_error = msg;
                return changed;
            }
            self.mcp_http_error = None;
            return had;
        }
        let port = port.unwrap_or(3917);
        let cfg = crate::mcp::HttpCfg {
            root: self.vault.as_ref().map(|v| v.path.clone()).unwrap_or_default(),
            mcp: m.clone(),
            update_links: self.persist.settings.update_links,
        };
        if let Some(srv) = &self.mcp_http {
            if srv.port == port {
                if let Ok(mut c) = srv.cfg.lock() {
                    if *c != cfg {
                        *c = cfg;
                    }
                }
                return false;
            }
            self.mcp_http = None;
        }
        if self.mcp_http_error.is_some() && self.mcp_http_failed_port == Some(port) {
            return false;
        }
        match crate::mcp::serve_http(port, cfg) {
            Ok(srv) => {
                self.mcp_http = Some(srv);
                self.mcp_http_error = None;
                self.mcp_http_failed_port = None;
            }
            Err(e) => {
                self.mcp_http_error = Some(format!("not running · port {port}: {e}"));
                self.mcp_http_failed_port = Some(port);
            }
        }
        true
    }

    pub fn flush_notes(&mut self) {
        self.save_at = None;
        let Some(v) = &self.vault else { return };
        let wrote = !self.dirty.is_empty();
        for rel in std::mem::take(&mut self.dirty) {
            if let Some(text) = v.files.get(&rel) {
                self.writer.send(Op::Write { root: v.path.clone(), rel, text: text.clone() });
            }
        }
        if wrote {
            self.sync_after_save();
        }
    }

    pub fn flush_state(&mut self) {
        self.state_at = None;
        self.store_session();
        self.writer.send(Op::SaveState(Box::new(self.persist.clone())));
    }

    /// Saves everything now, for quitting: queued disk work finishes, then unsaved notes are
    /// written on this thread, so nothing is lost when the process ends.
    pub fn flush_all(&mut self) {
        self.save_at = None;
        self.writer.wait(Duration::from_secs(5));
        if let Some(v) = &self.vault {
            for rel in std::mem::take(&mut self.dirty) {
                if let Some(text) = v.files.get(&rel) {
                    if let Err(e) = vault::write_note(&v.path, &rel, text) {
                        eprintln!("typo: could not save {rel}: {e}");
                    }
                }
            }
        }
        self.store_session();
        if let Some(v) = &self.vault {
            self.persist.last = Some(v.path.clone());
        }
        let _ = store::save(&self.persist);
    }

    fn mark_dirty(&mut self, rel: &str) {
        self.dirty.insert(rel.to_string());
        if self.save_at.is_none() {
            self.save_at = Some(self.now.get() + 0.5);
        }
    }

    /// Changes a note's text from outside the editor (a task box, a link rewrite).
    pub fn set_text(&mut self, rel: &str, text: String) {
        if self.unlocked.contains_key(rel) {
            if let Some(e) = self.edits.get_mut(rel)
                && e.text != text
            {
                let (a, b) = (e.anchor.min(text.len()), e.focus.min(text.len()));
                e.replace_text(text.clone());
                e.set_selection(a, b);
            }
            self.store_unlocked(rel, text);
            return;
        }
        if let Some(e) = self.edits.get_mut(rel) {
            if e.text != text {
                let (a, b) = (e.anchor.min(text.len()), e.focus.min(text.len()));
                e.replace_text(text.clone());
                e.set_selection(a, b);
            }
        }
        if let Some(v) = &mut self.vault {
            v.files.insert(rel.to_string(), text);
        }
        self.mark_dirty(rel);
    }

    // ---------- Folders ----------

    pub fn refresh_counts(&mut self) {
        self.counts = self.persist.vaults.iter().map(|v| (v.path.clone(), vault::count_notes(&v.path))).collect();
    }

    pub fn load_vault(&mut self, path: &Path) -> std::io::Result<()> {
        let v = vault::load(path)?;
        self.flush_notes();
        self.store_session();
        let key = path.to_string_lossy().into_owned();
        let ses = self.persist.sessions.get(&key).cloned().unwrap_or_default();
        self.tabs = ses.tabs.into_iter().filter(|p| v.files.contains_key(p)).collect();
        self.active = ses.active.filter(|a| v.files.contains_key(a)).or_else(|| self.tabs.first().cloned());
        self.recent = ses.recent.into_iter().filter(|p| v.files.contains_key(p)).collect();
        if self.recent.is_empty() {
            self.recent = self.tabs.clone();
        }
        self.expanded = ses.expanded.into_iter().collect();
        self.pinned = ses.pinned.into_iter().filter(|p| v.files.contains_key(p)).collect();
        self.pins_open = !ses.pins_closed;
        if !self.persist.vaults.iter().any(|r| r.path == path) {
            self.persist.vaults.push(VaultRef { name: v.name.clone(), path: path.to_path_buf() });
        }
        self.persist.last = Some(path.to_path_buf());
        self.vault = Some(v);
        self.edits.clear();
        self.rebuild_names();
        if self.waker.is_some() {
            self.watch_vault();
        }
        self.screen = Screen::Vault;
        self.creating = None;
        self.query = TextEdit::new("");
        self.left_panel = Panel::Files;
        self.focus = false;
        self.palette = None;
        self.settings_open = false;
        self.vault_menu = false;
        self.renaming = None;
        self.ensure_edit();
        if self.mode == Mode::Source {
            if let Some(e) = self.cur_edit_opt() {
                e.focused = true;
            }
        }
        self.want_editor_focus = self.mode == Mode::Source;
        self.persist_later();
        self.sync_auto(0.5);
        Ok(())
    }

    pub fn open_vault(&mut self, path: PathBuf, tx: &mut Tx) {
        match self.load_vault(&path) {
            Ok(()) => {
                if let Some(v) = &self.vault {
                    tx.set_title(format!("{} · Typo", v.name));
                }
                self.focus_editor(tx);
            }
            Err(e) => self.notice = Some(format!("Could not open {}: {e}", store::tilde(&path))),
        }
    }

    pub fn open_folder_dialog(&mut self, tx: &mut Tx) {
        let options = FileDialogOptions { title: "Open a folder of notes".into(), directory: Some(store::documents_dir()), ..Default::default() };
        tx.open_folder(options, |s: &mut App, result, tx| match result {
            Ok(Some(path)) => s.open_vault(path, tx),
            Ok(None) => {}
            Err(e) => s.notice = Some(format!("Could not show the folder picker: {e}")),
        });
    }

    pub fn go_welcome(&mut self, tx: &mut Tx) {
        self.flush_notes();
        self.store_session();
        self.screen = Screen::Welcome;
        self.palette = None;
        self.settings_open = false;
        self.vault_menu = false;
        self.focus = false;
        self.renaming = None;
        self.persist.last = None;
        self.watcher = None;
        self.refresh_counts();
        self.persist_later();
        tx.set_title("Typo");
    }

    pub fn new_vault_path(&self) -> PathBuf {
        let name = self.creating.as_ref().map_or("", |e| e.text.trim());
        let name = md::clean_name(if name.is_empty() { "Untitled notebook" } else { name });
        store::documents_dir().join(name)
    }

    pub fn create_vault(&mut self, tx: &mut Tx) {
        let path = self.new_vault_path();
        if let Err(e) = std::fs::create_dir_all(&path) {
            self.notice = Some(format!("Could not create {}: {e}", store::tilde(&path)));
            return;
        }
        self.open_vault(path, tx);
    }

    pub fn forget_vault(&mut self, path: &Path) {
        self.persist.vaults.retain(|v| v.path != path);
        self.persist_later();
    }

    pub fn reset_sample(&mut self, tx: &mut Tx) {
        let path = sample_path();
        if let Err(e) = vault::write_sample(&path) {
            self.notice = Some(format!("Could not restore the sample notes: {e}"));
            return;
        }
        let tabs: Vec<String> = crate::sample::TABS.iter().map(|s| s.to_string()).collect();
        self.persist.sessions.insert(
            path.to_string_lossy().into_owned(),
            Session { tabs: tabs.clone(), active: Some(tabs[0].clone()), recent: tabs, expanded: vec!["Drafts".into(), "Journal".into()], ..Default::default() },
        );
        // Load without storing the current session over the one just written.
        self.vault = None;
        self.open_vault(path, tx);
        if let Some(e) = self.cur_edit_opt() {
            e.set_selection(0, 0);
        }
    }

    // ---------- Notes ----------

    pub fn ensure_edit(&mut self) {
        if let Some(a) = self.active.clone() {
            let text = match self.unlocked.get(&a) {
                Some(u) => u.text.clone(),
                None => self.files().get(&a).filter(|t| !crate::lock::is_locked(t)).cloned().unwrap_or_default(),
            };
            self.edits.entry(a).or_insert_with(|| {
                let mut e = TextEdit::new(text);
                e.set_selection(0, 0);
                e
            });
        }
    }

    pub fn cur_edit_opt(&mut self) -> Option<&mut TextEdit> {
        let a = self.active.clone()?;
        self.edits.get_mut(&a)
    }

    /// The active note's editor. The view only builds the editor while a note is active.
    pub fn cur_edit(&mut self) -> &mut TextEdit {
        self.ensure_edit();
        let key = self.active.clone().unwrap_or_default();
        self.edits.entry(key).or_insert_with(|| TextEdit::new(""))
    }

    /// The editor's text changed: keep the folder copy in step and save soon.
    pub fn after_edit(&mut self) {
        let Some(a) = self.active.clone() else { return };
        let Some(text) = self.edits.get(&a).map(|e| e.text.clone()) else { return };
        if self.unlocked.contains_key(&a) {
            self.store_unlocked(&a, text);
            return;
        }
        if self.note_locked(&a) {
            return;
        }
        if let Some(v) = &mut self.vault {
            if v.files.get(&a) != Some(&text) {
                v.files.insert(a.clone(), text);
                self.mark_dirty(&a);
            }
        }
    }

    pub fn focus_editor(&mut self, tx: &mut Tx) {
        if self.active.is_some() && self.mode == Mode::Source {
            tx.focus("editor", false);
            self.want_editor_focus = true;
        }
    }

    pub fn open_file(&mut self, path: &str, sel: Option<Sel>, tx: &mut Tx) {
        if self.files().get(path).is_none() {
            return;
        }
        if !self.tabs.iter().any(|t| t == path) {
            self.tabs.push(path.to_string());
        }
        let changed = self.active.as_deref() != Some(path);
        self.active = Some(path.to_string());
        let mut d = md::dir(path).to_string();
        while !d.is_empty() {
            self.expanded.insert(d.clone());
            d = md::dir(&d).to_string();
        }
        self.recent.retain(|x| x != path);
        self.recent.insert(0, path.to_string());
        self.recent.truncate(30);
        self.palette = None;
        self.renaming = None;
        self.vault_menu = false;
        self.prev_head = 0;
        if sel.is_some_and(|s| s.force_source) {
            self.mode = Mode::Source;
        }
        self.ensure_edit();
        if let Some(s) = sel {
            if let Some(e) = self.cur_edit_opt() {
                let n = e.text.len();
                e.set_selection(s.start.min(n), s.end.min(n));
                e.break_history_group();
            }
            if let Some(line) = s.line {
                tx.scroll_key_to(format!("ln-{line}"), 120.0);
            }
        } else if changed {
            tx.scroll_to_instant("doc-scroll", 0.0, 0.0);
        }
        self.focus_editor(tx);
        self.persist_later();
    }

    pub fn close_tab(&mut self, p: &str) {
        let Some(i) = self.tabs.iter().position(|x| x == p) else { return };
        self.tabs.remove(i);
        self.edits.remove(p);
        // A closed note locks again.
        self.unlocked.remove(p);
        if self.active.as_deref() == Some(p) {
            self.active = self.tabs.get(i.min(self.tabs.len().saturating_sub(1))).cloned();
            if self.active.is_none() {
                self.focus = false;
            }
        }
        self.renaming = None;
        self.persist_later();
    }

    pub fn toggle_folder(&mut self, p: &str) {
        if !self.expanded.remove(p) {
            self.expanded.insert(p.to_string());
        }
        self.persist_later();
    }

    pub(crate) fn unique_path(&self, folder: &str, base: &str) -> String {
        let files = self.files();
        let mut p = md::join(folder, &format!("{base}.md"));
        let mut n = 2;
        while files.contains_key(&p) || self.vault.as_ref().is_some_and(|v| v.path.join(&p).exists()) {
            p = md::join(folder, &format!("{base} {n}.md"));
            n += 1;
        }
        p
    }

    pub fn new_note(&mut self, name: Option<&str>, tx: &mut Tx) {
        self.new_note_in(None, name, tx);
    }

    pub fn delete_note(&mut self) {
        let Some(p) = self.active.clone() else { return };
        self.unlocked.remove(&p);
        let Some(v) = &mut self.vault else { return };
        v.files.remove(&p);
        let root = v.path.clone();
        self.dirty.remove(&p);
        self.writer.send(Op::Delete { root, rel: p.clone() });
        self.recent.retain(|x| x != &p);
        self.pinned.retain(|x| x != &p);
        self.close_tab(&p);
        self.focus = false;
        self.rebuild_names();
        self.sync_after_save();
    }

    pub fn open_today(&mut self, tx: &mut Tx) {
        let ms = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis() as i64);
        let now = rune::LocalTime::at(ms);
        let p = format!("Journal/{:04}-{:02}-{:02}.md", now.year, now.month, now.day);
        let existing = self.files().get(&p).cloned();
        let text = match existing {
            Some(t) => t,
            None => {
                let c = format!("# {}\n\n#journal\n\n", long_date(now.year, now.month, now.day));
                if let Some(v) = &mut self.vault {
                    v.files.insert(p.clone(), c.clone());
                    v.born.insert(p.clone(), now_ns());
                    if !v.folders.iter().any(|f| f == "Journal") {
                        v.folders.push("Journal".into());
                        v.folders.sort();
                    }
                }
                self.rebuild_names();
                self.mark_dirty(&p);
                c
            }
        };
        let end = text.len();
        self.open_file(&p, Some(Sel { start: end, end, line: None, force_source: true }), tx);
    }

    pub fn open_link(&mut self, name: &str, tx: &mut Tx) {
        let t = md::link_target(name).to_string();
        if t.is_empty() {
            return;
        }
        match self.names.resolve(&t).map(str::to_string) {
            Some(p) => self.open_file(&p, None, tx),
            None => self.new_note(Some(&t), tx),
        }
    }

    pub fn toggle_task(&mut self, line: usize) {
        let Some(a) = self.active.clone() else { return };
        let t = md::toggle_task(self.active_text(), line);
        self.set_text(&a, t);
    }

    pub fn start_rename(&mut self, tx: &mut Tx) {
        let Some(a) = &self.active else { return };
        let mut e = TextEdit::new(md::base(a));
        let n = e.text.len();
        e.set_selection(0, n);
        e.focused = true;
        self.renaming = Some(e);
        tx.focus("rename", false);
    }

    pub fn cancel_rename(&mut self, tx: &mut Tx) {
        self.renaming = None;
        self.focus_editor(tx);
    }

    pub fn commit_rename(&mut self, tx: &mut Tx) {
        let Some(e) = self.renaming.take() else { return };
        let Some(old) = self.active.clone() else { return };
        let val = md::clean_name(&e.text);
        let np = md::join(md::dir(&old), &format!("{val}.md"));
        let exists = self.files().contains_key(&np);
        if val.is_empty() || np == old || exists {
            self.focus_editor(tx);
            return;
        }
        // Pending writes go first, so the file to rename exists on disk.
        self.flush_notes();
        let old_base = md::base(&old).to_string();
        let update = self.settings().update_links;
        let Some(v) = &mut self.vault else { return };
        let root = v.path.clone();
        let text = v.files.remove(&old).unwrap_or_default();
        v.files.insert(np.clone(), text);
        if let Some(b) = v.born.remove(&old) {
            v.born.insert(np.clone(), b);
        }
        let mut rewritten = Vec::new();
        if update {
            for (p, t) in v.files.iter_mut() {
                let nt = md::rewrite_links(t, &old_base, &val);
                if nt != *t {
                    *t = nt;
                    rewritten.push(p.clone());
                }
            }
        }
        // The rename goes to disk first, then any rewritten notes, the renamed one included.
        self.writer.send(Op::Rename { root, from: old.clone(), to: np.clone() });
        if let Some(u) = self.unlocked.remove(&old) {
            self.unlocked.insert(np.clone(), u);
        }
        if let Some(ed) = self.edits.remove(&old) {
            self.edits.insert(np.clone(), ed);
        }
        for p in rewritten {
            let t = self.files().get(&p).cloned().unwrap_or_default();
            self.set_text(&p, t);
        }
        for t in self.tabs.iter_mut().chain(self.recent.iter_mut()).chain(self.pinned.iter_mut()) {
            if *t == old {
                *t = np.clone();
            }
        }
        self.active = Some(np);
        self.rebuild_names();
        self.focus_editor(tx);
        self.persist_later();
        self.sync_after_save();
    }

    // ---------- Modes and panels ----------

    pub fn toggle_mode(&mut self, tx: &mut Tx) {
        if self.active.is_none() {
            return;
        }
        self.mode = if self.mode == Mode::Source { Mode::Preview } else { Mode::Source };
        if self.mode == Mode::Source {
            self.focus_editor(tx);
        }
        self.persist_later();
    }


    pub fn toggle_focus(&mut self, tx: &mut Tx) {
        if self.active.is_none() {
            return;
        }
        self.focus = !self.focus;
        self.palette = None;
        self.focus_editor(tx);
    }

    pub fn toggle_left(&mut self) {
        self.persist.left_open = !self.persist.left_open;
        self.persist_later();
    }

    pub fn toggle_right(&mut self) {
        self.persist.right_open = !self.persist.right_open;
        self.persist_later();
    }

    pub fn toggle_settings(&mut self, _tx: &mut Tx) {
        self.settings_open = !self.settings_open;
        self.palette = None;
    }

    /// A toolbar action on the active note's selection, as one undo step.
    pub fn format(&mut self, f: crate::format::Fmt, tx: &mut Tx) {
        if self.active.is_none() || self.mode != Mode::Source || self.active.as_deref().is_some_and(|a| self.note_locked(a)) {
            return;
        }
        let e = self.cur_edit();
        let ed = crate::format::apply(&e.text, e.anchor, e.focus, f);
        e.undo_group(|e| {
            e.set_selection(ed.start, ed.end);
            e.insert(&ed.insert);
            e.set_selection(ed.sel.0, ed.sel.1);
        });
        self.after_edit();
        self.focus_editor(tx);
    }

    /// Saves everything, then closes the window.
    pub fn quit(&mut self, tx: &mut Tx) {
        self.flush_all();
        tx.close_window();
    }

    pub fn open_settings(&mut self) {
        self.settings_open = true;
        self.vault_menu = false;
        self.palette = None;
    }

    pub fn open_search(&mut self, tx: &mut Tx) {
        self.persist.left_open = true;
        self.left_panel = Panel::Search;
        self.focus = false;
        self.palette = None;
        self.settings_open = false;
        let n = self.query.text.len();
        self.query.set_selection(0, n);
        tx.focus("search", false);
    }

    pub fn search_tag(&mut self, tag: &str) {
        self.persist.left_open = true;
        self.left_panel = Panel::Search;
        self.query = TextEdit::new(format!("#{tag}"));
        let n = self.query.text.len();
        self.query.set_selection(n, n);
        self.focus = false;
        self.palette = None;
    }

    pub fn open_palette(&mut self, mode: PalMode, tx: &mut Tx) {
        self.palette = Some(Palette { mode, q: TextEdit::new(""), idx: 0 });
        self.vault_menu = false;
        self.settings_open = false;
        tx.focus("pal-input", false);
    }

    pub fn close_palette(&mut self, tx: &mut Tx) {
        self.palette = None;
        self.focus_editor(tx);
    }

    /// Escape closes the topmost thing, as in the design.
    pub fn escape(&mut self, tx: &mut Tx) -> bool {
        if self.palette.is_some() {
            self.close_palette(tx);
        } else if self.settings_open {
            self.settings_open = false;
        } else if self.vault_menu {
            self.vault_menu = false;
        } else if self.tree_menu.is_some() {
            self.tree_menu = None;
        } else if self.new_folder.is_some() {
            self.new_folder = None;
        } else if self.renaming.is_some() {
            self.cancel_rename(tx);
        } else if self.creating.is_some() {
            self.creating = None;
        } else if self.focus {
            self.focus = false;
        } else {
            return false;
        }
        true
    }

    /// Moves the caret, or the preview, to line `line`.
    pub fn jump(&mut self, line: usize, tx: &mut Tx) {
        if self.mode == Mode::Source {
            let pos = md::line_start(self.active_text(), line);
            if let Some(e) = self.cur_edit_opt() {
                e.set_selection(pos, pos);
                e.break_history_group();
            }
            tx.scroll_key_to(format!("ln-{line}"), 120.0);
            self.focus_editor(tx);
        } else {
            tx.scroll_key_to(format!("pv-{line}"), 32.0);
        }
    }

    pub fn set_theme(&mut self, theme: &str) {
        self.persist.settings.theme = theme.into();
        self.persist_later();
    }

    pub fn settings_mut(&mut self) -> &mut Settings {
        self.persist_later();
        &mut self.persist.settings
    }

    /// A settings text field, made on first use with `init`.
    pub fn field(&mut self, key: &str, init: impl FnOnce() -> String) -> &mut TextEdit {
        self.fields.entry(key.to_string()).or_insert_with(|| TextEdit::new(init()))
    }
}

pub(crate) fn now_ns() -> u128 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos())
}

/// "Thursday 12 November", as `toLocaleDateString('en-GB')` writes it.
fn long_date(year: i32, month: u32, day: u32) -> String {
    const DAYS: [&str; 7] = ["Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday"];
    const MONTHS: [&str; 12] = ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];
    // Sakamoto's method: 0 is Sunday.
    const T: [i32; 12] = [0, 3, 2, 5, 0, 3, 5, 1, 4, 6, 2, 4];
    let y = if month < 3 { year - 1 } else { year };
    let w = (y + y / 4 - y / 100 + y / 400 + T[(month as usize + 11) % 12] + day as i32).rem_euclid(7);
    format!("{} {} {}", DAYS[w as usize], day, MONTHS[(month as usize + 11) % 12])
}
