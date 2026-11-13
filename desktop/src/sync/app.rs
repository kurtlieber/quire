//! Sync in the app: when it runs, the job thread, and what its outcome changes.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use rune::Tx;

use super::{Outcome, Remote};
use crate::app::{App, SyncStatus};
use crate::vault::Op;

/// Work on another thread whose answer the app picks up in `poll`.
pub struct Job<T> {
    slot: Arc<Mutex<Option<Result<T, String>>>>,
}

impl<T: Send + 'static> Job<T> {
    pub(crate) fn spawn(waker: Option<rune::Waker>, f: impl FnOnce() -> Result<T, String> + Send + 'static) -> Job<T> {
        let slot: Arc<Mutex<Option<Result<T, String>>>> = Arc::default();
        let s2 = slot.clone();
        let _ = std::thread::Builder::new().name("quire-sync".into()).spawn(move || {
            let r = f();
            if let Ok(mut g) = s2.lock() {
                *g = Some(r);
            }
            if let Some(w) = waker {
                w();
            }
        });
        Job { slot }
    }

    pub(crate) fn take(&self) -> Option<Result<T, String>> {
        self.slot.lock().ok().and_then(|mut g| g.take())
    }
}

#[derive(Default)]
pub struct SyncState {
    job: Option<Job<Outcome>>,
    test: Option<Job<String>>,
    /// Another sync was asked for while one ran.
    again: bool,
    /// When the next sync is due, on the app clock.
    at: Option<f64>,
    /// The person pressed "Sync now", so the settings show how it went.
    manual: bool,
    /// The last sync failed, with this message.
    pub error: Option<String>,
    /// The pins the running sync started with. The app applies only what the sync changed.
    pins_sent: Option<std::collections::BTreeSet<String>>,
    /// When the last sync started, on the app clock. "Every 5 min" counts from it, so a sync
    /// that fails does not start again at once.
    last_try: Option<f64>,
    last_tick: f64,
}

impl SyncState {
    pub fn busy(&self) -> bool {
        self.job.is_some()
    }
    pub fn testing(&self) -> bool {
        self.test.is_some()
    }
}

pub fn ago(ms: i64) -> String {
    let d = (super::sign::now_secs() * 1000 - ms).max(0) as f64 / 1000.0;
    if d < 45.0 {
        "just now".into()
    } else if d < 3600.0 {
        format!("{}m ago", (d / 60.0).round() as i64)
    } else if d < 86_400.0 {
        format!("{}h ago", (d / 3600.0).round() as i64)
    } else {
        format!("{}d ago", (d / 86_400.0).round() as i64)
    }
}

impl App {
    /// The remote for the open folder, from the settings.
    pub fn remote(&self) -> Option<Remote> {
        let sy = &self.settings().sync;
        if sy.provider == "none" {
            return None;
        }
        let cfg = sy.cfg.get(&sy.provider).cloned().unwrap_or_default();
        let prefix = self.remote_folder();
        Some(Remote { provider: sy.provider.clone(), cfg, prefix: super::clean_prefix(&prefix), encrypt: sy.encrypt, pass: sy.pass.clone() })
    }

    /// The remote folder of the open notes folder, as the settings show it.
    pub fn remote_folder(&self) -> String {
        let Some(v) = &self.vault else { return "quire/notes/".into() };
        let key = v.path.to_string_lossy().into_owned();
        self.settings().sync.folders.get(&key).filter(|p| !p.trim().is_empty()).cloned().unwrap_or_else(|| format!("quire/{}/", v.name))
    }

    /// Sync is set up and its test passed.
    pub fn sync_ready(&self) -> bool {
        let sy = &self.settings().sync;
        sy.provider != "none" && sy.connected && self.vault.is_some()
    }

    /// The settings changed, so the connection must be tested again.
    pub fn sync_changed(&mut self) {
        if self.persist.settings.sync.connected {
            self.persist.settings.sync.connected = false;
            self.persist_later();
        }
    }

    pub fn sync_soon(&mut self, delay: f64) {
        if self.sync_ready() {
            let due = self.now.get() + delay;
            self.sync.at = Some(self.sync.at.map_or(due, |a| a.min(due)));
        }
    }

    /// A sync the app starts by itself, when a folder opens or a test passes. "Manually" skips it.
    pub fn sync_auto(&mut self, delay: f64) {
        if self.settings().sync.when != "manual" {
            self.sync_soon(delay);
        }
    }

    /// Called after notes are saved: "On save" syncs a moment after typing stops.
    pub fn sync_after_save(&mut self) {
        if self.settings().sync.when == "save" {
            self.sync_soon(1.5);
        }
    }

    pub fn sync_now(&mut self) {
        self.sync.manual = true;
        self.sync_status = SyncStatus::Testing { msg: "Syncing…".into() };
        self.start_sync();
    }

    fn start_sync(&mut self) {
        self.sync.at = None;
        self.sync.last_try = Some(self.now.get());
        if self.sync.job.is_some() {
            self.sync.again = true;
            return;
        }
        let (Some(r), Some(root)) = (self.remote(), self.vault.as_ref().map(|v| v.path.clone())) else { return };
        // Saved notes go to disk first, and the sync starts once the writer has caught up.
        self.flush_notes_quiet();
        let done = self.writer.barrier();
        let device = super::device_name();
        let pins: std::collections::BTreeSet<String> = self.pinned.iter().cloned().collect();
        self.sync.pins_sent = Some(pins.clone());
        self.sync.job = Some(Job::spawn(self.waker.clone(), move || {
            let _ = done.recv_timeout(Duration::from_secs(10));
            super::run(&r, &root, &device, &pins)
        }));
    }

    /// Writes unsaved notes without asking for another sync.
    fn flush_notes_quiet(&mut self) {
        let Some(v) = &self.vault else { return };
        for rel in std::mem::take(&mut self.dirty) {
            if let Some(text) = v.files.get(&rel) {
                self.writer.send(Op::Write { root: v.path.clone(), rel, text: text.clone() });
            }
        }
    }

    pub fn start_test(&mut self) {
        let Some(r) = self.remote() else { return };
        let notes = self.files().len();
        let before = self.settings().sync.last_sync.is_some();
        self.sync_status = SyncStatus::Testing { msg: "Connecting…".into() };
        self.sync.test = Some(Job::spawn(self.waker.clone(), move || super::test(&r, notes, before)));
    }

    /// Picks up finished jobs and starts due syncs. True when something on screen changed.
    pub fn poll_sync(&mut self, t: f64, tx: &mut Tx) -> bool {
        let mut changed = false;
        if let Some(res) = self.sync.test.as_ref().and_then(|j| j.take()) {
            self.sync.test = None;
            changed = true;
            match res {
                Ok(msg) => {
                    self.persist.settings.sync.connected = true;
                    self.sync_status = SyncStatus::Ok { msg };
                    self.persist_later();
                    self.sync_auto(0.2);
                }
                Err(msg) => {
                    self.persist.settings.sync.connected = false;
                    self.sync_status = SyncStatus::Error { msg, missing: Vec::new() };
                }
            }
        }
        if let Some(res) = self.sync.job.as_ref().and_then(|j| j.take()) {
            self.sync.job = None;
            changed = true;
            match res {
                Ok(out) => {
                    let n = self.files().len();
                    let place = out.place.clone();
                    self.apply_sync(out);
                    self.sync.error = None;
                    self.persist.settings.sync.last_sync = Some(super::sign::now_secs() * 1000);
                    self.persist_later();
                    if self.sync.manual {
                        self.sync_status = SyncStatus::Ok { msg: format!("Synced {n} {} with {place}.", if n == 1 { "note" } else { "notes" }) };
                    }
                }
                Err(msg) => {
                    if self.sync.manual {
                        self.sync_status = SyncStatus::Error { msg: msg.clone(), missing: Vec::new() };
                    }
                    self.sync.error = Some(msg);
                }
            }
            // After a sync that started by itself, the settings show the live state again.
            if !self.sync.manual && matches!(self.sync_status, SyncStatus::Ok { .. }) {
                self.sync_status = SyncStatus::Idle;
            }
            self.sync.manual = false;
            if std::mem::take(&mut self.sync.again) {
                self.sync_soon(0.5);
            }
        }
        if self.sync_ready() && self.sync.job.is_none() {
            if self.settings().sync.when == "interval" && self.sync.at.is_none() {
                let next = self.sync.last_try.map_or(t, |l| l + 300.0);
                self.sync.at = Some(next);
            }
            if let Some(at) = self.sync.at {
                if t >= at {
                    self.start_sync();
                    changed = true;
                } else {
                    tx.wake_after(Duration::from_secs_f64(at - t));
                }
            }
        }
        // "synced 2m ago" in the status bar moves on by itself.
        if self.sync_ready() {
            if t - self.sync.last_tick >= 30.0 {
                self.sync.last_tick = t;
                changed = true;
            }
            tx.wake_after(Duration::from_secs(30));
        }
        changed
    }

    /// Lays a sync's outcome over the open folder. A note that changed here while the sync ran
    /// keeps its new text, and the remote text comes in beside it as a conflict copy. A note
    /// another device deleted stays if it changed here. The next sync sends both up.
    pub(super) fn apply_sync(&mut self, mut out: Outcome) {
        // A result for a folder that is no longer open is dropped. Its base is not saved, so the
        // next sync of that folder starts from the last good record.
        let Some(root) = self.vault.as_ref().map(|v| v.path.clone()) else { return };
        if !out.root.as_os_str().is_empty() && out.root != root {
            return;
        }
        let failures_before = self.writer.failed.load(std::sync::atomic::Ordering::SeqCst);
        let base = out.base.take();
        let hash = |t: &str| super::sign::sha256_hex(t.as_bytes());
        let mut conflicts = out.conflicts;
        let mut any = false;
        for pl in out.pulls {
            let cur = self.files().get(&pl.path).map(|t| hash(t));
            if cur.as_deref() == Some(hash(&pl.text).as_str()) {
                continue;
            }
            let path = if self.dirty.contains(&pl.path) || cur != pl.seen {
                let taken: std::collections::BTreeSet<String> = self.files().keys().cloned().collect();
                let c = super::conflict_path(&pl.path, &pl.device, &taken);
                conflicts.push(c.clone());
                c
            } else {
                pl.path
            };
            let text = pl.text;
            self.writer.send(Op::Write { root: root.clone(), rel: path.clone(), text: text.clone() });
            // An unlocked note opens the new sealed text with its key, or locks again.
            if self.unlocked.contains_key(&path) {
                self.outside_locked_text(&path, text);
                any = true;
                continue;
            }
            if let Some(e) = self.edits.get_mut(&path) {
                let (a, b) = (e.anchor.min(text.len()), e.focus.min(text.len()));
                e.replace_text(text.clone());
                e.set_selection(a, b);
            }
            if let Some(v) = &mut self.vault {
                if !v.files.contains_key(&path) {
                    v.born.insert(path.clone(), crate::app::now_ns());
                    let mut d = crate::md::dir(&path).to_string();
                    while !d.is_empty() {
                        if !v.folders.contains(&d) {
                            v.folders.push(d.clone());
                        }
                        d = crate::md::dir(&d).to_string();
                    }
                    v.folders.sort();
                }
                v.files.insert(path, text);
            }
            any = true;
        }
        for (p, seen) in out.removes {
            let cur = self.files().get(&p).map(|t| hash(t));
            if self.dirty.contains(&p) || cur.as_deref() != Some(seen.as_str()) {
                continue;
            }
            if let Some(v) = &mut self.vault {
                v.files.remove(&p);
            }
            self.writer.send(Op::Delete { root: root.clone(), rel: p.clone() });
            if self.tabs.contains(&p) {
                self.close_tab(&p);
            }
            self.recent.retain(|x| x != &p);
            any = true;
        }
        // Images: written when the file here is still what the sync saw, or else as a copy.
        let file_hash = |p: &std::path::Path| std::fs::read(p).ok().map(|b| super::sign::sha256_hex(&b));
        let had_images = !out.image_pulls.is_empty() || !out.image_removes.is_empty();
        for (path, bytes, seen, device) in out.image_pulls {
            let cur = file_hash(&root.join(&path));
            let rel = if cur.is_some() && cur != seen {
                let taken: std::collections::BTreeSet<String> = crate::images::list(&root).into_iter().collect();
                let c = super::conflict_path(&path, &device, &taken);
                conflicts.push(c.clone());
                c
            } else {
                path
            };
            self.writer.send(Op::WriteBytes { root: root.clone(), rel, bytes });
        }
        for (path, seen) in out.image_removes {
            if file_hash(&root.join(&path)).as_deref() == Some(seen.as_str()) {
                self.writer.send(Op::Delete { root: root.clone(), rel: path });
            }
        }
        if had_images {
            self.images.forget_index();
        }
        for f in out.mkdirs {
            if let Some(v) = &mut self.vault
                && !v.folders.contains(&f)
            {
                v.folders.push(f.clone());
                v.folders.sort();
                self.writer.send(Op::Mkdir { root: root.clone(), rel: f });
                any = true;
            }
        }
        for f in out.rmdirs {
            // A note that came in or stayed here keeps its folder. The next sync puts it back.
            let known = self.vault.as_ref().is_some_and(|v| v.folders.contains(&f));
            if known && self.notes_in(&f).is_empty() {
                self.remove_folder(&f);
                any = true;
            }
        }
        // Pins: what the sync added or removed. A pin changed here while it ran stays changed.
        if let (Some(after), Some(sent)) = (out.pins.take(), self.sync.pins_sent.take()) {
            let n = self.pinned.clone();
            self.pinned.retain(|p| after.contains(p) || !sent.contains(p));
            for p in after.difference(&sent) {
                if !self.pinned.contains(p) {
                    self.pinned.push(p.clone());
                }
            }
            if self.pinned != n {
                self.persist_later();
                any = true;
            }
        }
        if let Some((path, bytes)) = base {
            self.writer.send(Op::SaveSyncBase { path, bytes, failures_before });
        }
        if !conflicts.is_empty() {
            self.notice = Some(format!(
                "{} changed on two devices. Quire kept both versions: {}.",
                if conflicts.len() == 1 { "A note" } else { "Some notes" },
                conflicts.join(", ")
            ));
            // The copies go up on the next sync.
            self.sync_auto(1.0);
        }
        if any {
            self.rebuild_names_pub();
        }
    }

    /// The status bar's sync label, or None when sync is off.
    pub fn sync_label(&self) -> Option<(String, bool, bool)> {
        let sy = &self.settings().sync;
        if sy.provider == "none" || !sy.connected {
            return None;
        }
        if self.sync.busy() {
            return Some(("syncing…".into(), true, false));
        }
        if let Some(e) = &self.sync.error {
            let _ = e;
            return Some(("sync failed".into(), false, true));
        }
        Some((sy.last_sync.map_or_else(|| "not synced yet".to_string(), |t| format!("synced {}", ago(t))), false, false))
    }
}
