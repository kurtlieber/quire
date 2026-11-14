//! What Typo keeps between runs: settings, known folders and each folder's open tabs.
//! It lives in the user's config folder as `state.json`, never inside a notes folder.

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

fn yes() -> bool {
    true
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Settings {
    /// light, dark or system.
    pub theme: String,
    /// The theme for light mode. None picks Paper, or Omarchy's theme under "Match system".
    pub light_theme: Option<String>,
    /// The theme for dark mode. None picks Ink, or Omarchy's theme under "Match system".
    pub dark_theme: Option<String>,
    /// `#rrggbb`, or empty for the theme's own accent.
    pub accent: String,
    pub font: String,
    pub size: u32,
    pub width: String,
    pub preview_font: String,
    pub spellcheck: bool,
    pub continue_lists: bool,
    pub dim_focus: bool,
    pub typewriter: bool,
    /// The formatting toolbar above the note, in source mode.
    pub toolbar: bool,
    pub update_links: bool,
    pub new_folder: String,
    /// Ask GitHub for new releases, and show the Update button.
    pub check_updates: bool,
    pub sync: Sync,
    pub mcp: Mcp,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            theme: "light".into(),
            light_theme: None,
            dark_theme: None,
            accent: String::new(),
            font: "plex".into(),
            size: 16,
            width: "readable".into(),
            preview_font: "serif".into(),
            spellcheck: false,
            continue_lists: true,
            dim_focus: true,
            typewriter: true,
            toolbar: true,
            update_links: true,
            new_folder: String::new(),
            check_updates: true,
            sync: Sync::default(),
            mcp: Mcp::default(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Sync {
    pub provider: String,
    /// Provider id to field key to value.
    pub cfg: BTreeMap<String, BTreeMap<String, String>>,
    pub when: String,
    pub encrypt: bool,
    pub pass: String,
    /// The last "Test connection" passed with these settings.
    pub connected: bool,
    /// When the last sync finished, in milliseconds since 1970.
    pub last_sync: Option<i64>,
    /// The remote folder of each notes folder, by its path. A notes folder without one uses
    /// `quire/<folder name>/`, so two notes folders never share a remote folder by accident.
    pub folders: BTreeMap<String, String>,
}

impl Default for Sync {
    fn default() -> Self {
        Sync { provider: "none".into(), cfg: BTreeMap::new(), when: "save".into(), encrypt: false, pass: String::new(), connected: false, last_sync: None, folders: BTreeMap::new() }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Mcp {
    pub enabled: bool,
    pub transport: String,
    pub port: String,
    pub access: String,
    pub confirm: bool,
    /// Folders apps cannot see. An empty string is the folder root.
    pub hidden: Vec<String>,
    pub token: String,
    pub tools: BTreeMap<String, bool>,
}

pub const MCP_TOOLS: [(&str, &str, bool); 8] = [
    ("list_notes", "List notes and folders", false),
    ("read_note", "Read a note as Markdown", false),
    ("search_notes", "Full-text and tag search", false),
    ("get_backlinks", "Find notes that link to a note", false),
    ("create_note", "Create a new note", true),
    ("update_note", "Replace or append to a note", true),
    ("rename_note", "Rename a note and update links", true),
    ("delete_note", "Move a note to the trash", true),
];

impl Default for Mcp {
    fn default() -> Self {
        let on = ["list_notes", "read_note", "search_notes", "get_backlinks", "create_note", "update_note"];
        Mcp {
            enabled: false,
            transport: "stdio".into(),
            port: "3917".into(),
            access: "read".into(),
            confirm: true,
            hidden: Vec::new(),
            token: new_token(),
            tools: MCP_TOOLS.iter().map(|(id, _, _)| (id.to_string(), on.contains(id))).collect(),
        }
    }
}

impl Mcp {
    /// Whether a connected app may call `tool`, given the access level.
    pub fn allows(&self, tool: &str) -> bool {
        let write = MCP_TOOLS.iter().find(|t| t.0 == tool).is_some_and(|t| t.2);
        (!write || self.access == "write") && self.tools.get(tool).copied().unwrap_or(false)
    }
}

/// A random `qk_` token, 28 characters from an unambiguous alphabet.
pub fn new_token() -> String {
    let a = b"abcdefghijkmnopqrstuvwxyz23456789";
    let mut seed = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(7, |d| d.as_nanos() as u64);
    seed ^= std::process::id() as u64;
    let mut bytes = [0u8; 28];
    if std::fs::File::open("/dev/urandom").and_then(|mut f| std::io::Read::read_exact(&mut f, &mut bytes)).is_err() {
        for b in &mut bytes {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            *b = seed as u8;
        }
    }
    let mut t = String::from("qk_");
    for b in bytes {
        t.push(a[b as usize % a.len()] as char);
    }
    t
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct VaultRef {
    pub name: String,
    pub path: PathBuf,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Default)]
#[serde(default)]
pub struct Session {
    pub tabs: Vec<String>,
    pub active: Option<String>,
    pub recent: Vec<String>,
    pub expanded: Vec<String>,
    /// Pinned notes, in the order they were pinned.
    pub pinned: Vec<String>,
    /// The Pinned group in the tree is closed.
    pub pins_closed: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Persist {
    pub vaults: Vec<VaultRef>,
    /// The folder open when Typo quit. None opens the welcome screen.
    pub last: Option<PathBuf>,
    pub sessions: BTreeMap<String, Session>,
    pub mode: String,
    #[serde(default = "yes")]
    pub left_open: bool,
    #[serde(default = "yes")]
    pub right_open: bool,
    pub settings: Settings,
    /// Set once the sample folder exists, so deleting it does not bring it back.
    pub seeded: bool,
    /// The version that installed an update and restarted. The new version reads it once.
    pub updated_from: Option<String>,
}

impl Default for Persist {
    fn default() -> Self {
        Persist {
            vaults: Vec::new(),
            last: None,
            sessions: BTreeMap::new(),
            mode: "source".into(),
            left_open: true,
            right_open: true,
            settings: Settings::default(),
            seeded: false,
            updated_from: None,
        }
    }
}

pub fn dirs() -> Option<rune::dirs::AppDirs> {
    rune::dirs::AppDirs::new("typo")
}

/// Typo was Quire until 2.0.0. On the first start after the rename, Quire's folders become
/// Typo's: the settings and sync keys (config) and the sample notes (data). A folder Typo
/// already has stays as it is.
pub fn move_from_quire() {
    // TYPO_STATE marks an isolated run, which must leave the real folders alone.
    if std::env::var_os("TYPO_STATE").is_some() {
        return;
    }
    if let Some(d) = dirs() {
        move_dirs(&d);
    }
}

fn move_dirs(new: &rune::dirs::AppDirs) {
    let old = new.sibling("quire");
    // On macOS and Windows config and data are one folder, so the second move finds nothing.
    for (from, to) in [(old.config(), new.config()), (old.data(), new.data())] {
        if from == to || !from.is_dir() || to.exists() {
            continue;
        }
        if let Some(p) = to.parent() {
            let _ = std::fs::create_dir_all(p);
        }
        if let Err(e) = std::fs::rename(from, to) {
            eprintln!("typo: could not move {} to {}: {e}", from.display(), to.display());
        }
    }
}

fn state_path() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("TYPO_STATE") {
        return Some(PathBuf::from(p));
    }
    dirs().map(|d| d.config().join("state.json"))
}

pub fn load() -> Persist {
    let Some(p) = state_path() else { return Persist::default() };
    std::fs::read(&p).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

/// Writes the state file atomically, readable only by the user (it can hold sync keys).
pub fn save(p: &Persist) -> std::io::Result<()> {
    let Some(path) = state_path() else { return Ok(()) };
    if let Some(d) = path.parent() {
        std::fs::create_dir_all(d)?;
    }
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, serde_json::to_vec_pretty(p).unwrap_or_default())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o600));
    }
    std::fs::rename(&tmp, &path)
}

/// `~/Documents`, or the XDG documents folder on Linux.
pub fn documents_dir() -> PathBuf {
    let home = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")).map(PathBuf::from).unwrap_or_default();
    if cfg!(target_os = "linux") {
        if let Ok(conf) = std::fs::read_to_string(home.join(".config/user-dirs.dirs")) {
            for line in conf.lines() {
                if let Some(v) = line.strip_prefix("XDG_DOCUMENTS_DIR=") {
                    let v = v.trim_matches('"').replace("$HOME", &home.to_string_lossy());
                    if !v.is_empty() {
                        return PathBuf::from(v);
                    }
                }
            }
        }
    }
    home.join("Documents")
}

/// A path with the home folder written as `~`.
pub fn tilde(p: &std::path::Path) -> String {
    let home = std::env::var_os("HOME").map(PathBuf::from);
    match home {
        Some(h) if !h.as_os_str().is_empty() && p.starts_with(&h) => {
            let rest = p.strip_prefix(&h).unwrap_or(p);
            if rest.as_os_str().is_empty() { "~".into() } else { format!("~/{}", rest.to_string_lossy()) }
        }
        _ => p.to_string_lossy().into_owned(),
    }
}

#[cfg(test)]
mod move_tests {
    use super::move_dirs;
    use rune::dirs::AppDirs;

    #[test]
    fn quire_folders_become_typo_folders_once() {
        let root = std::env::temp_dir().join(format!("typo-move-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let new = AppDirs::rooted(&root, "typo");
        let old = new.sibling("quire");
        std::fs::create_dir_all(old.config().join("sync")).unwrap();
        std::fs::write(old.config().join("state.json"), "{}").unwrap();
        std::fs::create_dir_all(old.data().join("Field Notes")).unwrap();

        move_dirs(&new);
        assert!(new.config().join("state.json").is_file());
        assert!(new.config().join("sync").is_dir());
        assert!(new.data().join("Field Notes").is_dir());
        assert!(!old.config().exists() && !old.data().exists());

        // A Quire folder that turns up later doesn't replace Typo's.
        std::fs::create_dir_all(old.config()).unwrap();
        std::fs::write(old.config().join("state.json"), "old").unwrap();
        move_dirs(&new);
        assert_eq!(std::fs::read_to_string(new.config().join("state.json")).unwrap(), "{}");
        assert!(old.config().join("state.json").is_file());
        let _ = std::fs::remove_dir_all(&root);
    }
}
