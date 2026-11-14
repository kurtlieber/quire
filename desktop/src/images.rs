//! Images in notes: where an image's file is, the web images the preview downloads, and the
//! images you insert, paste or drop into a note. Inserted images go to `attachments/` in the
//! notes folder, and the note links them with a path relative to its own folder.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use rune::{FileDialogOptions, Tx};

use crate::app::App;
use crate::md::{self, Img};

/// The folder inside the notes folder that takes inserted images.
pub const ATTACHMENTS: &str = "attachments";
/// Larger files neither insert nor sync.
pub const MAX_BYTES: u64 = 50 * 1024 * 1024;

/// Where an image's pixels come from.
pub enum Source {
    File(PathBuf),
    Url(String),
    /// No file at the path the note gives.
    Missing(String),
}

/// A web image: on its way, here, or failed.
#[derive(Clone)]
pub enum Remote {
    Loading,
    Ready(rune::Image),
    Failed(String),
}

/// The image cache and the web images, made when the app gets its waker.
#[derive(Default)]
pub struct Images {
    pub cache: Option<Arc<rune::ImageCache>>,
    remote: Arc<Mutex<HashMap<String, Remote>>>,
    waker: Option<rune::Waker>,
    /// The image files in the open notes folder, for `![[name]]` embeds. Made on first use.
    index: std::cell::RefCell<Option<Vec<String>>>,
    /// Handles of the files the preview shows, by path and file time. The cache frees an image
    /// when its last handle drops, so these keep a loading image alive between frames.
    files: std::cell::RefCell<HashMap<(PathBuf, u128), rune::Image>>,
    /// How many kept images had loaded at the last poll.
    done: std::cell::Cell<usize>,
}

impl Images {
    pub fn start(&mut self, waker: rune::Waker) {
        self.cache = Some(Arc::new(rune::ImageCache::new(waker.clone())));
        self.waker = Some(waker);
    }

    /// An image file, loaded once and kept while the preview shows it. A file that changed on
    /// disk loads again.
    pub fn file(&self, path: &Path) -> Option<rune::Image> {
        let cache = self.cache.as_ref()?;
        let t = std::fs::metadata(path).and_then(|m| m.modified()).ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map_or(0, |d| d.as_nanos());
        let mut files = self.files.borrow_mut();
        if files.len() > 96 {
            files.clear();
        }
        Some(files.entry((path.to_path_buf(), t)).or_insert_with(|| cache.file(path, 1200.0, crate::theme::scale())).clone())
    }

    /// True when an image loaded since the last call, so the view runs again with its size.
    pub fn poll(&self) -> bool {
        let n = self.files.borrow().values().filter(|i| i.is_ready()).count()
            + self.remote.lock().unwrap_or_else(std::sync::PoisonError::into_inner).values().filter(|r| !matches!(r, Remote::Loading)).count();
        let changed = n != self.done.get();
        self.done.set(n);
        changed
    }

    /// The folder's image files changed: the next embed lookup lists them again.
    pub fn forget_index(&self) {
        *self.index.borrow_mut() = None;
    }

    /// A web image, downloaded once on another thread. Later calls get it from memory.
    pub fn remote(&self, url: &str) -> Remote {
        let mut map = self.remote.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(r) = map.get(url) {
            return r.clone();
        }
        let Some(cache) = self.cache.clone() else { return Remote::Failed("No image cache.".into()) };
        map.insert(url.to_string(), Remote::Loading);
        let (slot, waker, url2) = (self.remote.clone(), self.waker.clone(), url.to_string());
        let _ = std::thread::Builder::new().name("typo-image".into()).spawn(move || {
            let got = fetch(&url2).map(|bytes| cache.load(bytes));
            let r = match got {
                Ok(img) => Remote::Ready(img),
                Err(e) => Remote::Failed(e),
            };
            slot.lock().unwrap_or_else(std::sync::PoisonError::into_inner).insert(url2, r);
            if let Some(w) = waker {
                w();
            }
        });
        Remote::Loading
    }
}

fn fetch(url: &str) -> Result<Vec<u8>, String> {
    use std::io::Read;
    let agent = ureq::AgentBuilder::new().timeout_connect(std::time::Duration::from_secs(10)).timeout(std::time::Duration::from_secs(30)).build();
    let resp = agent.get(url).call().map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    resp.into_reader().take(MAX_BYTES).read_to_end(&mut out).map_err(|e| e.to_string())?;
    Ok(out)
}

/// Every image file under `root`, as paths relative to it. Hidden folders stay out.
pub fn list(root: &Path) -> Vec<String> {
    let mut out = Vec::new();
    let mut stack = vec![(root.to_path_buf(), String::new())];
    while let Some((dir, rel)) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&dir) else { continue };
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            if name.starts_with('.') || name == "node_modules" {
                continue;
            }
            let child = if rel.is_empty() { name.clone() } else { format!("{rel}/{name}") };
            match e.file_type() {
                Ok(t) if t.is_dir() => stack.push((e.path(), child)),
                Ok(t) if t.is_file() && md::is_image(&name) => out.push(child),
                _ => {}
            }
        }
    }
    out.sort();
    out
}

/// `name` with a number before the extension until no file in `dir` has it.
fn unique(dir: &Path, name: &str) -> String {
    let (stem, ext) = name.rsplit_once('.').map_or((name, ""), |(s, e)| (s, e));
    let mut n = 1;
    loop {
        let cand = match (n, ext.is_empty()) {
            (1, true) => stem.to_string(),
            (1, false) => format!("{stem}.{ext}"),
            (_, true) => format!("{stem} {n}"),
            (_, false) => format!("{stem} {n}.{ext}"),
        };
        if !dir.join(&cand).exists() {
            return cand;
        }
        n += 1;
    }
}

impl App {
    /// Where the pixels of an image in the active note come from.
    pub fn image_source(&self, img: &Img) -> Source {
        let src = img.src.trim();
        if src.starts_with("http://") || src.starts_with("https://") {
            return Source::Url(src.to_string());
        }
        let Some(v) = &self.vault else { return Source::Missing(src.to_string()) };
        let dir = self.active.as_deref().map_or("", md::dir);
        if img.embed {
            let mut idx = self.images.index.borrow_mut();
            let all = idx.get_or_insert_with(|| list(&v.path));
            let want = src.to_lowercase();
            let name = want.rsplit('/').next().unwrap_or(&want).to_string();
            // The exact path first, then the same name in the note's folder, then anywhere.
            let hit = all
                .iter()
                .find(|p| p.to_lowercase() == want)
                .or_else(|| all.iter().find(|p| md::dir(p) == dir && p.to_lowercase().ends_with(&name)))
                .or_else(|| all.iter().find(|p| p.rsplit('/').next().is_some_and(|n| n.to_lowercase() == name)));
            return hit.map_or_else(|| Source::Missing(src.to_string()), |p| Source::File(v.path.join(p)));
        }
        if let Some(p) = src.strip_prefix("file://") {
            return Source::File(PathBuf::from(p));
        }
        if Path::new(src).is_absolute() {
            return Source::File(PathBuf::from(src));
        }
        // Relative to the note, as Markdown means it, then relative to the notes folder.
        for base in [dir, ""] {
            if let Some(rel) = md::resolve(base, src) {
                let p = v.path.join(&rel);
                if p.is_file() {
                    return Source::File(p);
                }
            }
        }
        Source::Missing(src.to_string())
    }

    /// Links image files at the caret of the active note. Files from outside the notes folder
    /// are copied into `attachments/` first. False when none of the files is an image.
    pub fn insert_images(&mut self, paths: &[PathBuf], tx: &mut Tx) -> bool {
        let Some(root) = self.vault.as_ref().map(|v| v.path.clone()) else { return false };
        if self.active.is_none() {
            self.notice = Some("Open a note first, then add the image to it.".into());
            return false;
        }
        if self.active.as_deref().is_some_and(|a| self.note_locked(a)) {
            self.notice = Some("Unlock the note first, then add the image to it.".into());
            return false;
        }
        let images: Vec<&PathBuf> = paths.iter().filter(|p| md::is_image(&p.to_string_lossy())).collect();
        if images.is_empty() {
            if !paths.is_empty() {
                self.notice = Some("Only images can go into a note: PNG, JPEG, GIF, WebP, BMP, SVG or AVIF.".into());
            }
            return false;
        }
        let mut rels = Vec::new();
        for p in images {
            let inside = p.canonicalize().ok().zip(root.canonicalize().ok()).and_then(|(p, r)| p.strip_prefix(&r).ok().map(Path::to_path_buf));
            let rel = match inside {
                Some(r) if !r.components().any(|c| c.as_os_str().to_string_lossy().starts_with('.')) => r.to_string_lossy().replace('\\', "/"),
                _ => {
                    if std::fs::metadata(p).map_or(0, |m| m.len()) > MAX_BYTES {
                        self.notice = Some(format!("{} is larger than 50 MB, so Typo did not add it.", p.file_name().map_or_else(String::new, |n| n.to_string_lossy().into_owned())));
                        continue;
                    }
                    let dir = root.join(ATTACHMENTS);
                    let name = p.file_name().map_or_else(|| "image.png".to_string(), |n| n.to_string_lossy().into_owned());
                    let name = unique(&dir, &name);
                    if let Err(e) = std::fs::create_dir_all(&dir).and_then(|_| std::fs::copy(p, dir.join(&name))) {
                        self.notice = Some(format!("Could not copy {name}: {e}"));
                        continue;
                    }
                    format!("{ATTACHMENTS}/{name}")
                }
            };
            rels.push(rel);
        }
        self.link_images(&rels, tx)
    }

    /// Saves pasted PNG bytes into `attachments/` and links them at the caret.
    pub fn paste_image(&mut self, png: &[u8], tx: &mut Tx) -> bool {
        let Some(root) = self.vault.as_ref().map(|v| v.path.clone()) else { return false };
        if self.active.is_none() {
            return false;
        }
        let ms = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis() as i64);
        let t = rune::LocalTime::at(ms);
        let name = format!("Pasted image {:04}-{:02}-{:02} {:02}.{:02}.{:02}.png", t.year, t.month, t.day, t.hour, t.minute, t.second);
        let dir = root.join(ATTACHMENTS);
        let name = unique(&dir, &name);
        if let Err(e) = std::fs::create_dir_all(&dir).and_then(|_| crate::vault::write_atomic(&dir.join(&name), png)) {
            self.notice = Some(format!("Could not save the pasted image: {e}"));
            return false;
        }
        self.link_images(&[format!("{ATTACHMENTS}/{name}")], tx)
    }

    /// A paste into the editor:
    /// - Image files and a copied image become links.
    /// - Formatted text from a web page or a document goes in as Markdown.
    /// - A web address pasted over selected text links that text.
    ///
    /// Other pastes, and every paste inside a code block, go to the text field as usual.
    pub fn paste_content(&mut self, content: &rune::ClipboardContent, tx: &mut Tx) -> bool {
        if content.files().iter().any(|f| md::is_image(&f.to_string_lossy())) {
            return self.insert_images(content.files(), tx);
        }
        if let Some(img) = content.image().filter(|_| content.text().trim().is_empty()) {
            return self.paste_image(img.png(), tx);
        }
        let e = self.cur_edit();
        let (a, b) = (e.anchor.min(e.focus), e.anchor.max(e.focus).min(e.text.len()));
        if in_code_block(&e.text, a) {
            return false;
        }
        let insert = if let Some(html) = content.html().filter(|h| crate::html2md::has_formatting(h)) {
            crate::html2md::convert(html)
        } else {
            let url = content.text().trim();
            let sel = &e.text[a..b];
            if !is_web_address(url) || a == b || sel.contains('\n') || is_web_address(sel.trim()) {
                return false;
            }
            format!("[{sel}]({url})")
        };
        if insert.trim().is_empty() {
            return false;
        }
        self.cur_edit().paste(&insert);
        self.after_edit();
        self.focus_editor(tx);
        true
    }

    /// Asks for an image file, then links it in the active note.
    pub fn pick_image(&mut self, tx: &mut Tx) {
        if self.active.is_none() {
            return;
        }
        let options = FileDialogOptions { title: "Insert an image".into(), directory: Some(crate::store::documents_dir()), ..Default::default() }.filter("Images", &md::IMAGE_EXT);
        tx.open_file(options, |s: &mut App, result, tx| match result {
            Ok(Some(path)) => {
                s.insert_images(&[path], tx);
            }
            Ok(None) => {}
            Err(e) => s.notice = Some(format!("Could not show the file picker: {e}")),
        });
    }

    /// Writes `![name](path)` lines at the caret, each on a line of its own.
    fn link_images(&mut self, rels: &[String], tx: &mut Tx) -> bool {
        if rels.is_empty() {
            return false;
        }
        let dir = self.active.as_deref().map_or("", md::dir).to_string();
        let lines: Vec<String> = rels
            .iter()
            .map(|r| {
                let file = r.rsplit('/').next().unwrap_or(r);
                let alt = file.rsplit_once('.').map_or(file, |(b, _)| b);
                format!("![{}]({})", alt.replace(['[', ']'], ""), md::link_path(&md::relative(&dir, r)))
            })
            .collect();
        let e = self.cur_edit();
        let caret = e.focus.min(e.text.len());
        let before = e.text[..caret].chars().last();
        let after = e.text[caret..].chars().next();
        let mut s = String::new();
        if before.is_some_and(|c| c != '\n') {
            s.push('\n');
        }
        s.push_str(&lines.join("\n"));
        if after.is_none_or(|c| c != '\n') {
            s.push('\n');
        }
        e.paste(&s);
        self.after_edit();
        self.images.forget_index();
        self.focus_editor(tx);
        true
    }
}

/// Whether `at` is inside a fenced code block: an odd number of fence lines come before it.
fn in_code_block(text: &str, at: usize) -> bool {
    let at = at.min(text.len());
    let before = text.get(..at).unwrap_or(text);
    let fences = before.split('\n').filter(|l| {
        let t = l.trim_start();
        t.starts_with("```") || t.starts_with("~~~")
    });
    // The line the caret is on counts only when it ends before the caret.
    fences.count() % 2 == 1
}

/// One web address and nothing else.
fn is_web_address(s: &str) -> bool {
    (s.starts_with("https://") || s.starts_with("http://")) && s.len() > 8 && !s.chars().any(char::is_whitespace)
}

#[cfg(test)]
mod paste_tests {
    use super::*;

    #[test]
    fn code_blocks_and_addresses() {
        let t = "a\n```\ncode\n```\nb";
        assert!(!in_code_block(t, 1));
        assert!(in_code_block(t, t.find("code").unwrap()));
        assert!(!in_code_block(t, t.len()));
        assert!(is_web_address("https://example.com/a?b=c"));
        assert!(!is_web_address("https://example.com and more"));
        assert!(!is_web_address("example.com"));
    }
}
