//! Sync: keeps a notes folder the same on every device through an object store.
//!
//! The remote folder holds `.quire/format.json` (plain: whether the notes are encrypted, and the
//! key's salt), a manifest (`.quire/manifest.json`, or `.quire/manifest.bin` when encrypted)
//! that maps each note's path to the hash of its text and its object, and one object per note.
//! A sync compares, for each note, the local hash, the remote hash and the hash both had at the
//! last sync on this device. The side that changed wins. When both changed, the note keeps the
//! local text and the remote text comes down beside it as `name (conflict device).md`.
//! `SYNC.md` describes the format for other implementations.

pub mod app;
pub mod crypto;
pub mod sign;
pub mod store;

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::md;
use store::Store;

const FORMAT: &str = ".quire/format.json";

/// What sync needs from the settings.
#[derive(Clone, Debug)]
pub struct Remote {
    pub provider: String,
    pub cfg: BTreeMap<String, String>,
    pub prefix: String,
    pub encrypt: bool,
    pub pass: String,
}

impl Remote {
    fn val(&self, k: &str, d: &str) -> String {
        self.cfg.get(k).map(|v| v.trim().to_string()).filter(|v| !v.is_empty()).unwrap_or_else(|| d.to_string())
    }

    /// A stable id for this folder on this store, for the last-sync record.
    fn id(&self, root: &Path) -> String {
        let key = format!("{}|{}|{}|{}|{}", root.display(), self.provider, self.val("bucket", ""), self.val("container", ""), self.prefix);
        sign::sha256_hex(key.as_bytes())[..20].to_string()
    }
}

/// The region the settings show first for a provider, used until one is picked.
fn default_region(pid: &str) -> &'static str {
    crate::ui::settings_sync::PROVIDERS.iter().find(|p| p.id == pid).and_then(|p| p.regions.first()).map_or("us-east-1", |r| r.0)
}

/// The remote folder inside the bucket, always ending in `/` unless empty.
pub fn clean_prefix(p: &str) -> String {
    let t = p.trim().trim_matches('/');
    if t.is_empty() { String::new() } else { format!("{t}/") }
}

pub fn open_store(r: &Remote) -> Result<Box<dyn Store>, String> {
    let pid = r.provider.as_str();
    let region = r.val("region", default_region(pid));
    let s3 = |endpoint: String, sign_region: &str, path_style: bool, secret_label: &str| -> Box<dyn Store> {
        Box::new(store::S3::new(&endpoint, &r.val("bucket", ""), sign_region, &r.val("accessKey", ""), &r.val("secretKey", ""), path_style, &r.prefix, secret_label))
    };
    Ok(match pid {
        "s3" => s3(format!("https://s3.{region}.amazonaws.com"), &region, false, "secret access key"),
        "hetzner" => s3(format!("https://{region}.your-objectstorage.com"), &region, true, "secret access key"),
        "r2" => s3(format!("https://{}.r2.cloudflarestorage.com", r.val("accountId", "")), "auto", true, "secret access key"),
        "b2" => s3(format!("https://s3.{region}.backblazeb2.com"), &region, true, "application key"),
        "do" => s3(format!("https://{region}.digitaloceanspaces.com"), "us-east-1", true, "secret access key"),
        "wasabi" => s3(format!("https://s3.{region}.wasabisys.com"), &region, true, "secret access key"),
        "scaleway" => s3(format!("https://s3.{region}.scw.cloud"), &region, true, "secret access key"),
        "ovh" => s3(format!("https://s3.{region}.io.cloud.ovh.net"), &region, true, "secret access key"),
        "custom" => s3(r.val("endpoint", ""), &r.val("region", "us-east-1"), r.val("addressing", "path") != "vhost", "secret access key"),
        "azure" => Box::new(store::Azure::new(&r.val("account", ""), &r.val("container", ""), &r.val("azAuth", "key"), &r.val("azKey", ""), &r.prefix)?),
        "gcs" if r.val("gcsAuth", "sa") == "hmac" => Box::new(store::S3::new(
            "https://storage.googleapis.com",
            &r.val("bucket", ""),
            "auto",
            &r.val("hmacId", ""),
            &r.val("hmacSecret", ""),
            true,
            &r.prefix,
            "HMAC secret",
        )),
        "gcs" => Box::new(store::Gcs::new(&r.val("bucket", ""), &r.val("gcsJson", ""), &r.prefix)?),
        _ => return Err("Pick a storage backend first.".into()),
    })
}

#[derive(Serialize, Deserialize, Default)]
struct Format {
    version: u32,
    encrypted: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    kdf: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    iterations: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    salt: Option<String>,
    /// `quire` sealed with the key, to tell a wrong passphrase at once.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    check: Option<String>,
}

#[derive(Serialize, Deserialize, Default, Clone)]
struct Manifest {
    version: u32,
    files: BTreeMap<String, Entry>,
    /// Every folder, empty ones included. Folders that hold notes are here too.
    #[serde(default)]
    folders: BTreeSet<String>,
    /// The pinned notes. None in a manifest from a Quire version without pins.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pins: Option<BTreeSet<String>>,
}

/// What both sides had after the last sync on this device.
#[derive(Serialize, Deserialize, Default)]
struct Base {
    files: BTreeMap<String, String>,
    #[serde(default)]
    folders: BTreeSet<String>,
    /// For each image: its size, its file time in nanoseconds and its hash, so an image that
    /// did not change is not read again. Only this device uses it.
    #[serde(default)]
    stamps: BTreeMap<String, (u64, u64, String)>,
    /// The pins after the last sync. None in a record from before pins synced.
    #[serde(default)]
    pins: Option<BTreeSet<String>>,
}

/// A file this device has: a note's text, or an image on disk.
enum Local {
    Note(String),
    Image(PathBuf),
}

/// The bytes to upload for a local file.
fn body(l: &Local) -> Result<std::borrow::Cow<'_, [u8]>, String> {
    match l {
        Local::Note(t) => Ok(std::borrow::Cow::Borrowed(t.as_bytes())),
        Local::Image(path) => std::fs::read(path).map(std::borrow::Cow::Owned).map_err(|e| format!("Could not read {}: {e}", path.display())),
    }
}

/// The size and file time of a file, to tell when it changed.
fn stamp(p: &Path) -> Option<(u64, u64)> {
    let m = std::fs::metadata(p).ok()?;
    let t = m.modified().ok()?.duration_since(std::time::UNIX_EPOCH).ok()?.as_nanos() as u64;
    Some((m.len(), t))
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
struct Entry {
    hash: String,
    object: String,
    time: i64,
    device: String,
}

/// A note to write locally.
#[derive(Debug, Clone, PartialEq)]
pub struct Pull {
    pub path: String,
    pub text: String,
    /// The hash of the local text this sync saw, or None when the note was not there. If the
    /// note changed since, the app keeps both versions.
    pub seen: Option<String>,
    /// The device that wrote the remote text.
    pub device: String,
}

/// What a sync did, and what the app must still apply to the open folder.
#[derive(Debug, Default)]
pub struct Outcome {
    pub pulls: Vec<Pull>,
    /// Notes another device deleted, with the local hash this sync saw. The app moves them to
    /// `.trash` if they did not change since.
    pub removes: Vec<(String, String)>,
    pub uploaded: usize,
    pub deleted_remote: usize,
    pub conflicts: Vec<String>,
    /// Folders another device made. The app makes them here.
    pub mkdirs: Vec<String>,
    /// Folders another device deleted. The app moves each one to `.trash` if no note is left
    /// in it.
    pub rmdirs: Vec<String>,
    /// Images to write here: path, bytes, the local hash this sync saw, and the device.
    pub image_pulls: Vec<(String, Vec<u8>, Option<String>, String)>,
    /// Images another device deleted, with the local hash this sync saw.
    pub image_removes: Vec<(String, String)>,
    pub place: String,
    /// The notes folder this sync ran on.
    pub root: PathBuf,
    /// The pins after this sync. The app adds and removes what changed from the pins it sent.
    pub pins: Option<BTreeSet<String>>,
    /// The new base record and where it goes. The app saves it after it applied the rest, so
    /// a note that never reached the disk is not taken as in step.
    pub base: Option<(PathBuf, Vec<u8>)>,
}

impl Outcome {
    /// Saves the base record now, for tests that applied the outcome themselves.
    #[cfg(test)]
    pub fn save_base(&self) -> Result<(), String> {
        match &self.base {
            Some((p, b)) => crate::vault::write_atomic(p, b).map_err(|e| e.to_string()),
            None => Ok(()),
        }
    }
}

fn b64() -> base64::engine::GeneralPurpose {
    base64::engine::general_purpose::STANDARD
}

/// Opens the remote folder: reads or writes its format, and returns the keys if encrypted.
fn open_format(store: &dyn Store, r: &Remote, create: bool) -> Result<Option<crypto::Keys>, String> {
    use base64::Engine;
    let found = store.get(FORMAT)?;
    let Some(bytes) = found else {
        if !create {
            return Ok(None);
        }
        let mut f = Format { version: 1, encrypted: r.encrypt, ..Default::default() };
        let keys = if r.encrypt {
            let salt = crypto::random(16);
            let k = crypto::derive(&r.pass, &salt, crypto::ITERATIONS);
            f.kdf = Some("pbkdf2-sha256".into());
            f.iterations = Some(crypto::ITERATIONS);
            f.salt = Some(b64().encode(&salt));
            f.check = Some(b64().encode(crypto::seal(&k, b"quire")));
            Some(k)
        } else {
            None
        };
        store.put(FORMAT, serde_json::to_vec_pretty(&f).unwrap_or_default().as_slice())?;
        return Ok(keys);
    };
    let f: Format = serde_json::from_slice(&bytes).map_err(|_| "The remote folder has a format file Quire cannot read.".to_string())?;
    match (f.encrypted, r.encrypt) {
        (true, false) => return Err("The remote notes are encrypted. Turn on Encrypt before upload and enter the passphrase.".into()),
        (false, true) => return Err("The remote notes are not encrypted. Turn off Encrypt before upload, or use a new remote folder.".into()),
        (false, false) => return Ok(None),
        (true, true) => {}
    }
    let salt = f.salt.as_deref().and_then(|s| b64().decode(s).ok()).ok_or("The remote format has no salt.")?;
    let k = crypto::derive(&r.pass, &salt, f.iterations.unwrap_or(crypto::ITERATIONS));
    let check = f.check.as_deref().and_then(|s| b64().decode(s).ok()).ok_or("The remote format has no check value.")?;
    match crypto::open(&k, &check) {
        Ok(v) if v == b"quire" => Ok(Some(k)),
        _ => Err("The passphrase does not match the one these notes were encrypted with.".into()),
    }
}

fn manifest_key(keys: &Option<crypto::Keys>) -> &'static str {
    if keys.is_some() { ".quire/manifest.bin" } else { ".quire/manifest.json" }
}

fn read_manifest(store: &dyn Store, keys: &Option<crypto::Keys>) -> Result<Manifest, String> {
    let Some(bytes) = store.get(manifest_key(keys))? else { return Ok(Manifest { version: 1, ..Default::default() }) };
    let plain = match keys {
        Some(k) => crypto::open(k, &bytes)?,
        None => bytes,
    };
    serde_json::from_slice(&plain).map_err(|_| "The remote manifest could not be read.".to_string())
}

fn write_manifest(store: &dyn Store, keys: &Option<crypto::Keys>, m: &Manifest) -> Result<(), String> {
    let json = serde_json::to_vec(m).unwrap_or_default();
    let body = match keys {
        Some(k) => crypto::seal(k, &json),
        None => json,
    };
    store.put(manifest_key(keys), &body)
}

/// Checks the settings against the store, as "Test connection" does.
pub fn test(r: &Remote, local_notes: usize, synced_before: bool) -> Result<String, String> {
    if r.encrypt && r.pass.chars().count() < 12 {
        return Err("The passphrase needs at least 12 characters.".into());
    }
    let store = open_store(r)?;
    let keys = open_format(store.as_ref(), r, false)?;
    let place = store.place();
    let files = match store.get(manifest_key(&keys))? {
        Some(_) => read_manifest(store.as_ref(), &keys)?.files,
        None => BTreeMap::new(),
    };
    let notes = files.keys().filter(|p| !md::is_image(p)).count();
    let images = files.len() - notes;
    let plural = |n: usize, one: &str, many: &str| format!("{n} {}", if n == 1 { one } else { many });
    let held = if images > 0 { format!("{} and {}", plural(notes, "note", "notes"), plural(images, "image", "images")) } else { plural(notes, "note", "notes") };
    let folder = if r.prefix.is_empty() { "The bucket".to_string() } else { format!("The remote folder {}", r.prefix.trim_end_matches('/')) };
    Ok(if !files.is_empty() && !synced_before {
        // A first sync with notes on both sides joins them. Say so, so a remote folder that
        // belongs to another notes folder is easy to see.
        format!("Connected to {place}. {folder} holds {held}, and this folder has {}. The first sync brings them together.", plural(local_notes, "note", "notes"))
    } else if !files.is_empty() {
        format!("Connected to {place}. It holds {held}.")
    } else if synced_before {
        format!("Connected to {place}. Everything is up to date.")
    } else {
        format!("Connected to {place}. {local_notes} {} will upload on the first sync.", if local_notes == 1 { "note" } else { "notes" })
    })
}

fn base_path(r: &Remote, root: &Path) -> Option<PathBuf> {
    crate::store::dirs().map(|d| d.config().join("sync").join(format!("{}.json", r.id(root))))
}

fn load_base(p: &Option<PathBuf>) -> Base {
    let Some(bytes) = p.as_ref().and_then(|p| std::fs::read(p).ok()) else { return Base::default() };
    // A record from before folders synced is a plain map of notes.
    serde_json::from_slice(&bytes)
        .ok()
        .or_else(|| serde_json::from_slice::<BTreeMap<String, String>>(&bytes).ok().map(|files| Base { files, ..Default::default() }))
        .unwrap_or_default()
}


pub fn device_name() -> String {
    // The kernel's name is always there on Linux. Apps from a launcher do not get $HOSTNAME.
    let read = |p: &str| std::fs::read_to_string(p).ok().map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
    let h = read("/proc/sys/kernel/hostname").or_else(|| read("/etc/hostname"));
    h.or_else(|| std::env::var("COMPUTERNAME").ok()).or_else(|| std::env::var("HOSTNAME").ok()).unwrap_or_else(|| "desktop".into())
}

pub fn conflict_path(p: &str, device: &str, taken: &BTreeSet<String>) -> String {
    let dir = md::dir(p);
    let file = p.rsplit('/').next().unwrap_or(p);
    let (stem, ext) = file.rsplit_once('.').unwrap_or((file, "md"));
    let dev = md::clean_name(device);
    let mut c = md::join(dir, &format!("{stem} (conflict {dev}).{ext}"));
    let mut n = 2;
    while taken.contains(&c) {
        c = md::join(dir, &format!("{stem} (conflict {dev} {n}).{ext}"));
        n += 1;
    }
    c
}

/// Lets `run_with` use one call style for a borrowed store.
struct Boxed<'a>(&'a dyn Store);

impl Boxed<'_> {
    fn as_mut(&mut self) -> &dyn Store {
        self.0
    }
    fn as_ref(&self) -> &dyn Store {
        self.0
    }
    fn place(&self) -> String {
        self.0.place()
    }
}

/// How many requests run at once.
const WORKERS: usize = 8;

/// `f` on every item, `WORKERS` at a time, with the results in the items' order. The first
/// error stops the rest.
fn parallel<T: Sync, R: Send>(items: &[T], f: impl Fn(&T) -> Result<R, String> + Sync) -> Result<Vec<R>, String> {
    if items.len() <= 1 {
        return items.iter().map(&f).collect();
    }
    let next = std::sync::atomic::AtomicUsize::new(0);
    let failed = std::sync::atomic::AtomicBool::new(false);
    let slots: Vec<std::sync::Mutex<Option<Result<R, String>>>> = items.iter().map(|_| std::sync::Mutex::new(None)).collect();
    std::thread::scope(|sc| {
        for _ in 0..WORKERS.min(items.len()) {
            sc.spawn(|| {
                loop {
                    let i = next.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    if i >= items.len() || failed.load(std::sync::atomic::Ordering::SeqCst) {
                        break;
                    }
                    let r = f(&items[i]);
                    if r.is_err() {
                        failed.store(true, std::sync::atomic::Ordering::SeqCst);
                    }
                    *slots[i].lock().unwrap_or_else(std::sync::PoisonError::into_inner) = Some(r);
                }
            });
        }
    });
    let mut out = Vec::with_capacity(items.len());
    for s in slots {
        match s.into_inner().unwrap_or_else(std::sync::PoisonError::into_inner) {
            Some(Ok(r)) => out.push(r),
            Some(Err(e)) => return Err(e),
            None => return Err("A sync step did not run.".into()),
        }
    }
    Ok(out)
}

enum Change {
    Set(String, Entry),
    Remove(String),
    AddFolder(String),
    RemoveFolder(String),
    AddPin(String),
    RemovePin(String),
}

/// Merges pins as folders merge: a pin added on one side is added, and a pin removed on one
/// side is removed. `remote` is None when the manifest comes from a version without pins: the
/// pins here stay. `base` is None before the first sync with pins: nothing is removed.
fn merge_pins(local: &BTreeSet<String>, remote: Option<&BTreeSet<String>>, base: Option<&BTreeSet<String>>) -> BTreeSet<String> {
    let Some(remote) = remote else { return local.clone() };
    let Some(base) = base else { return local | remote };
    local
        .union(remote)
        .filter(|p| match (local.contains(*p), remote.contains(*p)) {
            (true, true) => true,
            // On one side only: added there, or removed on the other side.
            _ => !base.contains(*p),
        })
        .cloned()
        .collect()
}

/// One sync of the folder at `root`, which must hold every saved note. `pins` are the pinned
/// notes here.
pub fn run(r: &Remote, root: &Path, device: &str, pins: &BTreeSet<String>) -> Result<Outcome, String> {
    if r.encrypt && r.pass.chars().count() < 12 {
        return Err("The passphrase needs at least 12 characters.".into());
    }
    let store = open_store(r)?;
    let bp = base_path(r, root);
    run_with_pins(store.as_ref(), r, root, device, &bp, pins)
}

#[cfg(test)]
fn run_with(store: &dyn Store, r: &Remote, root: &Path, device: &str, bp: &Option<PathBuf>) -> Result<Outcome, String> {
    run_with_pins(store, r, root, device, bp, &BTreeSet::new())
}

fn run_with_pins(store: &dyn Store, r: &Remote, root: &Path, device: &str, bp: &Option<PathBuf>, pins: &BTreeSet<String>) -> Result<Outcome, String> {
    let mut store = Boxed(store);
    let keys = open_format(store.as_mut(), r, true)?;
    let manifest = read_manifest(store.as_mut(), &keys)?;
    let loaded = crate::vault::load(root).map_err(|e| format!("Could not read the folder: {e}"))?;
    let local_dirs = loaded.folders.into_iter().collect::<BTreeSet<String>>();
    let bp = bp.clone();
    let Base { files: base, folders: base_dirs, stamps: old_stamps, pins: base_pins } = load_base(&bp);
    let hash = |t: &[u8]| sign::sha256_hex(t);
    // Notes by their text, images by their bytes. An image keeps its hash while its size and
    // file time stay the same.
    let mut local: BTreeMap<String, (Local, String)> = loaded.files.into_iter().map(|(p, t)| {
        let h = hash(t.as_bytes());
        (p, (Local::Note(t), h))
    }).collect();
    let mut stamps = BTreeMap::new();
    for rel in crate::images::list(root) {
        let path = root.join(&rel);
        let Some((size, t)) = stamp(&path) else { continue };
        if size > crate::images::MAX_BYTES {
            continue;
        }
        let h = match old_stamps.get(&rel) {
            Some((s0, t0, h)) if *s0 == size && *t0 == t => h.clone(),
            _ => match std::fs::read(&path) {
                Ok(b) => hash(&b),
                Err(_) => continue,
            },
        };
        stamps.insert(rel.clone(), (size, t, h.clone()));
        local.insert(rel, (Local::Image(path), h));
    }
    let mut out = Outcome { place: store.place(), root: root.to_path_buf(), ..Default::default() };
    let mut new_base = BTreeMap::new();
    let mut changes = Vec::new();
    let now = sign::now_secs() * 1000;
    let mut paths: BTreeSet<String> = local.keys().cloned().collect();
    paths.extend(manifest.files.keys().cloned());
    paths.extend(base.keys().cloned());
    let mut taken: BTreeSet<String> = paths.clone();

    // 1. Decide what each path needs, without the network.
    enum Step<'a> {
        Up(&'a str),
        Down(&'a str, &'a Entry, Option<String>),
        /// Both sides changed: the remote version comes down under a new name, and goes up
        /// under it too, while this device's version goes up at the path.
        Both(&'a str, &'a Entry, String),
        DeleteRemote(&'a str, &'a Entry),
    }
    let mut steps = Vec::new();
    for p in &paths {
        // A path that leaves the notes folder, or is neither a note nor an image, is skipped.
        if p.split('/').any(|s| s == ".." || s.starts_with('.') || s.is_empty()) || !(p.to_lowercase().ends_with(".md") || md::is_image(p)) {
            continue;
        }
        let l = local.get(p);
        let rem = manifest.files.get(p);
        let b = base.get(p);
        match (l, rem) {
            (Some((_, lh)), Some(e)) if *lh == e.hash => {
                new_base.insert(p.clone(), lh.clone());
            }
            (Some((_, lh)), None) if b == Some(lh) => {
                // Another device deleted it and this one did not change it.
                if md::is_image(p) {
                    out.image_removes.push((p.clone(), lh.clone()));
                } else {
                    out.removes.push((p.clone(), lh.clone()));
                }
            }
            (Some(_), None) => steps.push(Step::Up(p)),
            (None, Some(e)) if b == Some(&e.hash) => steps.push(Step::DeleteRemote(p, e)),
            (None, Some(e)) => steps.push(Step::Down(p, e, None)),
            (None, None) => {}
            (Some(_), Some(e)) if b == Some(&e.hash) => steps.push(Step::Up(p)),
            (Some((_, lh)), Some(e)) if b == Some(lh) => steps.push(Step::Down(p, e, Some(lh.clone()))),
            (Some(_), Some(e)) => {
                let c = conflict_path(p, &e.device, &taken);
                taken.insert(c.clone());
                steps.push(Step::Both(p, e, c));
            }
        }
    }

    // 2. Downloads, 8 at a time.
    let wanted: Vec<&Entry> = steps.iter().filter_map(|s| match s {
        Step::Down(_, e, _) | Step::Both(_, e, _) => Some(*e),
        _ => None,
    }).collect();
    let store_ref: &dyn Store = store.as_ref();
    let got = parallel(&wanted, |e| -> Result<Option<Vec<u8>>, String> {
        let Some(bytes) = store_ref.get(&e.object)? else { return Ok(None) };
        Ok(Some(match &keys {
            Some(k) => crypto::open(k, &bytes)?,
            None => bytes,
        }))
    })?;
    let mut got: BTreeMap<String, Vec<u8>> = wanted.iter().zip(got).filter_map(|(e, b)| b.map(|b| (e.object.clone(), b))).collect();

    // What comes down: a note's text, or an image's bytes.
    let give = |out: &mut Outcome, p: String, data: Vec<u8>, seen: Option<String>, device: String| {
        if md::is_image(&p) {
            out.image_pulls.push((p, data, seen, device));
        } else {
            let text = String::from_utf8_lossy(&data).replace("\r\n", "\n");
            out.pulls.push(Pull { path: p, text, seen, device });
        }
    };
    // 3. Lay out the uploads and the outcome.
    let mut ups: Vec<(String, Vec<u8>)> = Vec::new();
    let mut gone: Vec<(&str, &Entry)> = Vec::new();
    for st in steps {
        match st {
            Step::Up(p) => ups.push((p.to_string(), body(&local[p].0)?.into_owned())),
            Step::Down(p, e, seen) => {
                if let Some(data) = got.remove(&e.object) {
                    new_base.insert(p.to_string(), e.hash.clone());
                    give(&mut out, p.to_string(), data, seen, e.device.clone());
                }
            }
            Step::Both(p, e, c) => {
                if let Some(data) = got.remove(&e.object) {
                    out.conflicts.push(c.clone());
                    // The copy goes up at once, so every device sees the conflict.
                    ups.push((c.clone(), data.clone()));
                    give(&mut out, c, data, None, e.device.clone());
                }
                ups.push((p.to_string(), body(&local[p].0)?.into_owned()));
            }
            Step::DeleteRemote(p, e) => gone.push((p, e)),
        }
    }

    // 4. Uploads and deletes, 8 at a time.
    let sent = parallel(&ups, |(p, data)| -> Result<Entry, String> {
        let object = match &keys {
            Some(k) => crypto::object_name(k, p),
            None => p.clone(),
        };
        let sealed;
        let payload: &[u8] = match &keys {
            Some(k) => {
                sealed = crypto::seal(k, data);
                &sealed
            }
            None => data,
        };
        store_ref.put(&object, payload)?;
        Ok(Entry { hash: sign::sha256_hex(data), object, time: now, device: device.to_string() })
    })?;
    for ((p, _), e) in ups.iter().zip(sent) {
        new_base.insert(p.clone(), e.hash.clone());
        changes.push(Change::Set(p.clone(), e));
        out.uploaded += 1;
    }
    parallel(&gone, |(_, e)| store_ref.delete(&e.object))?;
    for (p, _) in &gone {
        changes.push(Change::Remove(p.to_string()));
        out.deleted_remote += 1;
    }

    // Folders, after the notes: a folder stays while a note is left in it.
    let kept: BTreeSet<String> = local
        .keys()
        .filter(|p| !out.removes.iter().chain(&out.image_removes).any(|(r, _)| r == *p))
        .cloned()
        .chain(out.pulls.iter().map(|pl| pl.path.clone()))
        .chain(out.image_pulls.iter().map(|pl| pl.0.clone()))
        .chain(changes.iter().filter_map(|c| if let Change::Set(p, _) = c { Some(p.clone()) } else { None }))
        .collect();
    let holds_notes = |f: &str| {
        let pre = format!("{f}/");
        kept.iter().any(|p| p.starts_with(&pre))
    };
    let mut new_dirs = BTreeSet::new();
    let mut all_dirs: BTreeSet<String> = local_dirs.clone();
    all_dirs.extend(manifest.folders.iter().cloned());
    all_dirs.extend(base_dirs.iter().cloned());
    for f in &all_dirs {
        let (here, there, before) = (local_dirs.contains(f), manifest.folders.contains(f), base_dirs.contains(f));
        match (here, there) {
            (true, true) => {
                new_dirs.insert(f.clone());
            }
            (true, false) if before && !holds_notes(f) => out.rmdirs.push(f.clone()),
            (true, false) => {
                changes.push(Change::AddFolder(f.clone()));
                new_dirs.insert(f.clone());
            }
            (false, true) if before && !holds_notes(f) => changes.push(Change::RemoveFolder(f.clone())),
            (false, true) => {
                out.mkdirs.push(f.clone());
                new_dirs.insert(f.clone());
            }
            (false, false) => {}
        }
    }
    // The deepest folder goes first, so a parent is empty when its turn comes.
    out.rmdirs.sort_by_key(|f| std::cmp::Reverse(f.matches('/').count()));

    // Pins, after the notes: a pin for a note that is gone goes too.
    let mut new_pins = merge_pins(pins, manifest.pins.as_ref(), base_pins.as_ref());
    new_pins.retain(|p| kept.contains(p));
    let there = manifest.pins.clone().unwrap_or_default();
    // A manifest without pins gets them only when there is a pin to write.
    if manifest.pins.is_some() || !new_pins.is_empty() {
        changes.extend(new_pins.difference(&there).map(|p| Change::AddPin(p.clone())));
        changes.extend(there.difference(&new_pins).map(|p| Change::RemovePin(p.clone())));
    }

    if !changes.is_empty() {
        // Read the manifest again just before writing, so a device that synced meanwhile keeps
        // its entries, then lay this sync's changes over it.
        let mut fresh = read_manifest(store.as_mut(), &keys)?;
        fresh.version = 1;
        for c in changes {
            match c {
                Change::Set(p, e) => {
                    fresh.files.insert(p, e);
                }
                Change::Remove(p) => {
                    fresh.files.remove(&p);
                }
                Change::AddFolder(f) => {
                    fresh.folders.insert(f);
                }
                Change::RemoveFolder(f) => {
                    fresh.folders.remove(&f);
                }
                Change::AddPin(p) => {
                    fresh.pins.get_or_insert_with(BTreeSet::new).insert(p);
                }
                Change::RemovePin(p) => {
                    if let Some(s) = &mut fresh.pins {
                        s.remove(&p);
                    }
                }
            }
        }
        write_manifest(store.as_mut(), &keys, &fresh)?;
    }
    stamps.retain(|p, _| new_base.contains_key(p));
    out.pins = Some(new_pins.clone());
    out.base = bp.map(|p| (p, serde_json::to_vec(&Base { files: new_base, folders: new_dirs, stamps, pins: Some(new_pins) }).unwrap_or_default()));
    Ok(out)
}

#[cfg(test)]
mod tests;
