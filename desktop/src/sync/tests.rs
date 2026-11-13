use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex};

use super::*;

/// A bucket in memory, shared by the devices of a test.
#[derive(Clone, Default)]
struct Mem(Arc<Mutex<BTreeMap<String, Vec<u8>>>>);

impl Store for Mem {
    fn get(&self, key: &str) -> Result<Option<Vec<u8>>, String> {
        Ok(self.0.lock().unwrap().get(key).cloned())
    }
    fn put(&self, key: &str, body: &[u8]) -> Result<(), String> {
        self.0.lock().unwrap().insert(key.to_string(), body.to_vec());
        Ok(())
    }
    fn delete(&self, key: &str) -> Result<(), String> {
        self.0.lock().unwrap().remove(key);
        Ok(())
    }
    fn place(&self) -> String {
        "mem".into()
    }
}

struct Device {
    root: PathBuf,
    base: Option<PathBuf>,
    name: &'static str,
}

impl Device {
    fn new(tag: &str, name: &'static str) -> Device {
        let root = std::env::temp_dir().join(format!("quire-sync-{}-{tag}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        Device { base: Some(root.with_extension("base.json")), root, name }
    }
    fn write(&self, p: &str, t: &str) {
        let f = self.root.join(p);
        std::fs::create_dir_all(f.parent().unwrap()).unwrap();
        std::fs::write(f, t).unwrap();
    }
    fn read(&self, p: &str) -> Option<String> {
        std::fs::read_to_string(self.root.join(p)).ok()
    }
    /// Syncs, then applies the outcome as the app does.
    fn sync(&self, bucket: &mut Mem, r: &Remote) -> Outcome {
        let out = run_with(bucket, r, &self.root, self.name, &self.base).unwrap();
        for pl in &out.pulls {
            self.write(&pl.path, &pl.text);
        }
        for (p, _) in out.removes.iter().chain(&out.image_removes) {
            let _ = std::fs::remove_file(self.root.join(p));
        }
        for (p, bytes, _, _) in &out.image_pulls {
            let f = self.root.join(p);
            std::fs::create_dir_all(f.parent().unwrap()).unwrap();
            std::fs::write(f, bytes).unwrap();
        }
        for f in &out.mkdirs {
            std::fs::create_dir_all(self.root.join(f)).unwrap();
        }
        for f in &out.rmdirs {
            let _ = std::fs::remove_dir_all(self.root.join(f));
        }
        out.save_base().unwrap();
        out
    }
}

fn remote(encrypt: bool) -> Remote {
    Remote { provider: "mem".into(), cfg: BTreeMap::new(), prefix: String::new(), encrypt, pass: "a long enough passphrase".into() }
}

fn two_devices(encrypt: bool, tag: &str) {
    let mut bucket = Mem::default();
    let r = remote(encrypt);
    let (laptop, phone) = (Device::new(tag, "laptop"), Device::new(tag, "phone"));
    laptop.write("Ideas.md", "# Ideas");
    laptop.write("Drafts/Plan.md", "# Plan");
    assert_eq!(laptop.sync(&mut bucket, &r).uploaded, 2);
    // A second device gets both notes.
    let out = phone.sync(&mut bucket, &r);
    assert_eq!(out.pulls.len(), 2);
    assert_eq!(phone.read("Drafts/Plan.md").as_deref(), Some("# Plan"));
    // An edit on the phone reaches the laptop.
    phone.write("Ideas.md", "# Ideas\n\nfrom the phone");
    phone.sync(&mut bucket, &r);
    laptop.sync(&mut bucket, &r);
    assert_eq!(laptop.read("Ideas.md").as_deref(), Some("# Ideas\n\nfrom the phone"));
    // A delete on the laptop removes the note on the phone.
    std::fs::remove_file(laptop.root.join("Drafts/Plan.md")).unwrap();
    assert_eq!(laptop.sync(&mut bucket, &r).deleted_remote, 1);
    assert_eq!(phone.sync(&mut bucket, &r).removes, vec![("Drafts/Plan.md".to_string(), sign::sha256_hex(b"# Plan"))]);
    assert!(phone.read("Drafts/Plan.md").is_none());
    // Both change the same note: each keeps its own text and gets the other's beside it.
    laptop.write("Ideas.md", "laptop text");
    phone.write("Ideas.md", "phone text");
    laptop.sync(&mut bucket, &r);
    let out = phone.sync(&mut bucket, &r);
    assert_eq!(out.conflicts, vec!["Ideas (conflict laptop).md".to_string()]);
    assert_eq!(phone.read("Ideas.md").as_deref(), Some("phone text"));
    assert_eq!(phone.read("Ideas (conflict laptop).md").as_deref(), Some("laptop text"));
    laptop.sync(&mut bucket, &r);
    assert_eq!(laptop.read("Ideas.md").as_deref(), Some("phone text"));
    assert_eq!(laptop.read("Ideas (conflict laptop).md").as_deref(), Some("laptop text"));
    // An empty folder reaches the other device, and so does its delete. A folder with a note
    // in it stays, and goes back up.
    std::fs::create_dir_all(laptop.root.join("Archive/2025")).unwrap();
    laptop.sync(&mut bucket, &r);
    assert_eq!(phone.sync(&mut bucket, &r).mkdirs, vec!["Archive".to_string(), "Archive/2025".to_string()]);
    assert!(phone.root.join("Archive/2025").is_dir());
    std::fs::remove_dir_all(laptop.root.join("Archive")).unwrap();
    laptop.sync(&mut bucket, &r);
    assert_eq!(phone.sync(&mut bucket, &r).rmdirs, vec!["Archive/2025".to_string(), "Archive".to_string()]);
    assert!(!phone.root.join("Archive").exists());
    std::fs::remove_dir_all(phone.root.join("Drafts")).unwrap();
    laptop.write("Drafts/New.md", "# New");
    phone.sync(&mut bucket, &r);
    let out = laptop.sync(&mut bucket, &r);
    assert!(out.rmdirs.is_empty(), "a folder with a new note stays: {:?}", out.rmdirs);
    let out = phone.sync(&mut bucket, &r);
    assert_eq!(out.pulls.iter().map(|p| p.path.as_str()).collect::<Vec<_>>(), vec!["Drafts/New.md"]);
    assert!(phone.root.join("Drafts/New.md").is_file());
    // A sync with nothing new changes nothing.
    let quiet = phone.sync(&mut bucket, &r);
    assert_eq!((quiet.uploaded, quiet.pulls.len(), quiet.removes.len(), quiet.mkdirs.len(), quiet.rmdirs.len()), (0, 0, 0, 0, 0));
    if encrypt {
        let b = bucket.0.lock().unwrap();
        assert!(b.keys().all(|k| !k.contains("Ideas")), "object names hide the paths: {:?}", b.keys().collect::<Vec<_>>());
        assert!(b.values().all(|v| !String::from_utf8_lossy(v).contains("phone text")), "text is encrypted");
    }
}

#[test]
fn two_devices_stay_in_step() {
    two_devices(false, "plain");
}

#[test]
fn two_devices_stay_in_step_encrypted() {
    two_devices(true, "enc");
}

#[test]
fn a_wrong_passphrase_is_refused() {
    let mut bucket = Mem::default();
    let d = Device::new("pass", "laptop");
    d.write("A.md", "a");
    d.sync(&mut bucket, &remote(true));
    let mut other = remote(true);
    other.pass = "a different passphrase".into();
    let e = Device::new("pass2", "phone");
    let err = run_with(&mut bucket, &other, &e.root, "phone", &e.base).unwrap_err();
    assert!(err.contains("passphrase"), "{err}");
    let plain = run_with(&mut bucket, &remote(false), &e.root, "phone", &e.base).unwrap_err();
    assert!(plain.contains("encrypted"), "{plain}");
}

type Objects = Arc<Mutex<BTreeMap<String, Vec<u8>>>>;

/// A small S3 server for tests. It checks the signature of each request the way S3 does, from
/// the path and headers it received, then keeps the objects of one bucket in memory.
fn s3_server(secret: &'static str) -> (String, Objects) {
    s3_server_failing(secret, 0)
}

/// `s3_server` that answers 504 to every `fail_every`-th request (0 never, 1 always), as a
/// gateway in front of a busy store does.
fn s3_server_failing(secret: &'static str, fail_every: usize) -> (String, Objects) {
    let count = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    use std::io::{BufRead, BufReader, Read, Write};
    let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", l.local_addr().unwrap());
    let objects: Objects = Arc::default();
    let o2 = objects.clone();
    std::thread::spawn(move || {
        for stream in l.incoming().flatten() {
            let objects = o2.clone();
            let count = count.clone();
            std::thread::spawn(move || {
                let mut w = stream.try_clone().unwrap();
                let mut r = BufReader::new(stream);
                loop {
                    let mut line = String::new();
                    if r.read_line(&mut line).unwrap_or(0) == 0 {
                        return;
                    }
                    let mut parts = line.split_whitespace();
                    let (method, path) = (parts.next().unwrap_or("").to_string(), parts.next().unwrap_or("").to_string());
                    let mut headers = BTreeMap::new();
                    loop {
                        let mut h = String::new();
                        r.read_line(&mut h).unwrap();
                        let h = h.trim_end();
                        if h.is_empty() {
                            break;
                        }
                        let (k, v) = h.split_once(':').unwrap();
                        headers.insert(k.trim().to_lowercase(), v.trim().to_string());
                    }
                    let len: usize = headers.get("content-length").and_then(|v| v.parse().ok()).unwrap_or(0);
                    let mut body = vec![0u8; len];
                    r.read_exact(&mut body).unwrap();
                    let h = |k: &str| headers.get(k).cloned().unwrap_or_default();
                    let auth = h("authorization");
                    let date = h("x-amz-date");
                    let region = auth.split("Credential=").nth(1).and_then(|c| c.split('/').nth(2)).unwrap_or("").to_string();
                    let signed = [("host", h("host")), ("x-amz-content-sha256", h("x-amz-content-sha256")), ("x-amz-date", date.clone())];
                    let s = sign::SigV4 { access: "test", secret, region: &region, service: "s3" };
                    let want = s.authorization(&method, &path, "", &signed, &h("x-amz-content-sha256"), &date, &date[..8.min(date.len())]);
                    let n = count.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
                    let (code, out): (u16, Vec<u8>) = if fail_every > 0 && n % fail_every == 0 {
                        (504, b"<html><body><h1>504 Gateway Time-out</h1></body></html>".to_vec())
                    } else if auth != want || h("x-amz-content-sha256") != sign::sha256_hex(&body) {
                        (403, b"<Error><Code>SignatureDoesNotMatch</Code></Error>".to_vec())
                    } else if !path.starts_with("/notes/") {
                        (404, b"<Error><Code>NoSuchBucket</Code></Error>".to_vec())
                    } else {
                        let mut o = objects.lock().unwrap();
                        match method.as_str() {
                            "GET" => o.get(&path).map_or((404, b"<Error><Code>NoSuchKey</Code></Error>".to_vec()), |b| (200, b.clone())),
                            "PUT" => {
                                o.insert(path.clone(), body);
                                (200, Vec::new())
                            }
                            "DELETE" => {
                                o.remove(&path);
                                (204, Vec::new())
                            }
                            _ => (405, Vec::new()),
                        }
                    };
                    let head = format!("HTTP/1.1 {code} X\r\nContent-Length: {}\r\n\r\n", out.len());
                    if w.write_all(head.as_bytes()).and_then(|_| w.write_all(&out)).is_err() {
                        return;
                    }
                }
            });
        }
    });
    (url, objects)
}

fn custom(endpoint: &str, bucket: &str, secret: &str, encrypt: bool) -> Remote {
    let mut cfg = BTreeMap::new();
    for (k, v) in [("endpoint", endpoint), ("bucket", bucket), ("accessKey", "test"), ("secretKey", secret), ("addressing", "path"), ("region", "eu-north-1")] {
        cfg.insert(k.to_string(), v.to_string());
    }
    Remote { provider: "custom".into(), cfg, prefix: clean_prefix("quire/Field Notes"), encrypt, pass: "a long enough passphrase".into() }
}

#[test]
fn s3_requests_are_signed_and_stored() {
    let (url, objects) = s3_server("testsecret");
    for encrypt in [false, true] {
        let mut r = custom(&url, "notes", "testsecret", encrypt);
        if encrypt {
            r.prefix = clean_prefix("quire/Secret Notes");
        }
        let tag = format!("http-{encrypt}");
        let (a, b) = (Device::new(&tag, "laptop"), Device::new(&tag, "phone"));
        a.write("Notater/Blåbær og fløte.md", "# Blåbær");
        a.write("Ideas (draft) #2.md", "# Ideas");
        let store = open_store(&r).unwrap();
        let first = run_with(store.as_ref(), &r, &a.root, "laptop", &a.base).unwrap();
        assert_eq!(first.uploaded, 2);
        first.save_base().unwrap();
        let out = run_with(store.as_ref(), &r, &b.root, "phone", &b.base).unwrap();
        assert_eq!(out.pulls.len(), 2);
        assert!(test(&r, 2, true).unwrap().starts_with("Connected to notes. It holds 2 notes"), "{:?}", test(&r, 2, true));
        let first = test(&r, 5, false).unwrap();
        assert!(first.contains("holds 2 notes, and this folder has 5 notes. The first sync brings them together."), "{first}");
        std::fs::remove_file(a.root.join("Ideas (draft) #2.md")).unwrap();
        assert_eq!(run_with(store.as_ref(), &r, &a.root, "laptop", &a.base).unwrap().deleted_remote, 1);
    }
    let keys: Vec<String> = objects.lock().unwrap().keys().cloned().collect();
    assert!(keys.contains(&"/notes/quire/Field%20Notes/Notater/Bl%C3%A5b%C3%A6r%20og%20fl%C3%B8te.md".to_string()), "{keys:?}");
    assert!(keys.contains(&"/notes/quire/Field%20Notes/.quire/manifest.json".to_string()), "{keys:?}");
    assert!(keys.contains(&"/notes/quire/Secret%20Notes/.quire/manifest.bin".to_string()), "{keys:?}");
    assert!(!keys.iter().any(|k| k.contains("Ideas")), "the deleted note is gone: {keys:?}");
    assert!(!keys.iter().any(|k| k.starts_with("/notes/quire/Secret%20Notes/Notater")), "encrypted names hide paths: {keys:?}");
}

#[test]
fn s3_errors_say_what_to_fix() {
    let (url, _) = s3_server("testsecret");
    let err = test(&custom(&url, "notes", "wrong", false), 0, false).unwrap_err();
    assert_eq!(err, "Access denied (403). Check the secret access key.");
    let err = test(&custom(&url, "other", "testsecret", false), 0, false).unwrap_err();
    assert!(err.starts_with("The bucket does not exist"), "{err}");
    let err = test(&custom("http://127.0.0.1:9", "notes", "testsecret", false), 0, false).unwrap_err();
    assert_eq!(err, "Could not reach 127.0.0.1:9. The connection was refused or timed out.");
}

/// Two app windows, each with its own notes folder, sync through the S3 test server: the first
/// sync uploads, the second device gets the note, an edit comes back, and an edit made during a
/// sync is kept beside the remote text.
#[test]
fn two_apps_sync_through_s3() {
    use rune::{Env, Runtime};
    let _env = crate::scroll_tests::ENV.lock().unwrap_or_else(|e| e.into_inner());
    let dir = std::env::temp_dir().join(format!("quire-appsync-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    // SAFETY: the ENV lock keeps other tests that read these variables out.
    unsafe {
        std::env::set_var("HOME", &dir);
        std::env::set_var("XDG_CONFIG_HOME", dir.join("cfg"));
        std::env::set_var("XDG_DATA_HOME", dir.join("data"));
    }
    let (url, _) = s3_server("testsecret");
    let open = |name: &str, notes: &[(&str, &str)]| {
        let root = dir.join(name);
        std::fs::create_dir_all(&root).unwrap();
        for (p, t) in notes {
            std::fs::write(root.join(p), t).unwrap();
        }
        let mut rt = Runtime::new(crate::app::init(), crate::ui::view, (1280.0, 800.0), Env { system_dark: false, scale: 1.0 });
        rt.poll = Some(|s: &mut crate::app::App, t, tx| s.poll(t, tx));
        let remote = custom(&url, "notes", "testsecret", false);
        rt.update(move |s, tx| {
            let key = root.to_string_lossy().into_owned();
            s.open_vault(root, tx);
            let sy = &mut s.persist.settings.sync;
            sy.provider = "custom".into();
            sy.cfg.insert("custom".into(), remote.cfg.clone());
            // Two notes folders with other names share one remote folder only when told to.
            sy.folders.insert(key, "quire/apps/".into());
            sy.when = "manual".into();
            sy.connected = true;
        });
        rt
    };
    // Runs frames until the sync that `start` begins has finished.
    let sync = |rt: &mut Runtime<crate::app::App>| {
        rt.update(|s, _| s.sync_now());
        for _ in 0..1000 {
            rt.frame(1.0 / 60.0);
            let mut busy = true;
            rt.update_if(|s, _| {
                busy = s.sync.busy();
                false
            });
            if !busy {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        let mut err = None;
        rt.update_if(|s, _| {
            err = s.sync.error.clone();
            s.writer.wait(std::time::Duration::from_secs(5));
            false
        });
        assert_eq!(err, None);
    };
    let text = |rt: &mut Runtime<crate::app::App>, p: &str| {
        let mut out = None;
        let p = p.to_string();
        rt.update_if(|s, _| {
            out = s.files().get(&p).cloned();
            false
        });
        out
    };
    let mut laptop = open("laptop", &[("Plan.md", "# Plan")]);
    sync(&mut laptop);
    let mut phone = open("phone", &[]);
    sync(&mut phone);
    assert_eq!(text(&mut phone, "Plan.md").as_deref(), Some("# Plan"));
    assert_eq!(std::fs::read_to_string(dir.join("phone/Plan.md")).unwrap(), "# Plan");

    // An edit on the phone, saved the way the editor saves, reaches the laptop.
    phone.update(|s, _| {
        s.vault.as_mut().unwrap().files.insert("Plan.md".into(), "# Plan\n\nfrom the phone".into());
        s.dirty.insert("Plan.md".into());
    });
    sync(&mut phone);
    sync(&mut laptop);
    assert_eq!(text(&mut laptop, "Plan.md").as_deref(), Some("# Plan\n\nfrom the phone"));

    // The laptop gets a newer remote text, but the note changed here after the sync read it:
    // the note keeps the local text and the remote text comes in as a conflict copy.
    laptop.update(|s, _| {
        s.vault.as_mut().unwrap().files.insert("Plan.md".into(), "typed during the sync".into());
        let out = super::Outcome {
            pulls: vec![super::Pull { path: "Plan.md".into(), text: "remote".into(), seen: Some(sign::sha256_hex(b"# Plan\n\nfrom the phone")), device: "phone".into() }],
            ..Default::default()
        };
        s.apply_sync(out);
    });
    assert_eq!(text(&mut laptop, "Plan.md").as_deref(), Some("typed during the sync"));
    assert_eq!(text(&mut laptop, "Plan (conflict phone).md").as_deref(), Some("remote"));
    drop((laptop, phone));
    let _ = std::fs::remove_dir_all(&dir);
}

/// Images sync as files next to the notes: they go up and come down byte for byte, their
/// edits and deletes carry over, and a change on both sides keeps both images.
#[test]
fn images_sync_with_the_notes() {
    for encrypt in [false, true] {
        let mut bucket = Mem::default();
        let r = remote(encrypt);
        let tag = format!("img-{encrypt}");
        let (laptop, phone) = (Device::new(&tag, "laptop"), Device::new(&tag, "phone"));
        let png: Vec<u8> = (0..=255u8).cycle().take(5000).collect();
        std::fs::create_dir_all(laptop.root.join("attachments")).unwrap();
        std::fs::write(laptop.root.join("attachments/cat.png"), &png).unwrap();
        std::fs::write(laptop.root.join("attachments/notes.txt"), "not synced").unwrap();
        laptop.write("Pics.md", "![cat](attachments/cat.png)");
        assert_eq!(laptop.sync(&mut bucket, &r).uploaded, 2, "the note and the image, not the text file");
        let out = phone.sync(&mut bucket, &r);
        assert_eq!(out.image_pulls.len(), 1);
        assert_eq!(std::fs::read(phone.root.join("attachments/cat.png")).unwrap(), png);
        // A quiet sync reads no image again and changes nothing.
        let quiet = laptop.sync(&mut bucket, &r);
        assert_eq!((quiet.uploaded, quiet.image_pulls.len()), (0, 0));
        // An edit on the phone reaches the laptop.
        std::thread::sleep(std::time::Duration::from_millis(20));
        std::fs::write(phone.root.join("attachments/cat.png"), b"new pixels").unwrap();
        phone.sync(&mut bucket, &r);
        laptop.sync(&mut bucket, &r);
        assert_eq!(std::fs::read(laptop.root.join("attachments/cat.png")).unwrap(), b"new pixels");
        // Both change it: each keeps its own, and the other's comes in beside it.
        std::thread::sleep(std::time::Duration::from_millis(20));
        std::fs::write(laptop.root.join("attachments/cat.png"), b"laptop pixels").unwrap();
        std::fs::write(phone.root.join("attachments/cat.png"), b"phone pixels").unwrap();
        laptop.sync(&mut bucket, &r);
        let out = phone.sync(&mut bucket, &r);
        assert_eq!(out.conflicts, vec!["attachments/cat (conflict laptop).png".to_string()]);
        assert_eq!(std::fs::read(phone.root.join("attachments/cat (conflict laptop).png")).unwrap(), b"laptop pixels");
        // The laptop gets the copy too. Then a delete on the phone removes it on the laptop.
        laptop.sync(&mut bucket, &r);
        assert_eq!(std::fs::read(laptop.root.join("attachments/cat (conflict laptop).png")).unwrap(), b"laptop pixels");
        std::fs::remove_file(phone.root.join("attachments/cat (conflict laptop).png")).unwrap();
        assert_eq!(phone.sync(&mut bucket, &r).deleted_remote, 1);
        assert_eq!(laptop.sync(&mut bucket, &r).image_removes.len(), 1);
        assert!(!laptop.root.join("attachments/cat (conflict laptop).png").exists());
        if encrypt {
            let b = bucket.0.lock().unwrap();
            assert!(b.keys().all(|k| !k.contains("cat")), "image names are hidden: {:?}", b.keys().collect::<Vec<_>>());
        }
    }
}

/// The engine leaves the base record to the caller, so a note that never reached the disk is
/// not taken as in step: the next sync downloads it again instead of uploading the old text.
#[test]
fn an_unsaved_base_loses_no_remote_edit() {
    let mut bucket = Mem::default();
    let r = remote(false);
    let (laptop, phone) = (Device::new("unsaved", "laptop"), Device::new("unsaved", "phone"));
    laptop.write("A.md", "one");
    laptop.sync(&mut bucket, &r);
    phone.sync(&mut bucket, &r);
    phone.write("A.md", "two");
    phone.sync(&mut bucket, &r);
    // The laptop gets "two", but stops before it writes the note or saves the base.
    let lost = run_with(&mut bucket, &r, &laptop.root, "laptop", &laptop.base).unwrap();
    assert_eq!(lost.pulls.len(), 1);
    drop(lost);
    let again = laptop.sync(&mut bucket, &r);
    assert_eq!(again.uploaded, 0, "the old text does not go up");
    assert_eq!(laptop.read("A.md").as_deref(), Some("two"));
}

/// A read-only check against the store in a real settings file, with timings:
/// `QUIRE_LIVE_STATE=~/.config/quire/state.json cargo test live_store_check -- --ignored --nocapture`.
#[test]
#[ignore]
fn live_store_check() {
    let Ok(path) = std::env::var("QUIRE_LIVE_STATE") else { return };
    let state: serde_json::Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    let sy = &state["settings"]["sync"];
    let provider = sy["provider"].as_str().unwrap_or("none").to_string();
    let cfg: BTreeMap<String, String> = sy["cfg"][&provider].as_object().unwrap().iter().map(|(k, v)| (k.clone(), v.as_str().unwrap_or("").to_string())).collect();
    let prefix = clean_prefix(cfg.get("prefix").map_or("quire/notes/", |s| s.as_str()));
    let r = Remote { provider, cfg, prefix, encrypt: sy["encrypt"].as_bool().unwrap_or(false), pass: sy["pass"].as_str().unwrap_or("").to_string() };
    let store = open_store(&r).unwrap();
    for key in [".quire/format.json", ".quire/manifest.json", ".quire/manifest.json", "missing-object.md"] {
        let t = std::time::Instant::now();
        let res = store.get(key);
        println!("GET {key}: {} in {:.2} s", match &res { Ok(Some(b)) => format!("{} bytes", b.len()), Ok(None) => "not found".into(), Err(e) => format!("error: {e}") }, t.elapsed().as_secs_f64());
    }
    let t = std::time::Instant::now();
    let res = store.put(".quire/probe.txt", b"probe");
    println!("PUT probe: {:?} in {:.2} s", res, t.elapsed().as_secs_f64());
    let t = std::time::Instant::now();
    println!("DELETE probe: {:?} in {:.2} s", store.delete(".quire/probe.txt"), t.elapsed().as_secs_f64());
}

/// Prints a summary of the remote manifest in a real settings file: counts, devices and
/// conflict copies. `QUIRE_LIVE_STATE=… cargo test live_manifest_summary -- --ignored --nocapture`.
#[test]
#[ignore]
fn live_manifest_summary() {
    let Ok(path) = std::env::var("QUIRE_LIVE_STATE") else { return };
    let state: serde_json::Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    let sy = &state["settings"]["sync"];
    let provider = sy["provider"].as_str().unwrap_or("none").to_string();
    let cfg: BTreeMap<String, String> = sy["cfg"][&provider].as_object().unwrap().iter().map(|(k, v)| (k.clone(), v.as_str().unwrap_or("").to_string())).collect();
    let prefix = clean_prefix(cfg.get("prefix").map_or("quire/notes/", |s| s.as_str()));
    let r = Remote { provider, cfg, prefix, encrypt: sy["encrypt"].as_bool().unwrap_or(false), pass: sy["pass"].as_str().unwrap_or("").to_string() };
    let store = open_store(&r).unwrap();
    let keys = open_format(store.as_ref(), &r, false).unwrap();
    let m = read_manifest(store.as_ref(), &keys).unwrap();
    let mut devices: BTreeMap<String, usize> = BTreeMap::new();
    for e in m.files.values() {
        *devices.entry(e.device.clone()).or_default() += 1;
    }
    let conflicts = m.files.keys().filter(|p| p.contains("(conflict")).count();
    let deepest = m.files.keys().map(|p| p.matches("(conflict").count()).max().unwrap_or(0);
    let top: BTreeMap<String, usize> = m.files.keys().fold(BTreeMap::new(), |mut acc, p| {
        *acc.entry(p.split('/').next().unwrap_or("").chars().take(18).collect::<String>()).or_default() += 1;
        acc
    });
    println!("entries {} | folders {} | conflict copies {conflicts} | most nested conflict {deepest}", m.files.len(), m.folders.len());
    println!("by device {devices:?}");
    println!("by first path part {:?}", top.iter().filter(|(_, n)| **n > 2).collect::<Vec<_>>());
}

/// Reads every object in the remote manifest once, as a first sync does, and counts the
/// answers. Read-only. `QUIRE_LIVE_STATE=… cargo test live_burst -- --ignored --nocapture`.
#[test]
#[ignore]
fn live_burst() {
    let Ok(path) = std::env::var("QUIRE_LIVE_STATE") else { return };
    let state: serde_json::Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    let sy = &state["settings"]["sync"];
    let provider = sy["provider"].as_str().unwrap_or("none").to_string();
    let cfg: BTreeMap<String, String> = sy["cfg"][&provider].as_object().unwrap().iter().map(|(k, v)| (k.clone(), v.as_str().unwrap_or("").to_string())).collect();
    let prefix = clean_prefix(cfg.get("prefix").map_or("quire/notes/", |s| s.as_str()));
    let r = Remote { provider, cfg, prefix, encrypt: sy["encrypt"].as_bool().unwrap_or(false), pass: sy["pass"].as_str().unwrap_or("").to_string() };
    let store = open_store(&r).unwrap();
    let keys = open_format(store.as_ref(), &r, false).unwrap();
    let m = read_manifest(store.as_ref(), &keys).unwrap();
    let t = std::time::Instant::now();
    let entries: Vec<&Entry> = m.files.values().collect();
    let got = parallel(&entries, |e| store.get(&e.object));
    match got {
        Ok(v) => println!("{} objects in {:.1} s, {WORKERS} at a time | ok {} | missing {}", v.len(), t.elapsed().as_secs_f64(), v.iter().filter(|b| b.is_some()).count(), v.iter().filter(|b| b.is_none()).count()),
        Err(e) => println!("failed after {:.1} s: {e}", t.elapsed().as_secs_f64()),
    }
}

/// A gateway that drops one request in three still gives a full sync, and one that drops
/// them all gives a message without its HTML page.
#[test]
fn a_busy_gateway_is_retried() {
    let (url, objects) = s3_server_failing("testsecret", 3);
    let r = custom(&url, "notes", "testsecret", false);
    let (a, b) = (Device::new("flaky", "laptop"), Device::new("flaky", "phone"));
    for i in 0..40 {
        a.write(&format!("Note {i}.md"), &format!("# {i}"));
    }
    let store = open_store(&r).unwrap();
    let out = run_with(store.as_ref(), &r, &a.root, "laptop", &a.base).unwrap();
    assert_eq!(out.uploaded, 40);
    let out = run_with(store.as_ref(), &r, &b.root, "phone", &b.base).unwrap();
    assert_eq!(out.pulls.len(), 40);
    assert!(objects.lock().unwrap().len() >= 42);
    let (url, _) = s3_server_failing("testsecret", 1);
    let err = test(&custom(&url, "notes", "testsecret", false), 0, false).unwrap_err();
    assert_eq!(err, "The storage service did not answer in time (504). Quire tries again on the next sync.");
}

/// Syncs with `pins` as the device's pins, then takes the pins the sync returns.
fn sync_pins(d: &Device, bucket: &mut Mem, r: &Remote, pins: &mut BTreeSet<String>) -> Outcome {
    let out = run_with_pins(bucket, r, &d.root, d.name, &d.base, pins).unwrap();
    for pl in &out.pulls {
        d.write(&pl.path, &pl.text);
    }
    for (p, _) in &out.removes {
        let _ = std::fs::remove_file(d.root.join(p));
    }
    out.save_base().unwrap();
    *pins = out.pins.clone().unwrap();
    out
}

fn set(v: &[&str]) -> BTreeSet<String> {
    v.iter().map(|s| s.to_string()).collect()
}

#[test]
fn pin_merge_rules() {
    let (l, r, b) = (set(&["a", "b"]), set(&["b", "c"]), set(&["b"]));
    // Added on either side: kept.
    assert_eq!(merge_pins(&l, Some(&r), Some(&b)), set(&["a", "b", "c"]));
    // Removed on one side: removed.
    assert_eq!(merge_pins(&set(&["a"]), Some(&set(&["a", "b"])), Some(&set(&["a", "b"]))), set(&["a"]));
    assert_eq!(merge_pins(&set(&["a", "b"]), Some(&set(&["a"])), Some(&set(&["a", "b"]))), set(&["a"]));
    // A manifest from a version without pins: the pins here stay.
    assert_eq!(merge_pins(&l, None, Some(&b)), l);
    // The first sync with pins on this device removes nothing.
    assert_eq!(merge_pins(&l, Some(&r), None), set(&["a", "b", "c"]));
}

#[test]
fn pins_sync() {
    let mut bucket = Mem::default();
    let r = remote(true);
    let (laptop, phone) = (Device::new("pins", "laptop"), Device::new("pins", "phone"));
    for n in ["A.md", "B.md", "Drafts/C.md"] {
        laptop.write(n, "x");
    }
    let (mut lp, mut pp) = (set(&["A.md", "B.md"]), BTreeSet::new());
    sync_pins(&laptop, &mut bucket, &r, &mut lp);
    sync_pins(&phone, &mut bucket, &r, &mut pp);
    assert_eq!(pp, set(&["A.md", "B.md"]), "the phone gets the laptop's pins");
    // The phone unpins A and pins C. The laptop gets both changes.
    pp = set(&["B.md", "Drafts/C.md"]);
    sync_pins(&phone, &mut bucket, &r, &mut pp);
    sync_pins(&laptop, &mut bucket, &r, &mut lp);
    assert_eq!(lp, set(&["B.md", "Drafts/C.md"]));
    // A rename on the laptop moves the pin, on the phone too.
    std::fs::rename(laptop.root.join("B.md"), laptop.root.join("D.md")).unwrap();
    lp = set(&["D.md", "Drafts/C.md"]);
    sync_pins(&laptop, &mut bucket, &r, &mut lp);
    sync_pins(&phone, &mut bucket, &r, &mut pp);
    assert_eq!(pp, set(&["D.md", "Drafts/C.md"]));
    assert_eq!(phone.read("D.md").as_deref(), Some("x"));
    // A deleted note loses its pin on both devices.
    std::fs::remove_file(phone.root.join("Drafts/C.md")).unwrap();
    sync_pins(&phone, &mut bucket, &r, &mut pp);
    assert_eq!(pp, set(&["D.md"]));
    sync_pins(&laptop, &mut bucket, &r, &mut lp);
    assert_eq!(lp, set(&["D.md"]));
    // A sync with no pin change writes no pins.
    let before = bucket.0.lock().unwrap().clone();
    sync_pins(&laptop, &mut bucket, &r, &mut lp);
    assert_eq!(*bucket.0.lock().unwrap(), before);
}
