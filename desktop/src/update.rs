//! Updates. Quire asks GitHub for the latest release and shows an Update button in the title
//! bar when that release is newer. A press downloads the release file for this system, unpacks
//! it, runs the new binary once to check it, puts it in place of the running app and starts it
//! again. Linux, macOS and Windows do this. On another system, or when Quire cannot write to its
//! own folder, the button opens the release page.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use rune::Tx;

use crate::app::App;
use crate::sync::app::Job;

const API: &str = "https://api.github.com/repos/kurtlieber/quire/releases/latest";
const DOWNLOADS: &str = "https://github.com/kurtlieber/quire/releases/download/";
pub const PAGE: &str = "https://github.com/kurtlieber/quire/releases/latest";
// This fork: own releases are upstream's version plus -kl.N (update/fork.rs, FORK.md).
mod fork;
pub const CURRENT: &str = fork::VERSION;
/// The time from one automatic check to the next, in seconds.
const EVERY: f64 = 6.0 * 3600.0;
/// The time from the start to the first check, in seconds.
const FIRST: f64 = 10.0;

#[derive(Clone, Debug, PartialEq)]
pub struct Release {
    pub version: String,
    /// The file for this system. None: there is no file for it, so the button opens the page.
    pub url: Option<String>,
}

#[derive(Default)]
pub struct UpdateState {
    check: Option<Job<Release>>,
    apply: Option<Job<Restart>>,
    /// The check that runs now came from "Check now" in Settings.
    manual: bool,
    next_check: Option<f64>,
    /// A newer release.
    pub available: Option<Release>,
    /// The result of the last "Check now", for Settings.
    pub message: Option<String>,
}

impl UpdateState {
    pub fn checking(&self) -> bool {
        self.check.is_some()
    }
    pub fn applying(&self) -> bool {
        self.apply.is_some()
    }
}

/// `1.2.3` or `v1.2.3` as three numbers.
pub fn parse_version(s: &str) -> Option<(u64, u64, u64)> {
    let s = s.trim().trim_start_matches('v');
    let core = s.split(['-', '+']).next()?;
    let mut it = core.split('.').map(|p| p.parse::<u64>().ok());
    let v = (it.next()??, it.next()??, it.next()??);
    it.next().is_none().then_some(v)
}

/// Whether `latest` is newer than `current`.
pub fn newer(latest: &str, current: &str) -> bool {
    fork::newer(latest, current) // this fork: four numbers (update/fork.rs)
}

/// The release file for this system.
pub fn asset_name(version: &str) -> Option<String> {
    if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        Some(format!("quire-{version}-linux-x86_64.tar.gz"))
    } else if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
        Some(format!("quire-{version}-macos-arm64.zip"))
    } else if cfg!(all(target_os = "windows", target_arch = "x86_64")) {
        Some(format!("quire-{version}-windows-x86_64.zip"))
    } else {
        None
    }
}

/// Reads the answer of the GitHub releases API. Only a file under this project's release
/// downloads counts.
pub fn parse_release(json: &str, asset: impl Fn(&str) -> Option<String>) -> Result<Release, String> {
    let v: serde_json::Value = serde_json::from_str(json).map_err(|e| format!("The release answer is not JSON: {e}"))?;
    let tag = v["tag_name"].as_str().ok_or("The release has no tag.")?;
    let version = tag.trim_start_matches('v').to_string();
    if parse_version(&version).is_none() {
        return Err(format!("The release tag {tag} is not a version."));
    }
    let want = asset(&version);
    let url = want.and_then(|name| {
        v["assets"].as_array()?.iter().find(|a| a["name"].as_str() == Some(name.as_str()))?["browser_download_url"]
            .as_str()
            .filter(|u| u.starts_with(DOWNLOADS))
            .map(String::from)
    });
    Ok(Release { version, url })
}

fn agent() -> ureq::Agent {
    ureq::AgentBuilder::new().timeout_connect(Duration::from_secs(15)).timeout_read(Duration::from_secs(60)).user_agent("quire-updater").build()
}

/// Asks GitHub for the latest release.
pub fn latest() -> Result<Release, String> {
    let r = agent().get(API).set("Accept", "application/vnd.github+json").call().map_err(|e| format!("GitHub did not answer: {e}"))?;
    let body = r.into_string().map_err(|e| e.to_string())?;
    parse_release(&body, asset_name)
}

fn download(url: &str, to: &Path) -> Result<(), String> {
    let r = agent().get(url).call().map_err(|e| format!("The download failed: {e}"))?;
    let mut bytes = Vec::new();
    // A release file is about 20 MB. 300 MB is far past any real one.
    r.into_reader().take(300 << 20).read_to_end(&mut bytes).map_err(|e| format!("The download stopped: {e}"))?;
    std::fs::write(to, bytes).map_err(|e| format!("Quire could not save the download: {e}"))
}

fn run(cmd: &mut Command) -> Result<(), String> {
    let out = cmd.output().map_err(|e| format!("{cmd:?} did not start: {e}"))?;
    if out.status.success() { Ok(()) } else { Err(format!("{cmd:?} failed: {}", String::from_utf8_lossy(&out.stderr).trim())) }
}

/// Runs the new binary once. A file that is cut short or for another system does not run.
fn check_binary(exe: &Path) -> Result<(), String> {
    let out = Command::new(exe).arg("--help").output().map_err(|e| format!("The new version did not start: {e}"))?;
    if out.status.success() && String::from_utf8_lossy(&out.stdout).starts_with("Quire") {
        Ok(())
    } else {
        Err("The new version did not start.".into())
    }
}

/// The running binary. Linux adds " (deleted)" when the file was replaced since the start.
pub fn current_exe() -> Result<PathBuf, String> {
    let exe = std::env::current_exe().map_err(|e| format!("Quire could not find its own file: {e}"))?;
    let s = exe.to_string_lossy();
    Ok(match s.strip_suffix(" (deleted)") {
        Some(t) => PathBuf::from(t),
        None => exe,
    })
}

/// A build in a Cargo `target` folder is not an installed copy.
fn is_dev_build(exe: &Path) -> bool {
    let parts: Vec<_> = exe.components().map(|c| c.as_os_str().to_string_lossy().into_owned()).collect();
    parts.windows(2).any(|w| w[0] == "target" && (w[1] == "release" || w[1] == "debug"))
}

/// The `.app` bundle around a macOS binary: `Quire.app/Contents/MacOS/quire`.
pub fn bundle_of(exe: &Path) -> Option<PathBuf> {
    let b = exe.parent()?.parent()?.parent()?;
    (b.extension().is_some_and(|e| e == "app") && exe.parent()?.file_name()? == "MacOS").then(|| b.to_path_buf())
}

/// Whether Quire can make files in `dir`.
fn writable(dir: &Path) -> bool {
    let probe = dir.join(format!(".quire-write-{}", std::process::id()));
    let ok = std::fs::write(&probe, b"").is_ok();
    let _ = std::fs::remove_file(&probe);
    ok
}

/// What starts the new version.
#[derive(Debug)]
pub enum Restart {
    Exe(PathBuf),
    App(PathBuf),
}

impl Restart {
    fn spawn(&self) -> std::io::Result<()> {
        match self {
            Restart::Exe(exe) => Command::new(exe).args(std::env::args_os().skip(1)).stdin(std::process::Stdio::null()).spawn().map(|_| ()),
            Restart::App(bundle) => Command::new("open").arg("-n").arg(bundle).spawn().map(|_| ()),
        }
    }
}

/// The place Quire can update: the binary, or the bundle on macOS. Err when Quire cannot
/// replace itself there.
fn target() -> Result<PathBuf, String> {
    let exe = current_exe()?;
    if is_dev_build(&exe) {
        return Err("This copy of Quire runs from a build folder.".into());
    }
    let place = if cfg!(target_os = "macos") { bundle_of(&exe).ok_or("Quire is not in an app bundle.")? } else { exe };
    let dir = place.parent().ok_or("Quire is in no folder.")?;
    if !writable(dir) {
        return Err(format!("Quire cannot write to {}.", dir.display()));
    }
    Ok(place)
}

/// Downloads `url` and puts the new version in place of `place`.
pub fn apply_to(place: &Path, url: &str) -> Result<Restart, String> {
    let dir = place.parent().ok_or("Quire is in no folder.")?;
    // The work folder is next to the app, so a rename moves the new version in place.
    let work = dir.join(format!(".quire-update-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&work);
    std::fs::create_dir_all(&work).map_err(|e| format!("Quire could not make {}: {e}", work.display()))?;
    let r = apply_in(place, url, &work);
    let _ = std::fs::remove_dir_all(&work);
    r
}

fn apply_in(place: &Path, url: &str, work: &Path) -> Result<Restart, String> {
    let file = work.join(url.rsplit('/').next().unwrap_or("quire-update"));
    download(url, &file)?;
    let out = work.join("x");
    std::fs::create_dir_all(&out).map_err(|e| e.to_string())?;
    if cfg!(target_os = "macos") {
        run(Command::new("ditto").arg("-x").arg("-k").arg(&file).arg(&out))?;
        let new = out.join("Quire.app");
        check_binary(&new.join("Contents/MacOS/quire"))?;
        let old = work.join("Quire-old.app");
        std::fs::rename(place, &old).map_err(|e| format!("Quire could not move the old version: {e}"))?;
        if let Err(e) = std::fs::rename(&new, place) {
            let _ = std::fs::rename(&old, place);
            return Err(format!("Quire could not put the new version in place: {e}"));
        }
        return Ok(Restart::App(place.to_path_buf()));
    }
    // Linux: a .tar.gz. Windows 10 and later has tar.exe, which also unpacks a .zip.
    run(Command::new("tar").arg("-xf").arg(&file).arg("-C").arg(&out))?;
    let name = if cfg!(windows) { "quire.exe" } else { "quire" };
    let new = std::fs::read_dir(&out)
        .map_err(|e| e.to_string())?
        .filter_map(|e| e.ok())
        .map(|e| e.path().join(name))
        .find(|p| p.is_file())
        .ok_or("The download has no Quire binary.")?;
    check_binary(&new)?;
    if cfg!(windows) {
        // Windows does not let a running binary be replaced, but it can be renamed.
        let old = place.with_extension("exe.old");
        let _ = std::fs::remove_file(&old);
        std::fs::rename(place, &old).map_err(|e| format!("Quire could not move the old version: {e}"))?;
        if let Err(e) = std::fs::rename(&new, place) {
            let _ = std::fs::rename(&old, place);
            return Err(format!("Quire could not put the new version in place: {e}"));
        }
    } else {
        // A rename over the running binary is safe on Linux: the old file stays until Quire quits.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&new, std::fs::Permissions::from_mode(0o755));
        }
        std::fs::rename(&new, place).map_err(|e| format!("Quire could not put the new version in place: {e}"))?;
    }
    Ok(Restart::Exe(place.to_path_buf()))
}

/// Removes what an update on Windows leaves next to the binary.
pub fn clean_up() {
    if cfg!(windows) {
        if let Ok(exe) = current_exe() {
            let _ = std::fs::remove_file(exe.with_extension("exe.old"));
        }
    }
}

impl App {
    /// Starts a check now, from "Check now" in Settings.
    pub fn check_updates_now(&mut self) {
        if self.update.check.is_some() {
            return;
        }
        self.update.manual = true;
        self.update.message = None;
        self.update.check = Some(Job::spawn(self.waker.clone(), latest));
    }

    /// The Update button: put the new version in place and restart, or open the release page.
    pub fn start_update(&mut self, tx: &mut Tx) {
        let Some(rel) = self.update.available.clone() else { return };
        if self.update.apply.is_some() {
            return;
        }
        let place = match (rel.url.clone(), target()) {
            (Some(url), Ok(place)) => (url, place),
            (_, why) => {
                if let Err(e) = why {
                    eprintln!("quire: update: {e}");
                }
                if let Err(e) = tx.open_url(PAGE) {
                    self.notice = Some(format!("Quire could not open the release page: {e}"));
                }
                return;
            }
        };
        self.update.apply = Some(Job::spawn(self.waker.clone(), move || apply_to(&place.1, &place.0)));
    }

    /// Picks up checks and updates, and starts the automatic check when it is due.
    pub fn poll_update(&mut self, t: f64, tx: &mut Tx) -> bool {
        let mut changed = false;
        if let Some(r) = self.update.check.as_ref().and_then(|j| j.take()) {
            self.update.check = None;
            let manual = std::mem::take(&mut self.update.manual);
            match r {
                Ok(rel) if newer(&rel.version, CURRENT) => {
                    self.update.message = manual.then(|| format!("Quire {} is out. Select Update at the top right.", rel.version));
                    self.update.available = Some(rel);
                }
                Ok(_) => {
                    self.update.message = manual.then(|| format!("Quire {CURRENT} is the latest version."));
                    self.update.available = None;
                }
                Err(e) => {
                    self.update.message = manual.then_some(e);
                }
            }
            changed = true;
        }
        if let Some(r) = self.update.apply.as_ref().and_then(|j| j.take()) {
            self.update.apply = None;
            match r {
                Ok(restart) => {
                    self.persist.updated_from = Some(CURRENT.to_string());
                    self.flush_all();
                    match restart.spawn() {
                        Ok(()) => tx.close_window(),
                        Err(e) => self.notice = Some(format!("Quire is updated. Start it again to use the new version. ({e})")),
                    }
                }
                Err(e) => {
                    self.notice = Some(format!("Quire could not update: {e}"));
                }
            }
            changed = true;
        }
        if rune::is_headless() || !self.settings().check_updates {
            return changed;
        }
        let due = *self.update.next_check.get_or_insert(t + FIRST);
        if t >= due && self.update.check.is_none() {
            self.update.next_check = Some(t + EVERY);
            self.update.check = Some(Job::spawn(self.waker.clone(), latest));
        } else if t < due {
            tx.wake_after(Duration::from_secs_f64(due - t));
        }
        changed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const JSON: &str = r#"{"tag_name":"v0.6.0","draft":false,"assets":[
        {"name":"quire-0.6.0-linux-x86_64.tar.gz","browser_download_url":"https://github.com/kurtlieber/quire/releases/download/v0.6.0/quire-0.6.0-linux-x86_64.tar.gz"},
        {"name":"quire-0.6.0-macos-arm64.zip","browser_download_url":"https://example.com/quire-0.6.0-macos-arm64.zip"}]}"#;

    #[test]
    fn versions() {
        assert_eq!(parse_version("v0.5.3"), Some((0, 5, 3)));
        assert_eq!(parse_version("1.10.0"), Some((1, 10, 0)));
        assert_eq!(parse_version("1.2"), None);
        assert_eq!(parse_version("1.2.3.4"), None);
        assert!(newer("0.5.10", "0.5.9"));
        assert!(newer("v1.0.0", "0.9.9"));
        assert!(!newer("0.5.3", "0.5.3"));
        assert!(!newer("0.5.2", "0.5.3"));
        assert!(!newer("junk", "0.5.3"));
    }

    #[test]
    fn release_answer() {
        let linux = parse_release(JSON, |v| Some(format!("quire-{v}-linux-x86_64.tar.gz"))).unwrap();
        assert_eq!(linux.version, "0.6.0");
        assert!(linux.url.unwrap().starts_with(DOWNLOADS));
        // A file from another host does not count.
        assert_eq!(parse_release(JSON, |v| Some(format!("quire-{v}-macos-arm64.zip"))).unwrap().url, None);
        assert_eq!(parse_release(JSON, |v| Some(format!("quire-{v}-windows-x86_64.zip"))).unwrap().url, None);
        assert_eq!(parse_release(JSON, |_| None).unwrap().url, None);
        assert!(parse_release(r#"{"tag_name":"nightly"}"#, |_| None).is_err());
        assert!(parse_release("<html>", |_| None).is_err());
    }

    #[test]
    fn places() {
        assert_eq!(bundle_of(Path::new("/Applications/Quire.app/Contents/MacOS/quire")), Some(PathBuf::from("/Applications/Quire.app")));
        assert_eq!(bundle_of(Path::new("/usr/local/bin/quire")), None);
        assert!(is_dev_build(Path::new("/home/a/quire/desktop/target/release/quire")));
        assert!(!is_dev_build(Path::new("/home/a/.local/bin/quire")));
    }

    /// Downloads the latest Linux release and puts it in place of a copy of this binary in a
    /// scratch folder. It needs the network, so it runs only on request:
    /// `cargo test live_update -- --ignored`.
    #[test]
    #[ignore]
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    fn live_update() {
        let rel = latest().expect("latest release");
        let url = rel.url.expect("a Linux file");
        let dir = std::env::temp_dir().join(format!("quire-live-update-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let exe = dir.join("quire");
        std::fs::write(&exe, b"#!/bin/sh\necho old\n").unwrap();
        match apply_to(&exe, &url).expect("apply") {
            Restart::Exe(p) => assert_eq!(p, exe),
            r => panic!("{r:?}"),
        }
        check_binary(&exe).expect("the new binary runs");
        let left: Vec<_> = std::fs::read_dir(&dir).unwrap().filter_map(|e| e.ok()).map(|e| e.file_name()).collect();
        assert_eq!(left.len(), 1, "only the binary is left: {left:?}");
        std::fs::remove_dir_all(&dir).unwrap();
        println!("updated to {}", rel.version);
    }
}
