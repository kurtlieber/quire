//! Pinned notes. A pinned note shows in the Pinned group at the top of the tree, not in its
//! folder. The file does not move. The pins are kept for each notes folder in `state.json`.

use crate::app::App;
use crate::md;

/// The key of the Pinned group in the tree. A note dropped anywhere in the group gets a pin.
pub const GROUP: &str = "pins";

/// Whether an element path points into the Pinned group.
pub fn in_group(path: &str) -> bool {
    path.split('/').any(|s| s == GROUP)
}

impl App {
    pub fn is_pinned(&self, path: &str) -> bool {
        self.pinned.iter().any(|p| p == path)
    }

    /// The pinned notes that exist, sorted by name as the tree sorts notes.
    pub fn pinned_notes(&self) -> Vec<String> {
        let files = self.files();
        let mut v: Vec<String> = self.pinned.iter().filter(|p| files.contains_key(*p)).cloned().collect();
        v.sort_by(|a, b| {
            let (x, y) = (md::base(a), md::base(b));
            x.to_lowercase().cmp(&y.to_lowercase()).then_with(|| a.cmp(b))
        });
        v
    }

    pub fn pin(&mut self, path: &str) {
        if !self.is_pinned(path) && self.files().contains_key(path) {
            self.pinned.push(path.to_string());
            self.pins_open = true;
            self.persist_later();
            self.sync_auto(1.5);
        }
    }

    pub fn unpin(&mut self, path: &str) {
        let n = self.pinned.len();
        self.pinned.retain(|p| p != path);
        if self.pinned.len() != n {
            self.persist_later();
            self.sync_auto(1.5);
        }
    }

    pub fn toggle_pin(&mut self, path: &str) {
        if self.is_pinned(path) { self.unpin(path) } else { self.pin(path) }
    }

    pub fn toggle_pins_open(&mut self) {
        self.pins_open = !self.pins_open;
        self.persist_later();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn group_paths() {
        assert!(in_group("root/body/sidebar/side-scroll/side-list/tree/pins/pins-head"));
        assert!(in_group("root/body/sidebar/side-scroll/side-list/tree/pins/file:Journal|2026.md/#1"));
        assert!(!in_group("root/body/sidebar/side-scroll/side-list/tree/dir:pins"));
        assert!(!in_group("root/body/sidebar/side-scroll/side-list/tree/file:pins.md"));
    }
}
