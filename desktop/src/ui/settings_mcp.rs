//! Settings: MCP. `typo mcp --folder <path>` is the stdio server an AI app starts; the HTTP
//! transport runs inside this window while it is open.

use rune::{El, col, el, icon, row, text};

use super::V;
use super::settings::{block, row_};
use super::settings_sync::text_box;
use super::widgets::{CHECK, label, outline_button, seg, settings_seg, settings_seg_mono, switch};
use crate::app::App;
use crate::store::{MCP_TOOLS, new_token};
use crate::theme::{mono, sans};

pub fn config_json(v: V) -> String {
    let s = v.s;
    let m = &s.settings().mcp;
    let path = s.vault.as_ref().map_or_else(String::new, |x| x.path.to_string_lossy().into_owned());
    let exe = std::env::current_exe().ok().map_or_else(|| "typo".to_string(), |p| p.to_string_lossy().into_owned());
    let server = if m.transport == "http" {
        serde_json::json!({ "url": url(v), "headers": { "Authorization": format!("Bearer {}", m.token) } })
    } else {
        serde_json::json!({ "command": exe, "args": ["mcp", "--folder", path] })
    };
    serde_json::to_string_pretty(&serde_json::json!({ "mcpServers": { "typo": server } })).unwrap_or_default()
}

fn url(v: V) -> String {
    format!("http://127.0.0.1:{}/mcp", v.s.settings().mcp.port)
}

pub fn mcp(v: V) -> El {
    let p = v.p;
    let s = v.s;
    let m = &s.settings().mcp;
    let mut c = col().key("mcp-set").w_full().child(row_(
        v,
        "mcp-on",
        "MCP server",
        Some(&crate::md::pretty("Lets AI apps that speak the Model Context Protocol, such as Claude Desktop or Cursor, work with the notes in this folder.")),
        switch(p, "mcp-switch", m.enabled, "MCP server", |s, _| {
            let e = s.settings().mcp.enabled;
            s.settings_mut().mcp.enabled = !e;
        }),
    ));
    if !m.enabled {
        return c;
    }
    let http = m.transport == "http";
    let tr = |id: &'static str, l: &str| seg(l, m.transport == id, move |s, _| s.settings_mut().mcp.transport = id.into());
    c = c.child(row_(
        v,
        "transport",
        "Transport",
        Some("With stdio, the AI app starts Typo's helper itself. HTTP serves a local endpoint on this computer."),
        settings_seg_mono(p, "tr-seg", vec![tr("stdio", "stdio"), tr("http", "HTTP")]),
    ));
    if http {
        let port = m.port.clone();
        let token = m.token.clone();
        let grid = col()
            .key("http")
            .w_full()
            .gap(14.0)
            .pb(18.0)
            .child(
                row()
                    .key("http-top")
                    .w_full()
                    .gap(12.0)
                    .child(col().key("port").w(110.0).gap(6.0).child(label("Port", sans(12.5, 500.0), p.ink2)).child(text_box(v, "mcp-port", port, "", false, false, f32::INFINITY, |s, t| {
                        let digits: String = t.chars().filter(|c| c.is_ascii_digit()).take(5).collect();
                        s.settings_mut().mcp.port = digits;
                    })))
                    .child(
                        col().key("addr").grow(1.0).min_w(0.0).gap(6.0).child(label("Address", sans(12.5, 500.0), p.ink2)).child(
                            row().key("addr-box").h(32.0).w_full().px(10.0).center_y().radius(7.0).fill(p.code).child(text(url(v)).font(mono(12.0, 400.0)).color(p.ink2).truncate().shrink().min_w(0.0)),
                        ),
                    ),
            )
            .child(
                col()
                    .key("token")
                    .w_full()
                    .gap(6.0)
                    .child(
                        row()
                            .key("token-head")
                            .w_full()
                            .gap(8.0)
                            .justify_between()
                            .child(label("Access token", sans(12.5, 500.0), p.ink2))
                            .child(label("sent as a Bearer header", sans(11.5, 400.0), p.mute).mt(super::baseline_shift(sans(12.5, 500.0), sans(11.5, 400.0)))),
                    )
                    .child(
                        row()
                            .key("token-row")
                            .w_full()
                            .gap(8.0)
                            .child(token_box(v, &token))
                            .child(outline_button(p, "regen", "Regenerate", 32.0, |s, _| {
                                s.settings_mut().mcp.token = new_token();
                                s.fields.remove("mcp-token");
                            })),
                    ),
            );
        c = c.child(grid);
    }
    let access = |id: &'static str, l: &str| seg(l, (if m.access == "write" { "write" } else { "read" }) == id, move |s, _| s.settings_mut().mcp.access = id.into());
    c = c.child(row_(v, "access", "Access", Some("Read only turns off every tool that changes files."), settings_seg(p, "access-seg", vec![access("read", "Read only"), access("write", "Read & write")])));

    let ro = m.access != "write";
    let mut tools = col().key("tools").w_full().mt(8.0);
    let mut n_on = 0;
    for (id, desc, write) in MCP_TOOLS {
        let avail = !(write && ro);
        let on = avail && m.tools.get(id).copied().unwrap_or(false);
        if on {
            n_on += 1;
        }
        let mut bx = row().key("box").size(16.0, 16.0).center().radius(4.0).border(1.5, if on { p.button } else { p.mark }).border_layout();
        if on {
            bx = bx.fill(p.button).child(icon(&CHECK, 10.0).color(p.on_acc));
        }
        let mut r = row()
            .key(format!("tool-{id}"))
            .w_full()
            .gap(12.0)
            .py(7.0)
            .center_y()
            .opacity(if avail { 1.0 } else { 0.45 })
            .role("checkbox")
            .checked(on)
            .label(id)
            .on_press(move |s: &mut App| {
                if avail {
                    let cur = s.settings().mcp.tools.get(id).copied().unwrap_or(false);
                    s.settings_mut().mcp.tools.insert(id.into(), !cur);
                }
            })
            .child(bx)
            .child(label(id, mono(12.5, 400.0), p.ink).w(128.0))
            .child(text(desc).font(sans(12.5, 400.0)).color(p.ink2).grow(1.0).shrink().min_w(0.0).truncate());
        if write {
            r = r.child(super::widgets::padded("writes", mono(10.5, 400.0), p.acc, 6.0, 2.0, 0.0).radius(4.0).fill(p.soft));
        }
        tools = tools.child(r);
    }
    c = c.child(block(v, "tools-block", "Tools", Some("What connected apps are allowed to call.")).pb(10.0).child(tools));

    let hidden = &m.hidden;
    let mut folders = row().key("mcp-folders").w_full().flex_wrap().gap(6.0).mt(12.0);
    let mut list = vec![(String::new(), "root notes".to_string())];
    list.extend(s.folder_list().into_iter().map(|f| (f.clone(), format!("{f}/"))));
    for (i, (val, lbl)) in list.into_iter().enumerate() {
        let on = !hidden.contains(&val);
        folders = folders.child(
            row()
                .key(format!("mf-{i}"))
                .h(28.0)
                .px(10.0)
                .center()
                .radius(7.0)
                .border(1.0, if on { p.acc } else { p.line })
                .border_layout()
                .role("checkbox")
                .checked(on)
                .on_press(move |s: &mut App| {
                    let h = &mut s.settings_mut().mcp.hidden;
                    if let Some(k) = h.iter().position(|x| *x == val) {
                        h.remove(k);
                    } else {
                        h.push(val.clone());
                    }
                })
                .child({
                    let mut t = rune::rich(lbl.clone(), Vec::new()).font(mono(12.0, 400.0)).color(if on { p.acc } else { p.mute });
                    if !on {
                        t = t.strike(0, lbl.len(), Some(p.mute));
                    }
                    t
                }),
        );
    }
    c = c.child(block(v, "visible", "Folders apps can see", Some("Hidden folders don't show up in listings or search results.")).child(folders));
    if !ro {
        c = c.child(row_(
            v,
            "confirm",
            "Ask before changes",
            Some("Typo shows the change and waits for you to approve it."),
            switch(p, "confirm-switch", m.confirm, "Ask before changes", |s, _| {
                let e = s.settings().mcp.confirm;
                s.settings_mut().mcp.confirm = !e;
            }),
        ));
    }
    let cfg = config_json(v);
    let copied = s.mcp_copied;
    let status = if http {
        match &s.mcp_http_error {
            Some(e) => e.clone(),
            None => format!("ready on 127.0.0.1:{} · {n_on} {} · waiting for a client", m.port, if n_on == 1 { "tool" } else { "tools" }),
        }
    } else {
        format!("ready on stdio · {n_on} {} · waiting for a client", if n_on == 1 { "tool" } else { "tools" })
    };
    let cfg_copy = cfg.clone();
    c.child(
        col()
            .key("client")
            .w_full()
            .pt(16.0)
            .pb(4.0)
            .border_sides([1.0, 0.0, 0.0, 0.0], p.line)
            .border_layout()
            .child(
                row()
                    .key("client-head")
                    .w_full()
                    .gap(12.0)
                    .center_y()
                    .justify_between()
                    .child(col().key("text").min_w(0.0).child(label("Client configuration", sans(14.0, 500.0), p.ink)).child(super::settings::desc_text(v, "Paste this into the AI app's MCP settings.")))
                    .child(outline_button(p, "copy", if copied { "Copied" } else { "Copy" }, 30.0, move |s, tx| {
                        tx.copy(cfg_copy.clone());
                        s.mcp_copied = true;
                        tx.wake_after(std::time::Duration::from_millis(1600));
                        s.mcp_copied_at = Some(std::time::Instant::now());
                    })),
            )
            .child(el().key("cfg").w_full().mt(12.0).radius(8.0).fill(p.code).scroll_x().child(text(cfg).font(mono(11.5, 400.0).lh(11.5 * 1.6)).color(p.ink2).py(12.0).px(14.0)))
            .child(
                row()
                    .key("mcp-status")
                    .gap(8.0)
                    .mt(12.0)
                    .center_y()
                    .child(el().size(6.0, 6.0).radius(3.0).fill(p.acc))
                    .child(label(status, mono(11.0, 400.0), p.mute)),
            ),
    )
}

fn token_box(v: V, token: &str) -> El {
    let p = v.p;
    let s = v.s;
    let revealed = s.reveal.contains("mcp-token");
    let shown = if revealed { token.to_string() } else { "•".repeat(token.chars().count()) };
    row()
        .key("token-box")
        .grow(1.0)
        .min_w(0.0)
        .h(32.0)
        .center_y()
        .radius(7.0)
        .fill(p.bg)
        .border(1.0, p.line)
        .border_layout()
        .clip_rounded(7.0)
        .child(text(shown).font(mono(12.5, 400.0)).color(p.ink).selectable(revealed).truncate().grow(1.0).shrink().min_w(0.0).px(10.0))
        .child(
            row()
                .key("reveal")
                .h_full()
                .px(10.0)
                .center()
                .border_sides([0.0, 0.0, 0.0, 1.0], p.line)
                .border_layout()
                .hover_group()
                .role("button")
                .on_press(|s: &mut App| {
                    if !s.reveal.remove("mcp-token") {
                        s.reveal.insert("mcp-token".into());
                    }
                })
                .child(label(if revealed { "hide" } else { "show" }, mono(11.0, 400.0), p.mute).group_color(p.ink)),
        )
}
