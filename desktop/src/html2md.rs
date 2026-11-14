//! Pasted HTML to Markdown. A copy from a web page, a document or a mail app carries HTML next
//! to its plain text. Typo turns that HTML into Markdown, so headings, lists, links and
//! emphasis stay. HTML with no formatting, such as a copy from a code editor that only colours
//! its text, stays plain text.

/// A node of the parsed HTML.
#[derive(Debug)]
enum Node {
    Text(String),
    El { tag: String, attrs: Vec<(String, String)>, kids: Vec<Node> },
}

const VOID: [&str; 14] = ["br", "hr", "img", "input", "meta", "link", "wbr", "col", "source", "area", "base", "embed", "param", "track"];
const BLOCK: [&str; 33] = [
    "p", "div", "section", "article", "header", "footer", "main", "aside", "nav", "figure", "figcaption", "address", "center", "h1", "h2", "h3", "h4",
    "h5", "h6", "ul", "ol", "li", "blockquote", "pre", "hr", "table", "thead", "tbody", "tfoot", "tr", "dl", "dt", "dd",
];
/// Elements whose content never shows as text.
const SKIP: [&str; 17] = [
    "head", "title", "script", "style", "template", "noscript", "button", "select", "option", "textarea", "svg", "canvas", "iframe", "object", "video",
    "audio", "math",
];

fn attr<'a>(attrs: &'a [(String, String)], name: &str) -> Option<&'a str> {
    attrs.iter().find(|(k, _)| k == name).map(|(_, v)| v.as_str())
}

/// The value of one CSS property in a `style` attribute, in lower case.
fn style_of(attrs: &[(String, String)], prop: &str) -> Option<String> {
    attr(attrs, "style")?.split(';').find_map(|d| {
        let (k, v) = d.split_once(':')?;
        (k.trim().eq_ignore_ascii_case(prop)).then(|| v.trim().to_ascii_lowercase())
    })
}

fn bold_weight(v: &str) -> Option<bool> {
    match v.split_whitespace().next()? {
        "bold" | "bolder" => Some(true),
        "normal" | "lighter" => Some(false),
        n => n.parse::<u32>().ok().map(|w| w >= 600),
    }
}

fn decode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]);
        rest = &rest[i..];
        let end = rest[1..].find(';').map(|j| j + 1).filter(|&j| j <= 12);
        let ch = end.and_then(|j| {
            let name = &rest[1..j];
            let c = if let Some(num) = name.strip_prefix('#') {
                let n = match num.strip_prefix(['x', 'X']) {
                    Some(h) => u32::from_str_radix(h, 16).ok(),
                    None => num.parse().ok(),
                };
                n.and_then(char::from_u32)
            } else {
                Some(match name {
                    "amp" => '&',
                    "lt" => '<',
                    "gt" => '>',
                    "quot" => '"',
                    "apos" => '\'',
                    "nbsp" => ' ',
                    "mdash" => '—',
                    "ndash" => '–',
                    "hellip" => '…',
                    "lsquo" => '‘',
                    "rsquo" => '’',
                    "ldquo" => '“',
                    "rdquo" => '”',
                    "laquo" => '«',
                    "raquo" => '»',
                    "bull" => '•',
                    "middot" => '·',
                    "times" => '×',
                    "copy" => '©',
                    "reg" => '®',
                    "trade" => '™',
                    "euro" => '€',
                    "deg" => '°',
                    _ => return None,
                })
            }?;
            Some((c, j + 1))
        });
        match ch {
            Some((c, len)) => {
                out.push(if c == '\u{a0}' { ' ' } else { c });
                rest = &rest[len..];
            }
            None => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

/// Reads a tag after its `<`: the name, the attributes and the length up to and with `>`.
fn read_tag(s: &str) -> Option<(String, Vec<(String, String)>, usize, bool)> {
    let b = s.as_bytes();
    let mut i = 0;
    while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'-' || b[i] == b':') {
        i += 1;
    }
    if i == 0 {
        return None;
    }
    let name = s[..i].to_ascii_lowercase();
    let mut attrs = Vec::new();
    let mut self_closed = false;
    loop {
        while i < b.len() && b[i].is_ascii_whitespace() {
            i += 1;
        }
        if i >= b.len() {
            return None;
        }
        match b[i] {
            b'>' => return Some((name, attrs, i + 1, self_closed)),
            b'/' => {
                self_closed = true;
                i += 1;
                continue;
            }
            _ => {}
        }
        let k0 = i;
        while i < b.len() && !b[i].is_ascii_whitespace() && !matches!(b[i], b'=' | b'>' | b'/') {
            i += 1;
        }
        let key = s[k0..i].to_ascii_lowercase();
        while i < b.len() && b[i].is_ascii_whitespace() {
            i += 1;
        }
        let mut val = String::new();
        if i < b.len() && b[i] == b'=' {
            i += 1;
            while i < b.len() && b[i].is_ascii_whitespace() {
                i += 1;
            }
            if i < b.len() && (b[i] == b'"' || b[i] == b'\'') {
                let q = b[i];
                let v0 = i + 1;
                let close = s[v0..].bytes().position(|c| c == q)? + v0;
                val = decode(&s[v0..close]);
                i = close + 1;
            } else {
                let v0 = i;
                while i < b.len() && !b[i].is_ascii_whitespace() && b[i] != b'>' {
                    i += 1;
                }
                val = decode(&s[v0..i]);
            }
        }
        if !key.is_empty() {
            attrs.push((key, val));
        }
        self_closed = false;
    }
}

fn parse(html: &str) -> Vec<Node> {
    // The open elements, each with its children so far. The first entry is the root.
    let mut stack: Vec<(String, Vec<(String, String)>, Vec<Node>)> = vec![(String::new(), Vec::new(), Vec::new())];
    fn close(stack: &mut Vec<(String, Vec<(String, String)>, Vec<Node>)>) {
        if stack.len() > 1 {
            let (tag, attrs, kids) = stack.pop().unwrap_or_default();
            if let Some(top) = stack.last_mut() {
                top.2.push(Node::El { tag, attrs, kids });
            }
        }
    }
    let open = |stack: &Vec<(String, Vec<(String, String)>, Vec<Node>)>, tag: &str| stack.iter().rposition(|(t, _, _)| t == tag);
    let mut i = 0;
    while i < html.len() {
        let Some(lt) = html[i..].find('<').map(|n| n + i) else {
            if let Some(top) = stack.last_mut() {
                top.2.push(Node::Text(decode(&html[i..])));
            }
            break;
        };
        if lt > i {
            if let Some(top) = stack.last_mut() {
                top.2.push(Node::Text(decode(&html[i..lt])));
            }
        }
        let rest = &html[lt + 1..];
        if rest.starts_with("!--") {
            i = rest.find("-->").map_or(html.len(), |e| lt + 1 + e + 3);
        } else if rest.starts_with('!') || rest.starts_with('?') {
            i = rest.find('>').map_or(html.len(), |e| lt + 1 + e + 1);
        } else if let Some(r) = rest.strip_prefix('/') {
            match read_tag(r) {
                Some((name, _, len, _)) => {
                    if let Some(at) = open(&stack, &name).filter(|&a| a > 0) {
                        while stack.len() > at {
                            close(&mut stack);
                        }
                    }
                    i = lt + 2 + len;
                }
                None => {
                    if let Some(top) = stack.last_mut() {
                        top.2.push(Node::Text("<".into()));
                    }
                    i = lt + 1;
                }
            }
        } else {
            match read_tag(rest) {
                Some((name, attrs, len, self_closed)) => {
                    i = lt + 1 + len;
                    // Elements that end the open one of their kind, as a browser reads them.
                    let ends: &[&str] = match name.as_str() {
                        "p" => &["p"],
                        "li" => &["li"],
                        "dt" | "dd" => &["dt", "dd"],
                        "tr" => &["tr"],
                        "td" | "th" => &["td", "th"],
                        _ => &[],
                    };
                    let fence: &[&str] = match name.as_str() {
                        "li" => &["ul", "ol"],
                        "td" | "th" | "tr" => &["table"],
                        _ => &[],
                    };
                    if let Some(at) = stack.iter().rposition(|(t, _, _)| ends.contains(&t.as_str()) || fence.contains(&t.as_str())) {
                        if at > 0 && ends.contains(&stack[at].0.as_str()) {
                            while stack.len() > at {
                                close(&mut stack);
                            }
                        }
                    }
                    if BLOCK.contains(&name.as_str()) && name != "li" && stack.last().is_some_and(|(t, _, _)| t == "p") {
                        close(&mut stack);
                    }
                    if VOID.contains(&name.as_str()) || self_closed {
                        if let Some(top) = stack.last_mut() {
                            top.2.push(Node::El { tag: name, attrs, kids: Vec::new() });
                        }
                    } else {
                        stack.push((name, attrs, Vec::new()));
                    }
                }
                None => {
                    if let Some(top) = stack.last_mut() {
                        top.2.push(Node::Text("<".into()));
                    }
                    i = lt + 1;
                }
            }
        }
    }
    while stack.len() > 1 {
        close(&mut stack);
    }
    stack.pop().map(|r| r.2).unwrap_or_default()
}

#[derive(Clone, Copy, Default)]
struct Cx {
    bold: bool,
    italic: bool,
    strike: bool,
}

fn is_block(tag: &str) -> bool {
    BLOCK.contains(&tag)
}

fn has_block(kids: &[Node]) -> bool {
    kids.iter().any(|k| match k {
        Node::El { tag, kids, .. } => is_block(tag) || has_block(kids),
        Node::Text(_) => false,
    })
}

/// The style an element adds: bold, italic or struck through.
fn styled(tag: &str, attrs: &[(String, String)], cx: Cx) -> Cx {
    let mut c = cx;
    match tag {
        "strong" | "b" => c.bold = true,
        "em" | "i" | "cite" | "dfn" => c.italic = true,
        "s" | "del" | "strike" => c.strike = true,
        _ => {}
    }
    // Google Docs wraps a whole copy in <b style="font-weight:normal">.
    if let Some(w) = style_of(attrs, "font-weight").and_then(|v| bold_weight(&v)) {
        c.bold = w;
    }
    if let Some(v) = style_of(attrs, "font-style") {
        c.italic = v.starts_with("italic") || v.starts_with("oblique");
    }
    if style_of(attrs, "text-decoration").or_else(|| style_of(attrs, "text-decoration-line")).is_some_and(|v| v.contains("line-through")) {
        c.strike = true;
    }
    c
}

/// Wraps `inner` in a mark, with the spaces at its ends outside the mark.
fn wrap(inner: &str, mark: &str) -> String {
    let t = inner.trim();
    if t.is_empty() {
        return inner.to_string();
    }
    let lead = &inner[..inner.len() - inner.trim_start().len()];
    let trail = &inner[inner.trim_end().len()..];
    format!("{lead}{mark}{t}{mark}{trail}")
}

fn url(href: &str) -> String {
    href.trim().replace(' ', "%20").replace('(', "%28").replace(')', "%29")
}

/// The text of a node and its children as they are, for code.
fn raw(n: &Node, out: &mut String) {
    match n {
        Node::Text(t) => out.push_str(t),
        Node::El { tag, kids, .. } => {
            if tag == "br" {
                out.push('\n');
            } else if !SKIP.contains(&tag.as_str()) {
                for k in kids {
                    raw(k, out);
                }
                if matches!(tag.as_str(), "div" | "p" | "li" | "tr") && !out.ends_with('\n') {
                    out.push('\n');
                }
            }
        }
    }
}

fn inline(nodes: &[Node], cx: Cx) -> String {
    let mut s = String::new();
    for n in nodes {
        s.push_str(&inline_one(n, cx));
    }
    s
}

fn inline_one(n: &Node, cx: Cx) -> String {
    let (tag, attrs, kids) = match n {
        Node::Text(t) => return t.chars().map(|c| if c.is_whitespace() { ' ' } else { c }).collect(),
        Node::El { tag, attrs, kids } => (tag.as_str(), attrs.as_slice(), kids.as_slice()),
    };
    if SKIP.contains(&tag) {
        return String::new();
    }
    match tag {
        "br" => "\n".into(),
        "img" => {
            let src = attr(attrs, "src").unwrap_or("");
            let alt = attr(attrs, "alt").unwrap_or("").replace(['[', ']', '\n'], " ");
            // An image from a page's own folder or inline data has no address that works here.
            if src.starts_with("http://") || src.starts_with("https://") { format!("![{}]({})", alt.trim(), url(src)) } else { String::new() }
        }
        "input" => String::new(),
        "code" | "kbd" | "samp" | "tt" => {
            let mut t = String::new();
            for k in kids {
                raw(k, &mut t);
            }
            let t = t.replace('\n', " ");
            if t.trim().is_empty() {
                return t;
            }
            let fence = if t.contains('`') { "``" } else { "`" };
            let pad = if t.starts_with('`') || t.ends_with('`') { " " } else { "" };
            format!("{fence}{pad}{t}{pad}{fence}")
        }
        "a" => {
            let inner = inline(kids, cx);
            let href = attr(attrs, "href").unwrap_or("").trim();
            let lower = href.to_ascii_lowercase();
            if inner.trim().is_empty() || href.is_empty() || href.starts_with('#') || lower.starts_with("javascript:") || lower.starts_with("data:") {
                return inner;
            }
            let lead = &inner[..inner.len() - inner.trim_start().len()];
            let trail = &inner[inner.trim_end().len()..];
            format!("{lead}[{}]({}){trail}", inner.trim().replace('\n', " "), url(href))
        }
        _ => {
            let c = styled(tag, attrs, cx);
            let mut inner = inline(kids, c);
            // Each mark opens where the style starts, so a nested bold adds no second mark.
            if c.strike && !cx.strike {
                inner = wrap(&inner, "~~");
            }
            if c.italic && !cx.italic {
                inner = wrap(&inner, "*");
            }
            if c.bold && !cx.bold {
                inner = wrap(&inner, "**");
            }
            if is_block(tag) { format!("\n{inner}\n") } else { inner }
        }
    }
}

/// Runs of inline content as lines: spaces collapse, and each line is trimmed.
fn tidy(s: &str) -> String {
    let lines: Vec<String> = s
        .split('\n')
        .map(|l| {
            let mut out = String::new();
            for w in l.split(' ').filter(|w| !w.is_empty()) {
                if !out.is_empty() {
                    out.push(' ');
                }
                out.push_str(w);
            }
            out
        })
        .collect();
    let a = lines.iter().position(|l| !l.is_empty()).unwrap_or(lines.len());
    let b = lines.iter().rposition(|l| !l.is_empty()).map_or(a, |b| b + 1);
    lines[a..b].join("\n")
}

/// Block content: each block is one entry. Inline runs between blocks become paragraphs.
fn blocks(nodes: &[Node], cx: Cx, out: &mut Vec<String>) {
    let mut run = String::new();
    let flush = |run: &mut String, out: &mut Vec<String>| {
        let t = tidy(run);
        if !t.is_empty() {
            out.push(t);
        }
        run.clear();
    };
    for n in nodes {
        let Node::El { tag, attrs, kids } = n else {
            run.push_str(&inline_one(n, cx));
            continue;
        };
        let tag = tag.as_str();
        if SKIP.contains(&tag) {
            continue;
        }
        if !is_block(tag) {
            if has_block(kids) {
                // An inline element around blocks, such as Google Docs' <b> around the copy.
                flush(&mut run, out);
                blocks(kids, styled(tag, attrs, cx), out);
            } else {
                run.push_str(&inline_one(n, cx));
            }
            continue;
        }
        flush(&mut run, out);
        match tag {
            "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
                let level = (tag.as_bytes()[1] - b'0') as usize;
                let t = tidy(&inline(kids, Cx { bold: true, ..cx })).replace('\n', " ");
                if !t.is_empty() {
                    out.push(format!("{} {t}", "#".repeat(level)));
                }
            }
            "hr" => out.push("---".into()),
            "pre" => {
                let mut t = String::new();
                for k in kids {
                    raw(k, &mut t);
                }
                let lang = std::iter::once(attrs.as_slice())
                    .chain(kids.iter().filter_map(|k| if let Node::El { tag, attrs, .. } = k { (tag == "code").then_some(attrs.as_slice()) } else { None }))
                    .filter_map(|a| attr(a, "class"))
                    .flat_map(|c| c.split_whitespace())
                    .find_map(|c| c.strip_prefix("language-").or_else(|| c.strip_prefix("lang-")))
                    .unwrap_or("")
                    .to_string();
                let t = t.trim_matches('\n');
                let fence = if t.contains("```") { "~~~~" } else { "```" };
                out.push(format!("{fence}{lang}\n{t}\n{fence}"));
            }
            "ul" | "ol" => {
                let start = attr(attrs, "start").and_then(|s| s.trim().parse::<i64>().ok()).unwrap_or(1);
                let mut items = Vec::new();
                let mut n = start;
                for k in kids {
                    let Node::El { tag: t, kids: lk, .. } = k else {
                        continue;
                    };
                    if t != "li" {
                        // A list right inside a list belongs to the item before it.
                        if matches!(t.as_str(), "ul" | "ol") {
                            let mut sub = Vec::new();
                            blocks(std::slice::from_ref(k), cx, &mut sub);
                            if let Some(last) = items.last_mut() {
                                let s: &mut String = last;
                                for l in sub.join("\n").lines() {
                                    s.push_str("\n  ");
                                    s.push_str(l);
                                }
                            }
                        }
                        continue;
                    }
                    let marker = if tag == "ol" { format!("{n}. ") } else { "- ".to_string() };
                    n += 1;
                    let task = first_checkbox(lk).map(|on| if on { "[x] " } else { "[ ] " }).unwrap_or("");
                    let mut parts = Vec::new();
                    blocks(lk, cx, &mut parts);
                    let body = parts.join("\n");
                    let pad = " ".repeat(marker.len());
                    let mut item = format!("{marker}{task}");
                    for (i, l) in body.lines().enumerate() {
                        if i > 0 {
                            item.push('\n');
                            if !l.is_empty() {
                                item.push_str(&pad);
                            }
                        }
                        item.push_str(l);
                    }
                    items.push(item.trim_end().to_string());
                }
                if !items.is_empty() {
                    out.push(items.join("\n"));
                }
            }
            "li" => {
                let mut parts = Vec::new();
                blocks(kids, cx, &mut parts);
                let body = parts.join("\n").replace('\n', "\n  ");
                out.push(format!("- {body}"));
            }
            "blockquote" => {
                let mut parts = Vec::new();
                blocks(kids, cx, &mut parts);
                let q: Vec<String> = parts.join("\n\n").lines().map(|l| if l.is_empty() { ">".to_string() } else { format!("> {l}") }).collect();
                if !q.is_empty() {
                    out.push(q.join("\n"));
                }
            }
            "table" | "thead" | "tbody" | "tfoot" | "tr" => {
                let rows = table_rows(n);
                if let Some(t) = table(&rows) {
                    out.push(t);
                }
            }
            "dt" => {
                let t = tidy(&inline(kids, cx));
                if !t.is_empty() {
                    out.push(wrap(&t, "**"));
                }
            }
            _ => blocks(kids, styled(tag, attrs, cx), out),
        }
    }
    flush(&mut run, out);
}

/// Whether a list item starts with a checkbox, and whether it is ticked.
fn first_checkbox(kids: &[Node]) -> Option<bool> {
    for k in kids {
        match k {
            Node::Text(t) if t.trim().is_empty() => continue,
            Node::El { tag, attrs, .. } if tag == "input" => {
                return attr(attrs, "type").is_some_and(|t| t.eq_ignore_ascii_case("checkbox")).then(|| attr(attrs, "checked").is_some());
            }
            Node::El { tag, kids, .. } if matches!(tag.as_str(), "p" | "span" | "label" | "div") => return first_checkbox(kids),
            _ => return None,
        }
    }
    None
}

fn table_rows(n: &Node) -> Vec<Vec<String>> {
    let mut rows = Vec::new();
    fn walk(n: &Node, rows: &mut Vec<Vec<String>>) {
        let Node::El { tag, kids, .. } = n else { return };
        if tag == "tr" {
            let cells = kids
                .iter()
                .filter_map(|c| match c {
                    Node::El { tag, kids, .. } if tag == "td" || tag == "th" => Some(tidy(&inline(kids, Cx::default())).replace('\n', " ").replace('|', "\\|")),
                    _ => None,
                })
                .collect::<Vec<_>>();
            if !cells.is_empty() {
                rows.push(cells);
            }
        } else if tag != "table" || rows.is_empty() {
            for k in kids {
                walk(k, rows);
            }
        }
    }
    walk(n, &mut rows);
    rows
}

fn table(rows: &[Vec<String>]) -> Option<String> {
    let cols = rows.iter().map(Vec::len).max()?;
    let line = |r: &[String]| {
        let mut cells: Vec<&str> = r.iter().map(String::as_str).collect();
        cells.resize(cols, "");
        format!("| {} |", cells.join(" | "))
    };
    let mut out = vec![line(&rows[0]), format!("|{}", " --- |".repeat(cols))];
    out.extend(rows[1..].iter().map(|r| line(r)));
    Some(out.join("\n"))
}

/// Whether the HTML carries formatting that Markdown can keep. A copy from a code editor only
/// colours spans, so it pastes better as its plain text.
pub fn has_formatting(html: &str) -> bool {
    fn walk(nodes: &[Node]) -> bool {
        nodes.iter().any(|n| match n {
            Node::Text(_) => false,
            Node::El { tag, attrs, kids } => {
                let t = tag.as_str();
                let marks = matches!(
                    t,
                    "h1" | "h2" | "h3" | "h4" | "h5" | "h6" | "ul" | "ol" | "blockquote" | "pre" | "code" | "table" | "hr" | "em" | "i" | "s" | "del" | "strike"
                ) || (t == "a" && attr(attrs, "href").is_some_and(|h| !h.trim().is_empty() && !h.starts_with('#')))
                    || (t == "img" && attr(attrs, "src").is_some_and(|s| s.starts_with("http")))
                    || (matches!(t, "b" | "strong") && style_of(attrs, "font-weight").and_then(|v| bold_weight(&v)) != Some(false));
                let styled = style_of(attrs, "font-weight").and_then(|v| bold_weight(&v)) == Some(true)
                    || style_of(attrs, "font-style").is_some_and(|v| v.starts_with("italic"));
                // Code editors set `white-space: pre` and colour each token.
                let code = style_of(attrs, "white-space").is_some_and(|v| v.starts_with("pre"));
                !code && (marks || (styled && !matches!(t, "b" | "strong")) || walk(kids))
            }
        })
    }
    walk(&parse(html))
}

/// Pasted HTML as Markdown.
pub fn convert(html: &str) -> String {
    let mut out = Vec::new();
    blocks(&parse(html), Cx::default(), &mut out);
    out.join("\n\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn web_page() {
        let html = r#"<html><head><title>x</title></head><body><h1>Title <small>sub</small></h1>
            <p>Some <b>bold</b>, <em>italic</em> and <del>gone</del> text with a
            <a href="https://example.com/a b">link</a> &amp; <code>code</code>.</p>
            <p>Line one<br>Line two</p><hr><blockquote><p>Quoted</p><p>Twice</p></blockquote></body></html>"#;
        assert_eq!(
            convert(html),
            "# Title sub\n\nSome **bold**, *italic* and ~~gone~~ text with a [link](https://example.com/a%20b) & `code`.\n\nLine one\nLine two\n\n---\n\n> Quoted\n>\n> Twice"
        );
    }

    #[test]
    fn lists() {
        let html = "<ul><li>One<ul><li>Inner</li></ul></li><li><p>Two</p><p>more</p></li></ul><ol start=3><li>Three<li>Four</ol>\
            <ul><li><input type=checkbox checked> Done</li><li><input type=checkbox> Open</li></ul>";
        assert_eq!(convert(html), "- One\n  - Inner\n- Two\n  more\n\n3. Three\n4. Four\n\n- [x] Done\n- [ ] Open");
    }

    #[test]
    fn code_and_tables() {
        let html = "<pre><code class=\"language-rust\">fn main() {\n    let a = 1 &lt; 2;\n}</code></pre>\
            <table><tr><th>A</th><th>B</th></tr><tr><td>1</td><td>x | y</td></tr></table>";
        assert_eq!(convert(html), "```rust\nfn main() {\n    let a = 1 < 2;\n}\n```\n\n| A | B |\n| --- | --- |\n| 1 | x \\| y |");
    }

    #[test]
    fn google_docs() {
        let html = r#"<meta charset="utf-8"><b style="font-weight:normal;" id="docs-internal-guid-1"><p dir="ltr"><span style="font-weight:700;">Heavy</span><span style="font-weight:400;"> and </span><span style="font-style:italic;">slanted</span></p><ul><li dir="ltr"><p dir="ltr"><span>Item</span></p></li></ul></b>"#;
        assert_eq!(convert(html), "**Heavy** and *slanted*\n\n- Item");
        assert!(has_formatting(html));
    }

    #[test]
    fn plain_copies_stay_plain() {
        // VS Code: coloured spans in a `white-space: pre` div.
        let vscode = r#"<div style="color: #d4d4d4;font-family: monospace;white-space: pre;"><div><span style="color: #569cd6;">let</span> a = 1;</div></div>"#;
        assert!(!has_formatting(vscode));
        assert!(!has_formatting("<p>Just a sentence.</p><div>Another one.</div>"));
        assert!(has_formatting("<p>A <a href=\"https://a.b\">link</a></p>"));
        assert!(has_formatting("<h2>Heading</h2>"));
    }

    #[test]
    fn odd_input() {
        assert_eq!(convert("a &lt;b&gt; &#65;&#x42; &unknown; &amp"), "a <b> AB &unknown; &amp");
        assert_eq!(convert("<p>unclosed <b>bold"), "unclosed **bold**");
        assert_eq!(convert("<b> spaced </b>word"), "**spaced** word");
        assert_eq!(convert("<a href=\"javascript:alert(1)\">x</a> <img src=\"data:image/png;base64,AA\" alt=\"pic\">"), "x");
        assert_eq!(convert("x < y and <3"), "x < y and <3");
        assert_eq!(convert(""), "");
    }

    /// Broken and random input never panics: tags cut off, stray `&`, quotes and multibyte text.
    #[test]
    fn random_input() {
        let parts = ["<", ">", "</", "<b>", "</b>", "<p>", "<li>", "<ul>", "</ol>", "<a href=\"", "\"", "'", "&", "&amp;", "&#x", ";", "<!--", "-->", "æ", "—", "🙂", " ", "\n", "<pre>", "<code>", "<table><tr><td>", "<img src=", "=", "/>", "x"];
        let mut seed: u64 = 7;
        for _ in 0..4000 {
            let mut s = String::new();
            for _ in 0..(seed % 24) {
                seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                s.push_str(parts[(seed >> 33) as usize % parts.len()]);
            }
            let _ = convert(&s);
            let _ = has_formatting(&s);
        }
    }

    /// Converts the site's own page, a real document, without a panic.
    #[test]
    fn real_page() {
        let html = include_str!("../../site/index.html");
        let md = convert(html);
        assert!(md.contains("# "), "{}", &md[..md.len().min(400)]);
        assert!(has_formatting(html));
    }

}
