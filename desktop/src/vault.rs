//! A notes folder on disk: reading it, and a writer thread that keeps disk work off the UI thread.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Sender, channel};
use std::sync::{Arc, Mutex};

#[derive(Clone, Debug)]
pub struct Vault {
    pub name: String,
    pub path: PathBuf,
    /// Folders relative to the root, `/`-separated, including empty ones.
    pub folders: Vec<String>,
    /// Note path relative to the root (`Drafts/On slow software.md`) to its text.
    pub files: BTreeMap<String, String>,
    /// Creation order of each note (file birth time, else modification time), for lists the
    /// design keeps in the order notes were made, such as backlinks.
    pub born: std::collections::HashMap<String, u128>,
}

fn skip(name: &str) -> bool {
    name.starts_with('.') || name == "node_modules"
}

/// Reads every `.md` file under `root`. Hidden folders (`.git`, `.trash`) stay out.
pub fn load(root: &Path) -> std::io::Result<Vault> {
    let mut files = BTreeMap::new();
    let mut born = std::collections::HashMap::new();
    let mut folders = Vec::new();
    let mut stack = vec![(root.to_path_buf(), String::new())];
    let mut count = 0usize;
    while let Some((dir, rel)) = stack.pop() {
        let rd = match std::fs::read_dir(&dir) {
            Ok(r) => r,
            Err(e) if rel.is_empty() => return Err(e),
            Err(_) => continue,
        };
        for entry in rd.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if skip(&name) {
                continue;
            }
            let Ok(ft) = entry.file_type() else { continue };
            let child = if rel.is_empty() { name.clone() } else { format!("{rel}/{name}") };
            if ft.is_dir() {
                folders.push(child.clone());
                stack.push((entry.path(), child));
            } else if ft.is_file() && name.to_lowercase().ends_with(".md") {
                count += 1;
                if count > 20_000 {
                    continue;
                }
                if let Ok(t) = std::fs::read_to_string(entry.path()) {
                    let meta = entry.metadata().ok();
                    let when = meta.and_then(|m| m.created().or_else(|_| m.modified()).ok());
                    let ns = when.and_then(|w| w.duration_since(std::time::UNIX_EPOCH).ok()).map_or(0, |d| d.as_nanos());
                    born.insert(child.clone(), ns);
                    files.insert(child, t.replace("\r\n", "\n"));
                }
            }
        }
    }
    folders.sort();
    let name = root.file_name().map_or_else(|| root.to_string_lossy().into_owned(), |n| n.to_string_lossy().into_owned());
    Ok(Vault { name, path: root.to_path_buf(), folders, files, born })
}

pub fn count_notes(root: &Path) -> usize {
    let mut n = 0;
    let mut stack = vec![root.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&d) else { continue };
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            if skip(&name) {
                continue;
            }
            match e.file_type() {
                Ok(t) if t.is_dir() => stack.push(e.path()),
                Ok(t) if t.is_file() && name.to_lowercase().ends_with(".md") => n += 1,
                _ => {}
            }
        }
        if n > 99_999 {
            break;
        }
    }
    n
}

#[derive(Debug)]
pub enum Op {
    Write { root: PathBuf, rel: String, text: String },
    Rename { root: PathBuf, from: String, to: String },
    Delete { root: PathBuf, rel: String },
    /// Writes a file that is not a note, such as an image from sync.
    WriteBytes { root: PathBuf, rel: String, bytes: Vec<u8> },
    /// Moves a folder, with every file in it, to `.trash`.
    DeleteFolder { root: PathBuf, rel: String },
    Mkdir { root: PathBuf, rel: String },
    SaveState(Box<crate::store::Persist>),
    /// Answers once every operation sent before it is done.
    Barrier(std::sync::mpsc::Sender<()>),
    /// Saves a sync's base record after the sync's writes. If any operation failed after
    /// `failures_before`, it deletes the record, so the next sync keeps both texts of a note
    /// instead of taking the old one for an edit.
    SaveSyncBase { path: PathBuf, bytes: Vec<u8>, failures_before: usize },
}

/// Disk work runs on this thread in the order it was sent. Errors land in `errors`.
#[derive(Clone)]
pub struct Writer {
    tx: Sender<Op>,
    pub errors: Arc<Mutex<Vec<String>>>,
    /// How many operations failed since the start. It only goes up.
    pub failed: Arc<std::sync::atomic::AtomicUsize>,
}

impl Writer {
    pub fn start(waker: Option<rune::Waker>) -> Writer {
        let (tx, rx) = channel::<Op>();
        let errors: Arc<Mutex<Vec<String>>> = Arc::default();
        let errs = errors.clone();
        let failed: Arc<std::sync::atomic::AtomicUsize> = Arc::default();
        let failed2 = failed.clone();
        std::thread::Builder::new()
            .name("typo-writer".into())
            .spawn(move || {
                while let Ok(op) = rx.recv() {
                    if let Op::Barrier(done) = &op {
                        let _ = done.send(());
                        continue;
                    }
                    if let Op::SaveSyncBase { path, bytes, failures_before } = &op {
                        let r = if failed2.load(std::sync::atomic::Ordering::SeqCst) > *failures_before {
                            std::fs::remove_file(path).or_else(|e| if e.kind() == std::io::ErrorKind::NotFound { Ok(()) } else { Err(e) })
                        } else {
                            write_atomic(path, bytes)
                        };
                        if let Err(e) = r {
                            eprintln!("typo: could not save the sync record: {e}");
                        }
                        continue;
                    }
                    if let Err(e) = apply(&op) {
                        failed2.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                        let what = match &op {
                            Op::Write { rel, .. } | Op::WriteBytes { rel, .. } | Op::Delete { rel, .. } | Op::DeleteFolder { rel, .. } | Op::Mkdir { rel, .. } => rel.clone(),
                            Op::Rename { from, .. } => from.clone(),
                            Op::SaveState(_) => "settings".into(),
                            Op::Barrier(_) | Op::SaveSyncBase { .. } => String::new(),
                        };
                        if let Ok(mut v) = errs.lock() {
                            v.push(format!("Could not save {what}: {e}"));
                        }
                        if let Some(w) = &waker {
                            w();
                        }
                    }
                }
            })
            .ok();
        Writer { tx, errors, failed }
    }

    pub fn send(&self, op: Op) {
        let _ = self.tx.send(op);
    }
}

/// Writes a file through a temporary file next to it and a rename, so a reader never sees half.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    if let Some(d) = path.parent() {
        std::fs::create_dir_all(d)?;
    }
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(&tmp, path)
}

/// Writes a note atomically: a temporary file next to it, then a rename.
pub fn write_note(root: &Path, rel: &str, text: &str) -> std::io::Result<()> {
    apply(&Op::Write { root: root.to_path_buf(), rel: rel.to_string(), text: text.to_string() })
}

impl Writer {
    /// Waits, up to `max`, until every operation sent so far is done.
    pub fn wait(&self, max: std::time::Duration) {
        let (tx, rx) = std::sync::mpsc::channel();
        self.send(Op::Barrier(tx));
        let _ = rx.recv_timeout(max);
    }

    /// A barrier for another thread to wait on.
    pub fn barrier(&self) -> std::sync::mpsc::Receiver<()> {
        let (tx, rx) = std::sync::mpsc::channel();
        self.send(Op::Barrier(tx));
        rx
    }
}

fn apply(op: &Op) -> std::io::Result<()> {
    match op {
        Op::Write { root, rel, text } => {
            let p = root.join(rel);
            if let Some(d) = p.parent() {
                std::fs::create_dir_all(d)?;
            }
            let tmp = p.with_file_name(format!(".{}.typo-tmp", p.file_name().map_or_else(String::new, |n| n.to_string_lossy().into_owned())));
            std::fs::write(&tmp, text)?;
            std::fs::rename(&tmp, &p)
        }
        Op::Rename { root, from, to } => {
            let dst = root.join(to);
            if let Some(d) = dst.parent() {
                std::fs::create_dir_all(d)?;
            }
            std::fs::rename(root.join(from), dst)
        }
        Op::WriteBytes { root, rel, bytes } => write_atomic(&root.join(rel), bytes),
        Op::Delete { root, rel } => {
            // Move the file to `.trash` inside the folder, so it can come back.
            let src = root.join(rel);
            let trash = root.join(".trash");
            std::fs::create_dir_all(&trash)?;
            let name = rel.replace('/', " - ");
            let (stem, ext) = name.rsplit_once('.').map_or((name.as_str(), "md"), |(s, e)| (s, e));
            let mut dst = trash.join(&name);
            let mut n = 2;
            while dst.exists() {
                dst = trash.join(format!("{stem} {n}.{ext}"));
                n += 1;
            }
            std::fs::rename(src, dst)
        }
        Op::DeleteFolder { root, rel } => {
            let src = root.join(rel);
            if !src.is_dir() {
                return Ok(());
            }
            let trash = root.join(".trash");
            std::fs::create_dir_all(&trash)?;
            let name = rel.replace('/', " - ");
            let mut dst = trash.join(&name);
            let mut n = 2;
            while dst.exists() {
                dst = trash.join(format!("{name} {n}"));
                n += 1;
            }
            std::fs::rename(src, dst)
        }
        Op::Mkdir { root, rel } => std::fs::create_dir_all(root.join(rel)),
        Op::SaveState(p) => crate::store::save(p),
        Op::Barrier(_) | Op::SaveSyncBase { .. } => Ok(()),
    }
}

/// Writes the sample notes into `root`, replacing files with the same names.
pub fn write_sample(root: &Path) -> std::io::Result<()> {
    for f in crate::sample::FOLDERS {
        std::fs::create_dir_all(root.join(f))?;
    }
    // Files keep the order they were made in (some lists follow it). File times tick in
    // steps of a few milliseconds, so leave a gap between the notes.
    for (rel, text) in crate::sample::files() {
        let p = root.join(rel);
        let _ = std::fs::remove_file(&p);
        std::fs::write(&p, text)?;
        std::thread::sleep(std::time::Duration::from_millis(6));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_deleted_folder_moves_to_the_trash_with_its_files() {
        let root = std::env::temp_dir().join(format!("typo-delfolder-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        for p in ["Drafts/Old/A.md", "Drafts/Old/img.png"] {
            std::fs::create_dir_all(root.join(p).parent().unwrap()).unwrap();
            std::fs::write(root.join(p), "x").unwrap();
        }
        let del = |rel: &str| apply(&Op::DeleteFolder { root: root.clone(), rel: rel.into() }).unwrap();
        del("Drafts/Old");
        assert!(root.join(".trash/Drafts - Old/A.md").is_file());
        assert!(root.join(".trash/Drafts - Old/img.png").is_file(), "files that are not notes move too");
        assert!(!root.join("Drafts/Old").exists());
        // A second folder with the same path gets a number.
        std::fs::create_dir_all(root.join("Drafts/Old")).unwrap();
        del("Drafts/Old");
        assert!(root.join(".trash/Drafts - Old 2").is_dir());
        // A folder that is not on disk is no error.
        del("Missing");
        let _ = std::fs::remove_dir_all(&root);
    }
}
