//! Watches the open folder for changes made by other apps (an editor, a sync tool, the MCP
//! server) by checking file times every 1.5 s on its own thread.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

/// What changed on disk since the last look: new or changed notes with their text, and
/// removed notes.
#[derive(Default, Debug)]
pub struct Changes {
    pub changed: Vec<(String, String)>,
    pub removed: Vec<String>,
}

pub struct Watcher {
    pub inbox: Arc<Mutex<Changes>>,
    stop: Arc<AtomicBool>,
}

impl Drop for Watcher {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
    }
}

fn snapshot(root: &PathBuf) -> BTreeMap<String, (SystemTime, u64)> {
    let mut out = BTreeMap::new();
    let mut stack = vec![(root.clone(), String::new())];
    while let Some((dir, rel)) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&dir) else { continue };
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            if name.starts_with('.') || name == "node_modules" {
                continue;
            }
            let child = if rel.is_empty() { name.clone() } else { format!("{rel}/{name}") };
            let Ok(m) = e.metadata() else { continue };
            if m.is_dir() {
                stack.push((e.path(), child));
            } else if name.to_lowercase().ends_with(".md") {
                out.insert(child, (m.modified().unwrap_or(SystemTime::UNIX_EPOCH), m.len()));
            }
        }
        if out.len() > 20_000 {
            break;
        }
    }
    out
}

pub fn start(root: PathBuf, waker: Option<rune::Waker>) -> Watcher {
    let inbox: Arc<Mutex<Changes>> = Arc::default();
    let stop = Arc::new(AtomicBool::new(false));
    let (inbox2, stop2) = (inbox.clone(), stop.clone());
    let _ = std::thread::Builder::new().name("typo-watch".into()).spawn(move || {
        let mut last = snapshot(&root);
        while !stop2.load(Ordering::SeqCst) {
            std::thread::sleep(Duration::from_millis(1500));
            if stop2.load(Ordering::SeqCst) {
                break;
            }
            let now = snapshot(&root);
            if now == last {
                continue;
            }
            let mut c = Changes::default();
            for (rel, meta) in &now {
                if last.get(rel) != Some(meta) {
                    if let Ok(t) = std::fs::read_to_string(root.join(rel)) {
                        c.changed.push((rel.clone(), t.replace("\r\n", "\n")));
                    }
                }
            }
            c.removed = last.keys().filter(|k| !now.contains_key(*k)).cloned().collect();
            last = now;
            if c.changed.is_empty() && c.removed.is_empty() {
                continue;
            }
            if let Ok(mut b) = inbox2.lock() {
                b.changed.extend(c.changed);
                b.removed.extend(c.removed);
            }
            if let Some(w) = &waker {
                w();
            }
        }
    });
    Watcher { inbox, stop }
}

/// Watches the Omarchy theme for switches. The flag turns on, and the app wakes, when the
/// theme's files change.
pub fn watch_theme(waker: rune::Waker) -> Arc<AtomicBool> {
    let flag = Arc::new(AtomicBool::new(false));
    let f2 = flag.clone();
    let _ = std::thread::Builder::new().name("typo-theme".into()).spawn(move || {
        let stamp = || {
            let d = crate::theme::omarchy_dir()?;
            let generation = d.parent().map(|p| p.join("theme.generation"));
            let m = |p: &std::path::Path| std::fs::metadata(p).and_then(|m| m.modified()).ok();
            Some((generation.as_deref().and_then(m), m(&d.join("colors.toml")), d))
        };
        let mut last = stamp();
        loop {
            std::thread::sleep(Duration::from_millis(1500));
            let now = stamp();
            if now != last {
                last = now;
                f2.store(true, Ordering::SeqCst);
                waker();
            }
        }
    });
    flag
}

#[cfg(test)]
mod tests {
    #[test]
    fn reports_outside_edits_and_removals() {
        let dir = std::env::temp_dir().join(format!("typo-watch-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("Sub")).unwrap();
        std::fs::write(dir.join("A.md"), "one").unwrap();
        std::fs::write(dir.join("Sub/B.md"), "two").unwrap();
        let w = super::start(dir.clone(), None);
        std::thread::sleep(std::time::Duration::from_millis(300));
        std::fs::write(dir.join("A.md"), "one, edited elsewhere").unwrap();
        std::fs::remove_file(dir.join("Sub/B.md")).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(2200));
        let c = std::mem::take(&mut *w.inbox.lock().unwrap());
        assert_eq!(c.changed, vec![("A.md".to_string(), "one, edited elsewhere".to_string())]);
        assert_eq!(c.removed, vec!["Sub/B.md".to_string()]);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
