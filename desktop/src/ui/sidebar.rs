//! The left sidebar: the folder menu, files, search and tags.

use std::collections::BTreeMap;

use rune::{Color, El, ScrollSpec, Span, col, el, icon, rich, row, text, text_input};

use super::V;
use super::widgets::{CHEVRON_DOWN, CHEVRON_RIGHT, CLOSE, FOLDER, FOLDER_OPEN, FOLDER_PLUS, PIN, PLUS, SEARCH, label};
use crate::folders::{MenuOn, row_key};
use crate::app::{App, Panel, Sel};
use crate::keys::k;
use crate::md;
use crate::theme::{self, mono, sans};

pub fn sidebar(v: V) -> El {
    let p = v.p;
    let s = v.s;
    let vault = s.vault.as_ref();
    let name = vault.map_or(String::new(), |v| v.name.clone());

    let head = row().key("vault-head").pt(4.0).px(8.0).pb(6.0).w_full().child(
        row()
            .key("vault-button")
            .w_full()
            .h(32.0)
            .px(10.0)
            .gap(6.0)
            .center_y()
            .radius(7.0)
            .hover(p.hover)
            .role("button")
            .label(name.clone())
            .on_press(|s: &mut App| s.vault_menu = !s.vault_menu)
            .child(text(name).font(sans(14.0, 600.0)).color(p.ink).truncate().grow(1.0).shrink().min_w(0.0))
            .child(icon(&CHEVRON_DOWN, 10.0).color(p.mute)),
    );

    let mut tabs = row().key("panel-tabs").gap(2.0).pl(12.0).pr(12.0).pb(10.0).center_y();
    for (id, lbl) in [(Panel::Files, "files"), (Panel::Search, "search"), (Panel::Tags, "tags")] {
        let on = s.left_panel == id;
        tabs = tabs.child(
            row()
                .key(format!("panel-{lbl}"))
                .h(24.0)
                .px(8.0)
                .center()
                .radius(6.0)
                .fill(if on { p.act } else { Color::TRANSPARENT })
                .hover_group()
                .role("tab")
                .on_press_tx(move |s: &mut App, tx| {
                    s.left_panel = id;
                    if id == Panel::Search {
                        tx.focus("search", false);
                    }
                })
                .child(label(lbl, mono(11.5, 400.0), if on { p.ink } else { p.mute }).group_color(p.ink)),
        );
    }

    if s.left_panel == Panel::Files {
        tabs = tabs.child(el().grow(1.0)).child(
            row()
                .key("new-folder-button")
                .size(24.0, 24.0)
                .center()
                .radius(6.0)
                .hover(p.hover)
                .hover_group()
                .label("New folder")
                .help("New folder")
                .on_press_tx(|s: &mut App, tx| s.start_new_folder("", tx))
                .child(icon(&FOLDER_PLUS, 15.0).color(p.mute).group_color(p.ink)),
        );
    }
    let body = match s.left_panel {
        Panel::Files => files_panel(v),
        Panel::Search => search_panel(v),
        Panel::Tags => tags_panel(v),
    };
    let list = col().key("side-scroll").grow(1.0).min_h(0.0).w_full().scroll(ScrollSpec { stick_bottom: false }).child(col().key("side-list").w_full().pb(12.0).child(body));

    let foot_btn = |key: &str, t: &str, kbd: String, run: fn(&mut App, &mut rune::Tx)| {
        row()
            .key(key)
            .h(30.0)
            .px(10.0)
            .center_y()
            .justify_between()
            .radius(6.0)
            .hover(p.hover)
            .role("button")
            .on_press_tx(move |s: &mut App, tx| run(s, tx))
            .child(label(t, sans(13.0, 400.0), p.ink2))
            .child(label(kbd, mono(11.0, 400.0), p.mute))
    };
    let foot = col()
        .key("side-foot")
        .gap(1.0)
        .pt(6.0)
        .px(8.0)
        .pb(10.0)
        .child(foot_btn("foot-new", "New note", k(&["Mod", "N"]), |s, tx| s.new_note(None, tx)))
        .child(foot_btn("foot-settings", "Settings", k(&["Mod", ","]), |s, _| s.open_settings()));

    let mut side = col().key("sidebar").w(248.0).h_full().min_h(0.0).child(head).child(tabs).child(list).child(foot);
    if s.vault_menu {
        side = side.child(vault_menu(v));
    }
    side
}

fn vault_menu(v: V) -> El {
    let p = v.p;
    let path = v.s.vault.as_ref().map_or(String::new(), |x| crate::store::tilde(&x.path));
    let item = |key: &str, t: &str, run: fn(&mut App, &mut rune::Tx)| {
        row()
            .key(key)
            .w_full()
            .h(30.0)
            .px(10.0)
            .center_y()
            .radius(6.0)
            .hover(p.hover)
            .role("menuitem")
            .on_press_tx(move |s: &mut App, tx| run(s, tx))
            .child(label(t, sans(13.0, 400.0), p.ink))
    };
    let [a, b] = theme::menu_shadows();
    let panel = col()
        .key("vault-menu")
        .absolute(10.0, 40.0)
        .w(228.0)
        .padding(5.0)
        .radius(9.0)
        .fill(p.bg)
        .box_shadow(a)
        .box_shadow(b)
        .role("menu")
        .modal(true, |s: &mut App| s.vault_menu = false)
        .child(text(path).font(mono(11.0, 400.0).lh(11.0 * 1.45)).color(p.mute).wrap().wrap_anywhere().pt(7.0).px(10.0).pb(8.0))
        .child(item("menu-switch", "Switch folder…", |s, tx| s.go_welcome(tx)))
        .child(item("menu-settings", "Settings", |s, _| s.open_settings()));
    el().key("vault-menu-layer").pin(Some(0.0), Some(0.0), Some(0.0), Some(0.0)).paint_order(31).child(panel)
}

struct Node {
    folders: BTreeMap<String, Node>,
    files: Vec<String>,
    path: String,
}

impl Node {
    fn new(path: String) -> Node {
        Node { folders: BTreeMap::new(), files: Vec::new(), path }
    }
    fn add(&mut self, path: &str) -> &mut Node {
        let mut n = self;
        for part in path.split('/') {
            let child = if n.path.is_empty() { part.to_string() } else { format!("{}/{part}", n.path) };
            n = n.folders.entry(part.to_string()).or_insert_with(|| Node::new(child));
        }
        n
    }
}

fn natural(a: &str, b: &str) -> std::cmp::Ordering {
    a.to_lowercase().cmp(&b.to_lowercase()).then_with(|| a.cmp(b))
}

fn files_panel(v: V) -> El {
    let p = v.p;
    let s = v.s;
    let Some(vault) = &s.vault else { return col() };
    let mut root = Node::new(String::new());
    for f in &vault.folders {
        root.add(f);
    }
    let pinned = s.pinned_notes();
    for f in vault.files.keys().filter(|f| !pinned.contains(f)) {
        let d = md::dir(f);
        let n = if d.is_empty() { &mut root } else { root.add(d) };
        n.files.push(f.clone());
    }
    let mut rows = Vec::new();
    if !pinned.is_empty() {
        rows.push(pins_group(v, &pinned));
    }
    walk(v, &root, 0, &mut rows);
    let root_target = s.dragging.as_ref().is_some_and(|d| d.active && d.target.as_deref() == Some(""));
    let mut c = col()
        .key("tree")
        .w_full()
        .role("tree")
        .on_secondary_press(|s: &mut App, at, _| s.open_tree_menu(MenuOn::Root, at.x, at.y));
    if root_target {
        c = c.fill(p.soft.alpha(0.5)).radius(6.0);
    }
    if vault.files.is_empty() && s.new_folder.is_none() {
        c = c.child(label("No notes yet.", sans(13.0, 400.0), p.mute).py(6.0).px(22.0));
    }
    c.children(rows)
}

fn walk(v: V, n: &Node, depth: usize, rows: &mut Vec<El>) {
    let s = v.s;
    if let Some(nf) = &s.new_folder {
        if nf.parent == n.path {
            rows.push(new_folder_row(v, depth, &nf.edit));
        }
    }
    let mut names: Vec<&String> = n.folders.keys().collect();
    names.sort_by(|a, b| natural(a, b));
    for name in names {
        let f = &n.folders[name];
        let open = s.expanded.contains(&f.path);
        let path = f.path.clone();
        let menu_path = f.path.clone();
        let add_path = f.path.clone();
        let add = row()
            .key("add")
            .size(20.0, 20.0)
            .center()
            .radius(5.0)
            .hover(v.p.hover)
            .label("New note in this folder")
            .help("New note here")
            .group_opacity(0.0, 1.0)
            .on_press_tx(move |s: &mut App, tx| s.new_note_in(Some(&add_path), None, tx))
            .child(icon(&PLUS, 10.0).color(v.p.mute));
        rows.push(
            tree_row(v, &row_key("dir", &f.path), name, depth, true, open, false, None)
                .help(f.path.clone())
                .on_press(move |s: &mut App| s.toggle_folder(&path))
                .on_secondary_press(move |s: &mut App, at, _| s.open_tree_menu(MenuOn::Folder(menu_path.clone()), at.x, at.y))
                .child(add),
        );
        if open {
            walk(v, f, depth + 1, rows);
        }
    }
    let mut files: Vec<&String> = n.files.iter().collect();
    files.sort_by(|a, b| natural(md::base(a), md::base(b)));
    for f in files {
        rows.push(note_row(v, f, depth));
    }
}

/// A note's row: a press opens it, a drag moves it, and a right-click opens its menu.
fn note_row(v: V, f: &str, depth: usize) -> El {
    let on = v.s.active.as_deref() == Some(f);
    let (path, drag_path, menu_path) = (f.to_string(), f.to_string(), f.to_string());
    tree_row(v, &row_key("file", f), md::base(f), depth, false, false, on, None)
        .help(f.to_string())
        .on_press_tx(move |s: &mut App, tx| s.open_file(&path, None, tx))
        .on_drag(move |s: &mut App, d: &mut rune::Drag, tx| s.drag_note(&drag_path, d, tx))
        .on_secondary_press(move |s: &mut App, at, _| s.open_tree_menu(MenuOn::Note(menu_path.clone()), at.x, at.y))
}

/// The Pinned group at the top of the tree: a row with a pin in place of the folder icon, and
/// the pinned notes under it. A note from a folder shows the folder name at the right. A note
/// dropped on the group gets a pin.
fn pins_group(v: V, pinned: &[String]) -> El {
    let p = v.p;
    let s = v.s;
    let open = s.pins_open;
    let target = s.dragging.as_ref().is_some_and(|d| d.active && d.pin);
    let mut head = tree_row(v, "pins-head", "Pinned", 0, true, open, false, Some(&PIN))
        .help(format!("{} pinned {}", pinned.len(), if pinned.len() == 1 { "note" } else { "notes" }))
        .on_press(|s: &mut App| s.toggle_pins_open());
    if target {
        head = head.fill(p.soft).border(1.0, p.acc.alpha(0.45));
    }
    let mut c = col().key(crate::pins::GROUP).w_full().child(head);
    if open {
        for f in pinned {
            let mut r = note_row(v, f, 1);
            let dir = md::dir(f);
            if !dir.is_empty() {
                let name = dir.rsplit('/').next().unwrap_or(dir);
                r = r.child(label(name.to_string(), mono(11.0, 400.0), p.mute).shrink().min_w(0.0).truncate());
            }
            c = c.child(r);
        }
    }
    c
}

/// A row of the tree. `glyph` takes the place of the folder icon, as the Pinned group's pin does.
#[allow(clippy::too_many_arguments)]
fn tree_row(v: V, key: &str, name: &str, depth: usize, folder: bool, open: bool, on: bool, glyph: Option<&'static rune::Icon>) -> El {
    let p = v.p;
    let color = if folder || !on { p.ink2 } else { p.ink };
    let weight = if folder || on { 500.0 } else { 400.0 };
    let chev = el()
        .key("chev")
        .w(12.0)
        .h(12.0)
        .center()
        .opacity(if folder { 1.0 } else { 0.0 })
        .child(icon(&CHEVRON_RIGHT, 8.0).color(p.mute).rotate(if open { std::f32::consts::FRAC_PI_2 } else { 0.0 }).rotate_motion(rune::Spring::new(0.12, 0.0)));
    let dragged = v.s.dragging.as_ref().is_some_and(|d| d.active && key == row_key("file", &d.path));
    let target = folder && v.s.dragging.as_ref().is_some_and(|d| d.active && d.target.as_ref().is_some_and(|t| key == row_key("dir", t)));
    let mut r = row()
        .key(key)
        .h(28.0)
        .mx(8.0)
        .pl(8.0 + depth as f32 * 20.0)
        .pr(if folder { 4.0 } else { 8.0 })
        .gap(6.0)
        .center_y()
        .radius(6.0)
        .hover(p.hover)
        .hover_group()
        .role("treeitem")
        .label(name.to_string())
        .child(chev);
    if on {
        r = r.fill(p.act);
    }
    if target {
        r = r.fill(p.soft).border(1.0, p.acc.alpha(0.45));
    }
    if dragged {
        r = r.opacity(0.5);
    }
    if folder {
        let glyph = glyph.unwrap_or(if open { &FOLDER_OPEN } else { &FOLDER });
        r = r.expanded(open).child(el().key("ficon").size(15.0, 15.0).center().child(icon(glyph, 15.0).color(p.mute)));
    }
    r.child(text(name).font(sans(13.5, weight)).color(color).truncate().grow(1.0).shrink().min_w(0.0))
}

/// The row where a new folder gets its name.
fn new_folder_row(v: V, depth: usize, edit: &rune::TextEdit) -> El {
    let p = v.p;
    row()
        .key("new-folder-row")
        .h(28.0)
        .mx(8.0)
        .pl(8.0 + depth as f32 * 20.0)
        .pr(8.0)
        .gap(6.0)
        .center_y()
        .radius(6.0)
        .child(el().w(12.0).h(12.0))
        .child(el().size(15.0, 15.0).center().child(icon(&FOLDER, 15.0).color(p.mute)))
        .child(
            el().key("new-folder-box").grow(1.0).min_w(0.0).px(4.0).py(1.0).radius(5.0).fill(p.soft).child(
                text_input(edit, sans(13.5, 500.0))
                    .key("new-folder")
                    .single_line()
                    .w_full()
                    .color(p.ink)
                    .placeholder("Folder name", theme::hex("#9a9184"))
                    .caret(p.ink, p.sel)
                    .label("New folder name")
                    .on_edit(|s: &mut App| &mut s.new_folder.get_or_insert_with(|| crate::folders::NewFolder { parent: String::new(), edit: rune::TextEdit::new("") }).edit)
                    .on_submit(|s: &mut App, _| s.commit_new_folder())
                    .on_input_key(|s: &mut App, ev: &rune::KeyEvent, _| {
                        if ev.key == rune::Key::Escape {
                            s.new_folder = None;
                            return true;
                        }
                        false
                    }),
            ),
        )
}

/// The tree's menu, at the pointer: new notes and folders, rename or delete for a note, and
/// delete for a folder.
pub fn tree_menu(v: V) -> El {
    let p = v.p;
    let Some(m) = &v.s.tree_menu else { return el() };
    if let (MenuOn::Folder(f), true) = (&m.on, m.confirm) {
        return confirm_delete_folder(v, f, m.x, m.y);
    }
    type Run = Box<dyn Fn(&mut App, &mut rune::Tx)>;
    let mut items: Vec<(&str, Run)> = Vec::new();
    match m.on.clone() {
        MenuOn::Folder(f) => {
            let (f2, f3) = (f.clone(), f.clone());
            let (mx, my) = (m.x, m.y);
            items.push(("New note here", Box::new(move |s, tx| s.new_note_in(Some(&f), None, tx))));
            items.push(("New folder here", Box::new(move |s, tx| s.start_new_folder(&f2, tx))));
            items.push(("Delete folder", Box::new(move |s, _| s.ask_delete_folder(&f3, mx, my))));
        }
        MenuOn::Note(n) => {
            let (n1, n2, n3, n4) = (n.clone(), n.clone(), n.clone(), n.clone());
            let pin = if v.s.is_pinned(&n) { "Unpin" } else { "Pin" };
            items.push((pin, Box::new(move |s, _| s.toggle_pin(&n4))));
            items.push(("New note here", Box::new(move |s, tx| s.new_note_in(Some(md::dir(&n1)), None, tx))));
            items.push(("Rename", Box::new(move |s, tx| {
                s.tree_menu = None;
                s.open_file(&n2, None, tx);
                s.start_rename(tx);
            })));
            if !md::dir(&n).is_empty() {
                items.push(("Move to folder root", Box::new(move |s, tx| {
                    s.tree_menu = None;
                    s.move_note(&n3, "", tx);
                })));
            }
            items.push(("Delete note", Box::new(move |s, _| {
                s.tree_menu = None;
                s.delete_path(&n);
            })));
        }
        MenuOn::Root => {
            items.push(("New note", Box::new(|s, tx| s.new_note_in(Some(""), None, tx))));
            items.push(("New folder", Box::new(|s, tx| s.start_new_folder("", tx))));
        }
    }
    let w = 200.0;
    let h = 10.0 + 30.0 * items.len() as f32;
    let x = m.x.min(v.vw - w - 8.0).max(8.0);
    let y = m.y.min(v.vh - h - 8.0).max(8.0);
    let [a, b] = theme::menu_shadows();
    let mut panel = col().key("tree-menu").absolute(x, y).w(w).padding(5.0).radius(9.0).fill(p.bg).box_shadow(a).box_shadow(b).role("menu").modal(true, |s: &mut App| s.tree_menu = None);
    for (i, (t, run)) in items.into_iter().enumerate() {
        panel = panel.child(
            row()
                .key(format!("tm-{i}"))
                .w_full()
                .h(30.0)
                .px(10.0)
                .center_y()
                .radius(6.0)
                .hover(p.hover)
                .role("menuitem")
                .label(t)
                .on_press_tx(move |s: &mut App, tx| {
                    s.tree_menu = None;
                    run(s, tx);
                })
                .child(label(t, sans(13.0, 400.0), p.ink)),
        );
    }
    el().key("tree-menu-layer").pin(Some(0.0), Some(0.0), Some(0.0), Some(0.0)).paint_order(55).child(panel)
}

/// The tree menu's second step for a folder with notes: what moves to the trash, and two buttons.
fn confirm_delete_folder(v: V, folder: &str, mx: f32, my: f32) -> El {
    let p = v.p;
    let n = v.s.notes_in(folder).len();
    let name = folder.rsplit('/').next().unwrap_or(folder);
    let msg = format!("Move {name} and its {n} {} to the trash?", if n == 1 { "note" } else { "notes" });
    let w = 260.0;
    let h = 104.0;
    let x = mx.min(v.vw - w - 8.0).max(8.0);
    let y = my.min(v.vh - h - 8.0).max(8.0);
    let [a, b] = theme::menu_shadows();
    let f = folder.to_string();
    let button = |key: &str, t: &str, color: Color| {
        row().key(key).h(30.0).px(12.0).center_y().radius(6.0).hover(p.hover).role("button").label(t).child(label(t, sans(13.0, 500.0), color))
    };
    let panel = col()
        .key("tree-menu")
        .absolute(x, y)
        .w(w)
        .padding(12.0)
        .gap(10.0)
        .radius(9.0)
        .fill(p.bg)
        .box_shadow(a)
        .box_shadow(b)
        .role("alertdialog")
        .label(msg.clone())
        .modal(true, |s: &mut App| s.tree_menu = None)
        .child(label(msg, sans(13.0, 400.0).lh(19.0), p.ink).wrap())
        .child(
            row()
                .gap(4.0)
                .child(el().grow(1.0))
                .child(button("tm-cancel", "Cancel", p.ink2).on_press(|s: &mut App| s.tree_menu = None))
                .child(button("tm-delete", "Delete", p.err).on_press(move |s: &mut App| {
                    s.tree_menu = None;
                    s.delete_folder(&f);
                })),
        );
    el().key("tree-menu-layer").pin(Some(0.0), Some(0.0), Some(0.0), Some(0.0)).paint_order(55).child(panel)
}

/// The note under the pointer while it is dragged to a folder.
pub fn drag_ghost(v: V) -> El {
    let p = v.p;
    let Some(d) = v.s.dragging.as_ref().filter(|d| d.active) else { return el() };
    let [a, b] = theme::menu_shadows();
    let hint = match &d.target {
        _ if d.pin => "to Pinned".to_string(),
        Some(t) if t.is_empty() => "to the folder root".to_string(),
        Some(t) => format!("to {t}/"),
        None => String::new(),
    };
    let mut r = row()
        .key("drag-ghost")
        .absolute(d.x + 14.0, d.y + 10.0)
        .gap(8.0)
        .px(10.0)
        .h(28.0)
        .center_y()
        .radius(7.0)
        .fill(p.bg)
        .box_shadow(a)
        .box_shadow(b)
        .paint_order(70)
        .inert()
        .child(label(md::base(&d.path), sans(13.0, 500.0), p.ink));
    if !hint.is_empty() {
        r = r.child(label(hint, mono(11.0, 400.0), p.acc));
    }
    r
}

fn search_panel(v: V) -> El {
    let p = v.p;
    let s = v.s;
    let q = s.query.text.trim().to_string();
    let mut box_ = row()
        .key("search-box")
        .h(34.0)
        .pl(10.0)
        .pr(8.0)
        .gap(8.0)
        .center_y()
        .border(1.0, p.line)
        .border_layout()
        .radius(7.0)
        .fill(p.bg)
        .child(icon(&SEARCH, 13.0).color(p.mute))
        .child(
            text_input(&s.query, sans(13.0, 400.0))
                .key("search")
                .single_line()
                .px(2.0)
                .py(1.0)
                .grow(1.0)
                .min_w(0.0)
                .color(p.ink)
                .placeholder("Search notes", theme::hex("#9a9184"))
                .caret(p.ink, p.sel)
                .on_edit(|s: &mut App| &mut s.query),
        );
    if !s.query.text.is_empty() {
        box_ = box_.child(
            row()
                .key("search-clear")
                .size(20.0, 20.0)
                .center()
                .radius(5.0)
                .hover(p.hover)
                .label("Clear")
                .help("Clear")
                .on_press_tx(|s: &mut App, tx| {
                    s.query = rune::TextEdit::new("");
                    tx.focus("search", false);
                })
                .child(icon(&CLOSE, 8.0).color(p.mute)),
        );
    }
    let results = if q.is_empty() { Vec::new() } else { md::search(s.files(), &q) };
    let total: usize = results.iter().map(|r| r.hits.len()).sum();
    let summary = if q.is_empty() {
        "Searches every note in this folder".to_string()
    } else if results.is_empty() {
        "No matches".to_string()
    } else {
        format!("{total} {} in {} {}", if total == 1 { "match" } else { "matches" }, results.len(), if results.len() == 1 { "note" } else { "notes" })
    };
    let mut c = col()
        .key("search-panel")
        .w_full()
        .child(col().key("search-top").px(12.0).child(box_).child(text(summary).font(mono(11.0, 400.0)).color(p.mute).pt(10.0).px(2.0).pb(4.0)));
    let ql = q.len();
    for (ri, r) in results.iter().enumerate() {
        let lines: Vec<&str> = s.files().get(&r.path).map(|t| t.split('\n').collect()).unwrap_or_default();
        let first = r.hits.first().map(|h| Sel { start: h.pos, end: h.pos + ql, line: Some(h.line), force_source: true });
        let path = r.path.clone();
        let name_style = sans(13.0, 600.0);
        let small = mono(11.0, 400.0);
        let shift = super::baseline_shift(name_style, small);
        let mut block = col().key(format!("res-{ri}")).py(6.0).child(
            row()
                .key("res-head")
                .gap(8.0)
                .mx(8.0)
                .px(12.0)
                .py(4.0)
                .radius(6.0)
                .hover(p.hover)
                .role("button")
                .on_press_tx(move |s: &mut App, tx| s.open_file(&path, first, tx))
                .child(text(md::base(&r.path)).font(name_style).color(p.ink).truncate().shrink())
                .child(text(md::dir(&r.path)).font(small).color(p.mute).truncate().grow(1.0).shrink().mt(shift))
                .child(label(if r.hits.is_empty() { String::new() } else { r.hits.len().to_string() }, small, p.mute).mt(shift)),
        );
        for (hi, h) in r.hits.iter().take(4).enumerate() {
            let ln = lines.get(h.line).copied().unwrap_or("");
            let a0 = floor_char(ln, h.col.saturating_sub(28));
            let b0 = (h.col + ql).min(ln.len());
            let c1 = floor_char(ln, (b0 + 90).min(ln.len()));
            let pre = format!("{}{}", if a0 > 0 { "…" } else { "" }, ln[a0..h.col].trim_start());
            // `<mark>` has 1 px of padding on each side: a mono space 1 px wide takes it.
            let text_ = format!("{pre} {} {}", &ln[h.col..b0], &ln[b0..c1]);
            let (m0, m1) = (pre.len(), pre.len() + 1 + (b0 - h.col) + 1);
            let pad = Span { mono: true, size: Some(1.0 / 0.6), bg: Some(p.soft), ..Span::default() };
            let spans = vec![
                Span { start: m0 as u32, end: (m0 + 1) as u32, ..pad },
                Span { bg: Some(p.soft), color: Some(p.ink), ..Span::new(m0 + 1, m1 - 1) },
                Span { start: (m1 - 1) as u32, end: m1 as u32, ..pad },
            ];
            let path = r.path.clone();
            let sel = Sel { start: h.pos, end: h.pos + ql, line: Some(h.line), force_source: true };
            block = block.child(
                el().key(format!("hit-{hi}")).my(1.0).mx(8.0).px(12.0).py(4.0).radius(6.0).hover(p.hover).role("button").on_press_tx(move |s: &mut App, tx| s.open_file(&path, Some(sel), tx)).child(
                    rich(text_, spans).font(sans(12.5, 400.0).lh(12.5 * 1.45)).color(p.ink2).wrap().wrap_anywhere().w_full(),
                ),
            );
        }
        if r.hits.len() > 4 {
            block = block.child(text(format!("+ {} more", r.hits.len() - 4)).font(mono(11.0, 400.0)).color(p.mute).py(2.0).px(20.0));
        }
        c = c.child(block);
    }
    c
}

fn floor_char(s: &str, mut i: usize) -> usize {
    while i > 0 && !s.is_char_boundary(i) {
        i -= 1;
    }
    i
}

fn tags_panel(v: V) -> El {
    let p = v.p;
    let tags = md::tags(v.s.files());
    let mut c = col().key("tags").w_full();
    if tags.is_empty() {
        return c.child(label("No tags yet. Write #something in a note.", sans(13.0, 400.0), p.mute).py(6.0).px(22.0).wrap());
    }
    for (tag, n) in tags {
        let t = tag.clone();
        c = c.child(
            row()
                .key(format!("tag-{tag}"))
                .h(28.0)
                .mx(8.0)
                .px(10.0)
                .center_y()
                .justify_between()
                .radius(6.0)
                .hover(p.hover)
                .role("button")
                .on_press(move |s: &mut App| s.search_tag(&t))
                .child(label(format!("#{tag}"), mono(12.5, 400.0), p.acc))
                .child(label(n.to_string(), mono(11.0, 400.0), p.mute)),
        );
    }
    c
}
