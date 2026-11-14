//! A Model Context Protocol server for one notes folder.
//!
//! `typo mcp --folder <path>` serves it over stdio, one JSON-RPC message per line. While the
//! app runs with the HTTP transport on, the same handler answers `POST /mcp` on 127.0.0.1.
//! Settings (tools, access, hidden folders) are read again for every call, so changes made in
//! the app apply at once.

use std::io::{BufRead, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{Value, json};

use crate::md;
use crate::store::{self, MCP_TOOLS, Mcp};

const PROTOCOL: &str = "2025-06-18";

pub struct Ctx {
    pub root: PathBuf,
    pub mcp: Mcp,
    pub update_links: bool,
}

fn tool_schema(id: &str) -> Value {
    let path = json!({ "type": "string", "description": "Note path relative to the folder, such as Drafts/Ideas.md" });
    match id {
        "list_notes" => json!({ "type": "object", "properties": { "folder": { "type": "string", "description": "Only list notes in this folder" } } }),
        "read_note" | "get_backlinks" | "delete_note" => json!({ "type": "object", "properties": { "path": path }, "required": ["path"] }),
        "search_notes" => json!({ "type": "object", "properties": { "query": { "type": "string", "description": "Text to find, or #tag" } }, "required": ["query"] }),
        "create_note" => json!({ "type": "object", "properties": { "path": path, "content": { "type": "string" } }, "required": ["path"] }),
        "update_note" => json!({ "type": "object", "properties": { "path": path, "content": { "type": "string" }, "mode": { "type": "string", "enum": ["replace", "append"] } }, "required": ["path", "content"] }),
        "rename_note" => json!({ "type": "object", "properties": { "path": path, "new_name": { "type": "string", "description": "The new file name, without a folder" } }, "required": ["path", "new_name"] }),
        _ => json!({ "type": "object" }),
    }
}

fn visible(mcp: &Mcp, rel: &str) -> bool {
    let d = md::dir(rel);
    !mcp.hidden.iter().any(|h| if h.is_empty() { d.is_empty() } else { d == h || d.starts_with(&format!("{h}/")) })
}

/// A safe note path inside the folder: relative, no `..`, ending in `.md`.
fn clean_path(p: &str) -> Result<String, String> {
    let p = p.trim().trim_start_matches("./").replace('\\', "/");
    if p.is_empty() || p.starts_with('/') || p.split('/').any(|c| c == ".." || c.is_empty() || c.starts_with('.')) {
        return Err(format!("Not a note path inside the folder: {p}"));
    }
    Ok(if p.to_lowercase().ends_with(".md") { p } else { format!("{p}.md") })
}

fn text(t: impl Into<String>) -> Value {
    json!({ "content": [{ "type": "text", "text": t.into() }] })
}

fn fail(t: impl Into<String>) -> Value {
    json!({ "content": [{ "type": "text", "text": t.into() }], "isError": true })
}

fn arg<'a>(a: &'a Value, k: &str) -> &'a str {
    a.get(k).and_then(Value::as_str).unwrap_or("")
}

fn call(ctx: &Ctx, name: &str, a: &Value) -> Value {
    if !MCP_TOOLS.iter().any(|t| t.0 == name) || !ctx.mcp.allows(name) {
        return fail(format!("The tool {name} is turned off in Typo's settings."));
    }
    let write = MCP_TOOLS.iter().any(|t| t.0 == name && t.2);
    if write && ctx.mcp.confirm {
        return fail("Typo is set to ask before changes, and this version cannot show that prompt yet. Turn off \"Ask before changes\" in Typo's MCP settings to allow edits.");
    }
    let vault = match crate::vault::load(&ctx.root) {
        Ok(v) => v,
        Err(e) => return fail(format!("Cannot read the folder: {e}")),
    };
    let files: std::collections::BTreeMap<String, String> = vault.files.into_iter().filter(|(p, _)| visible(&ctx.mcp, p)).collect();
    let get = |p: &str| -> Result<String, Value> {
        let p = clean_path(p).map_err(fail)?;
        if files.contains_key(&p) { Ok(p) } else { Err(fail(format!("No note at {p}"))) }
    };
    match name {
        "list_notes" => {
            let f = arg(a, "folder").trim_matches('/');
            let list: Vec<&String> = files.keys().filter(|p| f.is_empty() || md::dir(p) == f || md::dir(p).starts_with(&format!("{f}/"))).collect();
            text(serde_json::to_string_pretty(&json!({ "notes": list, "folders": vault.folders.iter().filter(|d| visible(&ctx.mcp, &format!("{d}/x.md"))).collect::<Vec<_>>() })).unwrap_or_default())
        }
        "read_note" => match get(arg(a, "path")) {
            Ok(p) => text(files[&p].clone()),
            Err(e) => e,
        },
        "search_notes" => {
            let q = arg(a, "query").trim();
            if q.is_empty() {
                return fail("The query is empty.");
            }
            let mut out = String::new();
            for r in md::search(&files, q).iter().take(50) {
                out.push_str(&format!("{}\n", r.path));
                let lines: Vec<&str> = files[&r.path].split('\n').collect();
                for h in r.hits.iter().take(5) {
                    out.push_str(&format!("  {}: {}\n", h.line + 1, lines.get(h.line).unwrap_or(&"").trim()));
                }
            }
            text(if out.is_empty() { "No matches.".into() } else { out })
        }
        "get_backlinks" => match get(arg(a, "path")) {
            Ok(p) => {
                let b: Vec<String> = md::backlinks(&files, &p).into_iter().map(|(p, n)| format!("{p} ({n})")).collect();
                text(if b.is_empty() { "No notes link here.".into() } else { b.join("\n") })
            }
            Err(e) => e,
        },
        "create_note" => {
            let p = match clean_path(arg(a, "path")) {
                Ok(p) => p,
                Err(e) => return fail(e),
            };
            if ctx.root.join(&p).exists() {
                return fail(format!("A note already exists at {p}"));
            }
            write_file(&ctx.root, &p, arg(a, "content")).map_or_else(|e| fail(e.to_string()), |_| text(format!("Created {p}")))
        }
        "update_note" => match get(arg(a, "path")) {
            Ok(p) => {
                let content = arg(a, "content");
                let new = if arg(a, "mode") == "append" {
                    let old = &files[&p];
                    if old.is_empty() || old.ends_with('\n') { format!("{old}{content}") } else { format!("{old}\n{content}") }
                } else {
                    content.to_string()
                };
                write_file(&ctx.root, &p, &new).map_or_else(|e| fail(e.to_string()), |_| text(format!("Updated {p}")))
            }
            Err(e) => e,
        },
        "rename_note" => match get(arg(a, "path")) {
            Ok(p) => {
                let name = md::clean_name(md::strip_md(arg(a, "new_name")));
                if name.is_empty() {
                    return fail("The new name is empty.");
                }
                let np = md::join(md::dir(&p), &format!("{name}.md"));
                if ctx.root.join(&np).exists() {
                    return fail(format!("A note already exists at {np}"));
                }
                if let Err(e) = std::fs::rename(ctx.root.join(&p), ctx.root.join(&np)) {
                    return fail(e.to_string());
                }
                let mut changed = 0;
                if ctx.update_links {
                    for (q, t) in &files {
                        let nt = md::rewrite_links(t, md::base(&p), &name);
                        if nt != *t {
                            let q = if *q == p { np.clone() } else { q.clone() };
                            if write_file(&ctx.root, &q, &nt).is_ok() {
                                changed += 1;
                            }
                        }
                    }
                }
                text(format!("Renamed {p} to {np}. Updated links in {changed} notes."))
            }
            Err(e) => e,
        },
        "delete_note" => match get(arg(a, "path")) {
            Ok(p) => {
                let trash = ctx.root.join(".trash");
                let dst = trash.join(p.replace('/', " - "));
                std::fs::create_dir_all(&trash).and_then(|_| std::fs::rename(ctx.root.join(&p), dst)).map_or_else(|e| fail(e.to_string()), |_| text(format!("Moved {p} to the folder's .trash")))
            }
            Err(e) => e,
        },
        _ => fail("Unknown tool"),
    }
}

fn write_file(root: &Path, rel: &str, t: &str) -> std::io::Result<()> {
    let p = root.join(rel);
    if let Some(d) = p.parent() {
        std::fs::create_dir_all(d)?;
    }
    std::fs::write(p, t)
}

/// Answers one JSON-RPC message. Notifications get no answer.
pub fn handle(ctx: &Ctx, msg: &Value) -> Option<Value> {
    let id = msg.get("id").cloned();
    let method = msg.get("method").and_then(Value::as_str).unwrap_or("");
    let id = id?;
    let result = match method {
        "initialize" => {
            let v = msg.pointer("/params/protocolVersion").and_then(Value::as_str).unwrap_or(PROTOCOL);
            json!({ "protocolVersion": v, "capabilities": { "tools": { "listChanged": false } }, "serverInfo": { "name": "typo", "version": env!("CARGO_PKG_VERSION") },
                "instructions": "Notes are Markdown files. Link notes with [[Note name]] and tag them with #tag." })
        }
        "ping" => json!({}),
        "tools/list" => {
            let tools: Vec<Value> = MCP_TOOLS
                .iter()
                .filter(|t| ctx.mcp.allows(t.0))
                .map(|(id, desc, write)| json!({ "name": id, "description": desc, "inputSchema": tool_schema(id), "annotations": { "readOnlyHint": !write } }))
                .collect();
            json!({ "tools": tools })
        }
        "tools/call" => {
            let name = msg.pointer("/params/name").and_then(Value::as_str).unwrap_or("");
            let empty = json!({});
            let a = msg.pointer("/params/arguments").unwrap_or(&empty);
            call(ctx, name, a)
        }
        _ => return Some(json!({ "jsonrpc": "2.0", "id": id, "error": { "code": -32601, "message": format!("Unknown method {method}") } })),
    };
    Some(json!({ "jsonrpc": "2.0", "id": id, "result": result }))
}

fn ctx_for(root: &Path) -> Ctx {
    let p = store::load();
    Ctx { root: root.to_path_buf(), mcp: p.settings.mcp, update_links: p.settings.update_links }
}

/// `typo mcp --folder <path>`: serves the folder over stdio until stdin closes.
pub fn serve_stdio(root: PathBuf) -> std::io::Result<()> {
    let stdin = std::io::stdin();
    let mut out = std::io::stdout().lock();
    for line in stdin.lock().lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let ctx = ctx_for(&root);
        let reply = match serde_json::from_str::<Value>(&line) {
            Ok(Value::Array(batch)) => {
                let r: Vec<Value> = batch.iter().filter_map(|m| handle(&ctx, m)).collect();
                (!r.is_empty()).then(|| Value::Array(r))
            }
            Ok(msg) => handle(&ctx, &msg),
            Err(e) => Some(json!({ "jsonrpc": "2.0", "id": null, "error": { "code": -32700, "message": e.to_string() } })),
        };
        if let Some(r) = reply {
            writeln!(out, "{r}")?;
            out.flush()?;
        }
    }
    Ok(())
}

/// What the HTTP server needs, shared with the app so changes apply at once.
#[derive(Clone, Debug, PartialEq)]
pub struct HttpCfg {
    pub root: PathBuf,
    pub mcp: Mcp,
    pub update_links: bool,
}

pub struct HttpServer {
    pub port: u16,
    pub cfg: Arc<Mutex<HttpCfg>>,
    stop: Arc<AtomicBool>,
}

impl Drop for HttpServer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
    }
}

/// Serves `POST /mcp` on 127.0.0.1:`port` with the Streamable HTTP transport's JSON replies.
pub fn serve_http(port: u16, cfg: HttpCfg) -> std::io::Result<HttpServer> {
    let listener = TcpListener::bind(("127.0.0.1", port))?;
    let port = listener.local_addr()?.port();
    listener.set_nonblocking(true)?;
    let cfg = Arc::new(Mutex::new(cfg));
    let stop = Arc::new(AtomicBool::new(false));
    let (c2, s2) = (cfg.clone(), stop.clone());
    std::thread::Builder::new().name("typo-mcp-http".into()).spawn(move || {
        while !s2.load(Ordering::SeqCst) {
            match listener.accept() {
                Ok((stream, _)) => {
                    let c = c2.lock().map(|g| g.clone()).ok();
                    if let Some(c) = c {
                        std::thread::spawn(move || {
                            let _ = http_conn(stream, &c);
                        });
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => std::thread::sleep(Duration::from_millis(150)),
                Err(_) => std::thread::sleep(Duration::from_millis(150)),
            }
        }
    })?;
    Ok(HttpServer { port, cfg, stop })
}

fn http_conn(mut stream: TcpStream, cfg: &HttpCfg) -> std::io::Result<()> {
    stream.set_nonblocking(false)?;
    stream.set_read_timeout(Some(Duration::from_secs(10)))?;
    let mut buf = Vec::new();
    let mut tmp = [0u8; 8192];
    let header_end = loop {
        let n = stream.read(&mut tmp)?;
        if n == 0 {
            return Ok(());
        }
        buf.extend_from_slice(&tmp[..n]);
        if let Some(i) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            break i + 4;
        }
        if buf.len() > 64 * 1024 {
            return respond(&mut stream, 431, "", "");
        }
    };
    let head = String::from_utf8_lossy(&buf[..header_end]).to_string();
    let mut lines = head.lines();
    let first = lines.next().unwrap_or("");
    let mut parts = first.split_whitespace();
    let (method, path) = (parts.next().unwrap_or(""), parts.next().unwrap_or(""));
    let mut len = 0usize;
    let mut auth = String::new();
    let mut origin = String::new();
    for l in lines {
        let Some((k, v)) = l.split_once(':') else { continue };
        match k.trim().to_ascii_lowercase().as_str() {
            "content-length" => len = v.trim().parse().unwrap_or(0),
            "authorization" => auth = v.trim().to_string(),
            "origin" => origin = v.trim().to_string(),
            _ => {}
        }
    }
    if path.split('?').next() != Some("/mcp") {
        return respond(&mut stream, 404, "", "");
    }
    // Browsers may only reach the server from a local page.
    if !origin.is_empty() && !(origin.starts_with("http://127.0.0.1") || origin.starts_with("http://localhost")) {
        return respond(&mut stream, 403, "", "");
    }
    let token = auth.strip_prefix("Bearer ").unwrap_or("");
    if cfg.mcp.token.is_empty() || !constant_eq(token.as_bytes(), cfg.mcp.token.as_bytes()) {
        return respond(&mut stream, 401, "application/json", r#"{"error":"missing or wrong token"}"#);
    }
    if method != "POST" {
        return respond(&mut stream, 405, "", "");
    }
    if len > 8 * 1024 * 1024 {
        return respond(&mut stream, 413, "", "");
    }
    let mut body = buf[header_end..].to_vec();
    while body.len() < len {
        let n = stream.read(&mut tmp)?;
        if n == 0 {
            break;
        }
        body.extend_from_slice(&tmp[..n]);
    }
    let ctx = Ctx { root: cfg.root.clone(), mcp: cfg.mcp.clone(), update_links: cfg.update_links };
    let reply = match serde_json::from_slice::<Value>(&body) {
        Ok(Value::Array(batch)) => {
            let r: Vec<Value> = batch.iter().filter_map(|m| handle(&ctx, m)).collect();
            (!r.is_empty()).then(|| Value::Array(r))
        }
        Ok(m) => handle(&ctx, &m),
        Err(e) => Some(json!({ "jsonrpc": "2.0", "id": null, "error": { "code": -32700, "message": e.to_string() } })),
    };
    match reply {
        Some(r) => respond(&mut stream, 200, "application/json", &r.to_string()),
        None => respond(&mut stream, 202, "", ""),
    }
}

fn constant_eq(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

fn respond(s: &mut TcpStream, code: u16, ctype: &str, body: &str) -> std::io::Result<()> {
    let reason = match code {
        200 => "OK",
        202 => "Accepted",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        413 => "Payload Too Large",
        _ => "Error",
    };
    let mut h = format!("HTTP/1.1 {code} {reason}\r\nContent-Length: {}\r\nConnection: close\r\n", body.len());
    if !ctype.is_empty() {
        h.push_str(&format!("Content-Type: {ctype}\r\n"));
    }
    h.push_str("\r\n");
    s.write_all(h.as_bytes())?;
    s.write_all(body.as_bytes())?;
    s.flush()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx(root: &Path) -> Ctx {
        let mut m = Mcp::default();
        m.access = "write".into();
        m.confirm = false;
        m.tools.insert("rename_note".into(), true);
        Ctx { root: root.to_path_buf(), mcp: m, update_links: true }
    }

    #[test]
    fn lists_reads_and_renames_with_link_updates() {
        let dir = std::env::temp_dir().join(format!("typo-mcp-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("Docs")).unwrap();
        std::fs::write(dir.join("A.md"), "# A\n\nSee [[B]].").unwrap();
        std::fs::write(dir.join("Docs/B.md"), "# B").unwrap();
        let c = ctx(&dir);
        let init = handle(&c, &json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18"}})).unwrap();
        assert_eq!(init["result"]["serverInfo"]["name"], "typo");
        let list = handle(&c, &json!({"jsonrpc":"2.0","id":2,"method":"tools/list"})).unwrap();
        assert!(list["result"]["tools"].as_array().unwrap().iter().any(|t| t["name"] == "read_note"));
        let r = handle(&c, &json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"read_note","arguments":{"path":"Docs/B"}}})).unwrap();
        assert_eq!(r["result"]["content"][0]["text"], "# B");
        let r = handle(&c, &json!({"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"rename_note","arguments":{"path":"Docs/B.md","new_name":"C"}}})).unwrap();
        assert!(r["result"].get("isError").is_none(), "{r}");
        assert_eq!(std::fs::read_to_string(dir.join("A.md")).unwrap(), "# A\n\nSee [[C]].");
        let bad = handle(&c, &json!({"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"read_note","arguments":{"path":"../etc/passwd"}}})).unwrap();
        assert_eq!(bad["result"]["isError"], true);
        assert!(handle(&c, &json!({"jsonrpc":"2.0","method":"notifications/initialized"})).is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn http_needs_the_token_and_answers_json_rpc() {
        let dir = std::env::temp_dir().join(format!("typo-mcp-http-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("A.md"), "# A").unwrap();
        let mcp = Mcp::default();
        let token = mcp.token.clone();
        let srv = serve_http(0, HttpCfg { root: dir.clone(), mcp, update_links: true }).unwrap();
        let post = |auth: &str| {
            let body = r#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#;
            let mut s = TcpStream::connect(("127.0.0.1", srv.port)).unwrap();
            write!(s, "POST /mcp HTTP/1.1\r\nHost: x\r\nAuthorization: {auth}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}", body.len()).unwrap();
            let mut out = String::new();
            s.read_to_string(&mut out).unwrap();
            out
        };
        assert!(post("Bearer nope").starts_with("HTTP/1.1 401"));
        let ok = post(&format!("Bearer {token}"));
        assert!(ok.starts_with("HTTP/1.1 200"), "{ok}");
        assert!(ok.contains("read_note"));
        drop(srv);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
