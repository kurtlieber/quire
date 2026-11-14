//! Settings: Sync & backup. "Test connection" checks the form, then reaches the store; once it
//! passes, notes sync on save, every 5 minutes or by hand.

use rune::{Color, El, TextEdit, col, row, text, text_input};

use super::V;
use super::settings::{block, body_width, desc_text, row_};
use super::widgets::{label, primary_button, seg, settings_seg, switch};
use crate::app::{App, SyncStatus};
use crate::theme::{self, mono, sans};

pub struct Provider {
    pub id: &'static str,
    pub name: &'static str,
    pub full: &'static str,
    pub kind: &'static str,
    pub regions: &'static [(&'static str, &'static str)],
}

pub const PROVIDERS: [Provider; 12] = [
    Provider { id: "none", name: "Local only", full: "", kind: "no sync", regions: &[] },
    Provider { id: "s3", name: "Amazon S3", full: "Amazon S3", kind: "S3", regions: &[("us-east-1", "us-east-1"), ("us-west-2", "us-west-2"), ("eu-west-1", "eu-west-1"), ("eu-central-1", "eu-central-1"), ("ap-southeast-1", "ap-southeast-1")] },
    Provider { id: "hetzner", name: "Hetzner", full: "Hetzner Object Storage", kind: "Object Storage · S3", regions: &[("fsn1", "fsn1 · Falkenstein"), ("nbg1", "nbg1 · Nuremberg"), ("hel1", "hel1 · Helsinki")] },
    Provider { id: "azure", name: "Azure", full: "Azure Blob Storage", kind: "Blob Storage", regions: &[] },
    Provider { id: "gcs", name: "Google Cloud", full: "Google Cloud Storage", kind: "Cloud Storage", regions: &[] },
    Provider { id: "r2", name: "Cloudflare R2", full: "Cloudflare R2", kind: "S3-compatible", regions: &[] },
    Provider { id: "b2", name: "Backblaze B2", full: "Backblaze B2", kind: "S3-compatible", regions: &[("us-west-004", "us-west-004"), ("us-east-005", "us-east-005"), ("eu-central-003", "eu-central-003")] },
    Provider { id: "do", name: "DigitalOcean", full: "DigitalOcean Spaces", kind: "Spaces · S3", regions: &[("ams3", "ams3"), ("fra1", "fra1"), ("nyc3", "nyc3"), ("sfo3", "sfo3"), ("sgp1", "sgp1")] },
    Provider { id: "wasabi", name: "Wasabi", full: "Wasabi", kind: "S3-compatible", regions: &[("us-east-1", "us-east-1"), ("eu-central-1", "eu-central-1"), ("ap-northeast-1", "ap-northeast-1")] },
    Provider { id: "scaleway", name: "Scaleway", full: "Scaleway Object Storage", kind: "S3-compatible", regions: &[("fr-par", "fr-par"), ("nl-ams", "nl-ams"), ("pl-waw", "pl-waw")] },
    Provider { id: "ovh", name: "OVHcloud", full: "OVHcloud Object Storage", kind: "S3-compatible", regions: &[("gra", "gra"), ("sbg", "sbg"), ("de", "de"), ("uk", "uk"), ("waw", "waw"), ("bhs", "bhs")] },
    Provider { id: "custom", name: "Other", full: "S3-compatible server", kind: "MinIO, Garage, Ceph…", regions: &[] },
];

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Text,
    Secret,
    Choice,
    Area,
    Read,
}

pub struct Field {
    pub key: &'static str,
    pub label: &'static str,
    pub kind: Kind,
    pub wide: bool,
    pub req: bool,
    pub def: String,
    pub hint: &'static str,
    pub ph: &'static str,
    pub opts: Vec<(&'static str, &'static str)>,
    pub value: String,
}

fn f(key: &'static str, label: &'static str) -> Field {
    Field { key, label, kind: Kind::Text, wide: false, req: true, def: String::new(), hint: "", ph: "", opts: Vec::new(), value: String::new() }
}

impl Field {
    fn kind(mut self, k: Kind) -> Self {
        self.kind = k;
        self
    }
    fn wide(mut self) -> Self {
        self.wide = true;
        self
    }
    fn opt(mut self) -> Self {
        self.req = false;
        self
    }
    fn def(mut self, d: impl Into<String>) -> Self {
        self.def = d.into();
        self
    }
    fn ph(mut self, p: &'static str) -> Self {
        self.ph = p;
        self
    }
    fn hint(mut self, h: &'static str) -> Self {
        self.hint = h;
        self
    }
    fn opts(mut self, o: Vec<(&'static str, &'static str)>) -> Self {
        self.opts = o;
        self
    }
    fn value(mut self, v: String) -> Self {
        self.value = v;
        self
    }
}

pub fn provider(id: &str) -> &'static Provider {
    PROVIDERS.iter().find(|p| p.id == id).unwrap_or(&PROVIDERS[0])
}

/// The form for one provider, as the design's `syncSpec` builds it.
pub fn spec(s: &App, pid: &str) -> Vec<Field> {
    let cfg = s.settings().sync.cfg.get(pid).cloned().unwrap_or_default();
    let val = |k: &str, d: &str| cfg.get(k).filter(|v| !v.is_empty()).cloned().unwrap_or_else(|| d.to_string());
    let vault = s.vault.as_ref().map_or(String::new(), |v| v.name.clone());
    let prefix = || f("prefix", "Remote folder").opt().def(format!("quire/{vault}/")).hint("inside the bucket");
    let mut out = Vec::new();
    match pid {
        "none" => return out,
        "azure" => {
            out.push(f("account", "Storage account").ph("mynotesstorage"));
            out.push(f("container", "Container").ph("notes"));
            out.push(f("azAuth", "Sign in with").kind(Kind::Choice).wide().opt().def("key").opts(vec![("key", "account key"), ("sas", "SAS token")]));
            out.push(f("azKey", if val("azAuth", "key") == "sas" { "SAS token" } else { "Account key" }).kind(Kind::Secret).wide());
            out.push(prefix());
            let ep = format!("https://{}.blob.core.windows.net/{}", val("account", "<account>"), val("container", "<container>"));
            out.push(f("ep", "Endpoint").kind(Kind::Read).wide().opt().value(ep));
            return out;
        }
        "gcs" => {
            out.push(f("bucket", "Bucket").ph("my-notes"));
            out.push(prefix());
            out.push(f("gcsAuth", "Sign in with").kind(Kind::Choice).wide().opt().def("sa").opts(vec![("sa", "service account key"), ("hmac", "HMAC keys")]));
            if val("gcsAuth", "sa") == "sa" {
                out.push(f("gcsJson", "Service account key").kind(Kind::Area).wide().hint("paste the JSON key file").ph("{ \"type\": \"service_account\", \"project_id\": \"…\" }"));
            } else {
                out.push(f("hmacId", "Access ID"));
                out.push(f("hmacSecret", "Secret").kind(Kind::Secret));
            }
            let ep = format!("https://storage.googleapis.com/{}", val("bucket", "<bucket>"));
            out.push(f("ep", "Endpoint").kind(Kind::Read).wide().opt().value(ep));
            return out;
        }
        _ => {}
    }
    let pv = provider(pid);
    if !pv.regions.is_empty() {
        out.push(f("region", "Region").kind(Kind::Choice).wide().opt().opts(pv.regions.to_vec()).def(pv.regions[0].0));
    }
    if pid == "r2" {
        out.push(f("accountId", "Account ID").wide().ph("from the R2 overview page"));
    }
    if pid == "custom" {
        out.push(f("endpoint", "Endpoint").wide().ph("https://minio.example.com:9000"));
        out.push(f("region", "Region").opt().def("us-east-1"));
        out.push(f("addressing", "Addressing").kind(Kind::Choice).opt().def("path").opts(vec![("path", "path-style"), ("vhost", "virtual-hosted")]));
    }
    out.push(f("bucket", if pid == "do" { "Space name" } else { "Bucket" }).ph("my-notes"));
    out.push(prefix());
    out.push(f("accessKey", if pid == "b2" { "Key ID" } else { "Access key ID" }));
    out.push(f("secretKey", if pid == "b2" { "Application key" } else { "Secret access key" }).kind(Kind::Secret));
    let region = val("region", pv.regions.first().map_or("", |r| r.0));
    let ep = match pid {
        "s3" => Some(format!("https://s3.{region}.amazonaws.com")),
        "hetzner" => Some(format!("https://{region}.your-objectstorage.com")),
        "r2" => Some(format!("https://{}.r2.cloudflarestorage.com", val("accountId", "<account-id>"))),
        "b2" => Some(format!("https://s3.{region}.backblazeb2.com")),
        "do" => Some(format!("https://{region}.digitaloceanspaces.com")),
        "wasabi" => Some(format!("https://s3.{region}.wasabisys.com")),
        "scaleway" => Some(format!("https://s3.{region}.scw.cloud")),
        "ovh" => Some(format!("https://s3.{region}.io.cloud.ovh.net")),
        _ => None,
    };
    if let Some(ep) = ep {
        out.push(f("ep", "Endpoint").kind(Kind::Read).wide().opt().value(ep));
    }
    out
}

fn cfg_value(s: &App, pid: &str, key: &str) -> Option<String> {
    if key == "prefix" {
        return Some(s.remote_folder());
    }
    s.settings().sync.cfg.get(pid).and_then(|c| c.get(key)).cloned()
}

/// The text field's key. The remote folder belongs to the open notes folder, so its field does too.
fn field_key(s: &App, pid: &str, key: &str) -> String {
    match (key, &s.vault) {
        ("prefix", Some(v)) => format!("sync:prefix:{}", v.path.display()),
        _ => format!("sync:{pid}:{key}"),
    }
}

pub fn set_cfg(s: &mut App, key: &str, v: String) {
    let pid = s.settings().sync.provider.clone();
    if key == "prefix" {
        if let Some(path) = s.vault.as_ref().map(|v| v.path.to_string_lossy().into_owned()) {
            s.settings_mut().sync.folders.insert(path, v);
        }
    } else {
        s.settings_mut().sync.cfg.entry(pid).or_default().insert(key.to_string(), v);
    }
    s.sync_changed();
    s.sync_status = match std::mem::replace(&mut s.sync_status, SyncStatus::Idle) {
        SyncStatus::Error { msg, missing } => SyncStatus::Error { msg, missing: missing.into_iter().filter(|m| m != key).collect() },
        _ => SyncStatus::Idle,
    };
}

pub fn test_sync(s: &mut App) {
    let pid = s.settings().sync.provider.clone();
    let fields = spec(s, &pid);
    let missing: Vec<&Field> = fields
        .iter()
        .filter(|d| d.req && cfg_value(s, &pid, d.key).filter(|v| !v.trim().is_empty()).is_none() && d.def.trim().is_empty())
        .collect();
    if !missing.is_empty() {
        let names: Vec<String> = missing.iter().map(|d| d.label.to_lowercase()).collect();
        s.sync_status = SyncStatus::Error { msg: format!("Fill in {}.", names.join(", ")), missing: missing.iter().map(|d| d.key.to_string()).collect() };
        return;
    }
    if s.settings().sync.encrypt && s.settings().sync.pass.chars().count() < 12 {
        s.sync_status = SyncStatus::Error { msg: "The passphrase needs at least 12 characters.".into(), missing: Vec::new() };
        return;
    }
    s.start_test();
}

pub fn sync(v: V) -> El {
    let p = v.p;
    let s = v.s;
    let sy = &s.settings().sync;
    let pid = sy.provider.clone();
    let w = body_width(v);
    let cols = (((w + 8.0) / 158.0).floor() as usize).max(1);
    let card_w = (w - 8.0 * (cols as f32 - 1.0)) / cols as f32;
    let mut grid = col().key("providers").w_full().gap(8.0).mt(14.0);
    let mut line = row().key("prow-0").w_full().gap(8.0);
    for (i, pv) in PROVIDERS.iter().enumerate() {
        if i > 0 && i % cols == 0 {
            grid = grid.child(line);
            line = row().key(format!("prow-{}", i / cols)).w_full().gap(8.0);
        }
        let on = pv.id == pid;
        let id = pv.id;
        let mut card = col()
            .key(format!("pv-{id}"))
            .w(card_w)
            .gap(3.0)
            .py(10.0)
            .px(12.0)
            .radius(9.0)
            .fill(p.bg)
            .border(1.0, if on { p.acc } else { p.line })
            .border_layout()
            .role("radio")
            .checked(on)
            .label(pv.name)
            .on_press(move |s: &mut App| {
                if s.settings().sync.provider != id {
                    s.settings_mut().sync.provider = id.into();
                    s.sync_changed();
                    s.sync_status = SyncStatus::Idle;
                }
            })
            .child(text(pv.name).font(sans(13.0, 500.0)).color(p.ink).truncate().max_w(card_w - 26.0))
            .child(text(pv.kind).font(mono(10.5, 400.0)).color(if on { p.acc } else { p.mute }).truncate().max_w(card_w - 26.0));
        if on {
            card = card.box_shadow(rune::Shadow { x: 0.0, y: 0.0, blur: 0.0, spread: 1.0, color: p.acc, inset: false });
        }
        line = line.child(card);
    }
    grid = grid.child(line);
    let mut c = col().key("sync-set").w_full().child(
        block(v, "backend", "Storage backend", Some(&crate::md::pretty("Notes always live on this computer first. A backend keeps a copy in object storage you control."))).child(grid),
    );
    if pid == "none" {
        return c;
    }
    let pv = provider(&pid);
    let missing: Vec<String> = match &s.sync_status {
        SyncStatus::Error { missing, .. } => missing.clone(),
        _ => Vec::new(),
    };
    // The design's two-column grid: wide fields take a row, others pair up.
    let mut form = col().key("fields").w_full().gap(14.0).mt(14.0);
    let mut pending: Vec<El> = Vec::new();
    let fields = spec(s, &pid);
    let half = (w - 12.0) / 2.0;
    for fd in &fields {
        let fe = field(v, &pid, fd, missing.contains(&fd.key.to_string()));
        if fd.wide {
            if !pending.is_empty() {
                form = form.child(row().w_full().gap(12.0).children(pending.drain(..)));
            }
            form = form.child(fe.w_full());
        } else {
            pending.push(fe.w(half));
            if pending.len() == 2 {
                form = form.child(row().w_full().gap(12.0).children(pending.drain(..)));
            }
        }
    }
    if !pending.is_empty() {
        form = form.child(row().w_full().gap(12.0).children(pending.drain(..)));
    }
    c = c.child(col().key("provider-form").w_full().pt(16.0).pb(18.0).border_sides([1.0, 0.0, 0.0, 0.0], p.line).border_layout().child(label(pv.full, sans(14.0, 500.0), p.ink)).child(form));
    let when = |id: &'static str, l: &str| seg(l, sy.when == id, move |s, _| s.settings_mut().sync.when = id.into());
    c = c.child(row_(v, "when", "When to sync", Some("On save uploads a note a moment after you stop typing."), settings_seg(p, "when-seg", vec![when("save", "On save"), when("interval", "Every 5 min"), when("manual", "Manually")])));
    let enc = sy.encrypt;
    c = c.child(row_(
        v,
        "encrypt",
        "Encrypt before upload",
        Some("Notes are encrypted on this computer, so the provider only stores ciphertext."),
        switch(p, "enc-switch", enc, "Encrypt before upload", |s, _| {
            let e = s.settings().sync.encrypt;
            s.settings_mut().sync.encrypt = !e;
            s.sync_changed();
        }),
    ));
    if enc {
        let pass = sy.pass.clone();
        c = c.child(
            col()
                .key("pass")
                .w_full()
                .gap(6.0)
                .pb(16.0)
                .child(label("Passphrase", sans(12.5, 500.0), p.ink2))
                .child(text_box(v, "pass", pass, "At least 12 characters", true, false, 320.0, |s, t| {
                    s.settings_mut().sync.pass = t;
                    s.sync_changed();
                }))
                .child(label("If you lose this passphrase, the remote copy can't be recovered.", sans(12.0, 400.0), p.mute)),
        );
    }
    let (msg, color) = match &s.sync_status {
        SyncStatus::Idle if sy.connected && s.sync.error.is_some() => (format!("The last sync failed. {}", s.sync.error.clone().unwrap_or_default()), p.err),
        SyncStatus::Idle if sy.connected => (
            match sy.last_sync {
                Some(t) => format!("Connected. Last synced {}.", crate::sync::app::ago(t)),
                None => "Connected. Not synced yet.".to_string(),
            },
            p.mute,
        ),
        SyncStatus::Idle => ("Not connected yet.".to_string(), p.mute),
        SyncStatus::Testing { msg } => (msg.clone(), p.mute),
        SyncStatus::Error { msg, .. } => (msg.clone(), p.err),
        SyncStatus::Ok { msg } => (msg.clone(), p.ink2),
    };
    let working = s.sync.testing() || s.sync.busy();
    c = c.child(
        row()
            .key("sync-actions")
            .w_full()
            .flex_wrap()
            .gap(10.0)
            .pt(16.0)
            .pb(4.0)
            .center_y()
            .border_sides([1.0, 0.0, 0.0, 0.0], p.line)
            .border_layout()
            .child(primary_button(p, "test", if s.sync.testing() { "Working…" } else { "Test connection" }, 32.0, 14.0, 13.0, |s, _| {
                if !s.sync.testing() {
                    test_sync(s);
                }
            }))
            .children(if sy.connected && !working {
                vec![super::widgets::outline_button(p, "sync-now", "Sync now", 32.0, |s, _| s.sync_now()).px(14.0)]
            } else {
                Vec::new()
            })
            .child(text(msg).font(sans(12.5, 400.0).lh(12.5 * 1.45)).color(color).wrap().grow(1.0).min_w(180.0)),
    );
    c.child(label("Keys are saved in Typo's settings file, never inside the notes folder.", mono(11.0, 400.0).lh(16.5), p.mute).pt(14.0).wrap())
}

fn field(v: V, pid: &str, fd: &Field, bad: bool) -> El {
    let p = v.p;
    let s = v.s;
    let value = if fd.kind == Kind::Read { fd.value.clone() } else { cfg_value(s, pid, fd.key).unwrap_or_else(|| fd.def.clone()) };
    let head = row()
        .key("head")
        .w_full()
        .gap(8.0)
        .justify_between()
        .child(label(fd.label, sans(12.5, 500.0), p.ink2))
        .child(text(fd.hint).font(sans(11.5, 400.0)).color(p.mute).truncate().shrink().min_w(0.0).mt(super::baseline_shift(sans(12.5, 500.0), sans(11.5, 400.0))));
    let mut c = col().key(format!("f-{}", fd.key)).gap(6.0).min_w(0.0).child(head);
    let key = fd.key;
    match fd.kind {
        Kind::Text | Kind::Secret => {
            c = c.child(text_box(v, &field_key(s, pid, key), value, fd.ph, fd.kind == Kind::Secret, bad, f32::INFINITY, move |s, t| set_cfg(s, key, t)));
        }
        Kind::Choice => {
            let mut r = row().key("opts").w_full().flex_wrap().gap(6.0);
            for (i, (ov, ol)) in fd.opts.iter().enumerate() {
                let on = value == *ov;
                let ov = *ov;
                r = r.child(
                    row()
                        .key(format!("o-{i}"))
                        .h(28.0)
                        .px(10.0)
                        .center()
                        .radius(7.0)
                        .border(1.0, if on { p.acc } else { p.line })
                        .border_layout()
                        .role("radio")
                        .checked(on)
                        .on_press(move |s: &mut App| set_cfg(s, key, ov.into()))
                        .child(label(*ol, mono(11.5, 400.0), if on { p.acc } else { p.ink2 })),
                );
            }
            c = c.child(r);
        }
        Kind::Area => {
            let fk = format!("sync:{pid}:{key}");
            let edit = s.fields.get(&fk).cloned().unwrap_or_else(|| TextEdit::new(value.clone()));
            let init = value.clone();
            let fk2 = fk.clone();
            c = c.child(
                col().key("area")
                    .w_full()
                    .h(84.0)
                    .py(8.0)
                    .px(10.0)
                    .radius(7.0)
                    .fill(p.bg)
                    .border(1.0, if bad { p.err } else { p.line })
                    .border_layout()
                    .scroll(rune::ScrollSpec { stick_bottom: false })
                    .child(
                        text_input(&edit, mono(11.5, 400.0).lh(11.5 * 1.5))
                            .key(fk.clone())
                            .w_full()
                            .color(p.ink)
                            .placeholder(fd.ph, theme::hex("#9a9184"))
                            .caret(p.ink, p.sel)
                            .on_edit(move |s: &mut App| s.field(&fk, || init.clone()))
                            .on_submit(|s: &mut App, _| {
                                let _ = s;
                            })
                            .on_input_change(move |s: &mut App, _| {
                                let t = s.fields.get(&fk2).map(|e| e.text.clone()).unwrap_or_default();
                                set_cfg(s, key, t);
                            }),
                    ),
            );
        }
        Kind::Read => {
            c = c.child(text(value).font(mono(12.0, 400.0)).color(p.ink2).wrap().wrap_anywhere().w_full().py(7.0).px(10.0).radius(7.0).fill(p.code));
        }
    }
    c
}

/// A 32-point single-line field with an optional show and hide button for secrets.
pub fn text_box(v: V, fk: &str, value: String, ph: &'static str, secret: bool, bad: bool, max_w: f32, set: impl Fn(&mut App, String) + 'static) -> El {
    let p = v.p;
    let s = v.s;
    let revealed = s.reveal.contains(fk);
    let mut edit = s.fields.get(fk).cloned().unwrap_or_else(|| TextEdit::new(value.clone()));
    if secret && !revealed {
        edit = edit.secure();
    }
    let fk_edit = fk.to_string();
    let fk_change = fk.to_string();
    let init = value.clone();
    let mut b = row()
        .key(format!("box-{fk}"))
        .w_full()
        .h(32.0)
        .center_y()
        .radius(7.0)
        .fill(p.bg)
        .border(1.0, if bad { p.err } else { p.line })
        .border_layout()
        .clip_rounded(7.0)
        .child(
            text_input(&edit, mono(12.5, 400.0))
                .key(fk.to_string())
                .single_line()
                .grow(1.0)
                .min_w(0.0)
                .px(10.0)
                .color(p.ink)
                .placeholder(ph, theme::hex("#9a9184"))
                .caret(p.ink, p.sel)
                .on_edit(move |s: &mut App| s.field(&fk_edit, || init.clone()))
                .on_input_change(move |s: &mut App, _| {
                    let t = s.fields.get(&fk_change).map(|e| e.text.clone()).unwrap_or_default();
                    set(s, t);
                }),
        );
    if max_w.is_finite() {
        b = b.max_w(max_w);
    }
    if secret {
        let rk = fk.to_string();
        b = b.child(
            row()
                .key("reveal")
                .h_full()
                .px(10.0)
                .center()
                .border_sides([0.0, 0.0, 0.0, 1.0], p.line)
                .border_layout()
                .hover_group()
                .role("button")
                .on_press(move |s: &mut App| {
                    if !s.reveal.remove(&rk) {
                        s.reveal.insert(rk.clone());
                    }
                    if let Some(e) = s.fields.get_mut(&rk) {
                        e.set_secure(!s.reveal.contains(&rk));
                    }
                })
                .child(label(if revealed { "hide" } else { "show" }, mono(11.0, 400.0), p.mute).group_color(p.ink)),
        );
    }
    let _ = (desc_text, Color::TRANSPARENT);
    b
}
