//! The Markdown that Quire reads: GitHub Flavored Markdown, parsed by comrak, plus the Obsidian
//! syntax that notes use: front matter, `[[note|alias]]`, `![[embed]]` and `#tags`. Line breaks
//! stay as typed, and only `~~` strikes, as in Obsidian.
//!
//! One parse gives the blocks the preview draws and the facts the panels read: the outline's
//! headings, the tags, and the wiki links that backlinks count and a rename rewrites. Code holds
//! no tags or links.

use std::borrow::Cow;
use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap};
use std::hash::{DefaultHasher, Hash, Hasher};
use std::ops::Range;
use std::rc::Rc;

use comrak::nodes::{AstNode, ListDelimType, ListType, NodeValue, Sourcepos, TableAlignment};
use comrak::{Arena, Options, parse_document};

use super::{Img, base, is_image, is_word, link_target, percent_decode, pretty, prev_char, scan, strip_md};

/// Inline content, as the preview draws it.
#[derive(Clone, Debug, PartialEq)]
pub enum Inline {
    Text(String),
    Code(String),
    Strong(Vec<Inline>),
    Emph(Vec<Inline>),
    Strike(Vec<Inline>),
    /// `[text](url)`, `<url>` or a bare URL.
    Link { url: String, kids: Vec<Inline> },
    /// `[[target|shown]]`.
    Wiki { target: String, shown: String },
    Image(Img),
    /// A `#tag`, without its `#`.
    Tag(String),
    /// A line break, soft or hard: each shows as typed.
    Break,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Align {
    Left,
    Center,
    Right,
}

/// A list item. `task` is `Some(done)` for a task.
#[derive(Clone, Debug, PartialEq)]
pub struct Item {
    pub line: usize,
    pub task: Option<bool>,
    pub blocks: Vec<Block>,
}

/// A block of the preview. `line` is the 0-based source line it starts on.
#[derive(Clone, Debug, PartialEq)]
pub enum Block {
    /// Front matter, as properties: each key with its values joined by commas.
    Props { line: usize, props: Vec<(String, Vec<Inline>)> },
    Para { line: usize, inlines: Vec<Inline> },
    /// A line of a paragraph that holds nothing but images.
    Image { line: usize, images: Vec<Img> },
    Heading { line: usize, level: usize, inlines: Vec<Inline> },
    Pre { line: usize, text: String },
    Rule { line: usize },
    Quote { line: usize, blocks: Vec<Block> },
    List { line: usize, ordered: bool, start: usize, delim: char, tight: bool, items: Vec<Item> },
    /// The first row is the header. Every row has a cell for each column.
    Table { line: usize, align: Vec<Align>, rows: Vec<Vec<Vec<Inline>>> },
}

#[derive(Clone, Debug, PartialEq)]
pub struct Heading {
    pub level: usize,
    pub text: String,
    pub line: usize,
}

/// A `[[link]]`: its target and, where it could be placed, its bytes in the note.
#[derive(Clone, Debug)]
struct WikiRef {
    target: String,
    range: Option<Range<usize>>,
}

#[derive(Debug, Default)]
struct Facts {
    headings: Vec<Heading>,
    /// Lowercased, once each.
    tags: Vec<String>,
    links: Vec<WikiRef>,
}

fn options() -> Options<'static> {
    let mut o = Options::default();
    o.extension.table = true;
    o.extension.tasklist = true;
    o.extension.strikethrough = true;
    o.extension.autolink = true;
    o.extension.front_matter_delimiter = Some("---".into());
    o.extension.wikilinks_title_after_pipe = true;
    o
}

fn parse(text: &str) -> (Vec<Block>, Facts) {
    let arena = Arena::new();
    let (src, mark) = lone_tildes(text);
    let root = parse_document(&arena, &src, &options());
    let mut starts = vec![0];
    starts.extend(text.match_indices('\n').map(|(i, _)| i + 1));
    let mut c = Conv { text, mark, starts, facts: Facts::default() };
    let blocks = c.blocks(root);
    (blocks, c.facts)
}

/// GFM strikes text between single tildes too, but Obsidian strikes only `~~`, and notes write a
/// lone `~` for "about". So each lone `~` is parsed as a mark the note doesn't use: punctuation, as
/// `~` is, so emphasis beside it reads the same; inert with these options; one byte, so every
/// offset holds. `Conv::own` turns it back.
fn lone_tildes(text: &str) -> (Cow<'_, str>, Option<char>) {
    let b = text.as_bytes();
    let lone = |i: usize| b[i] == b'~' && (i == 0 || b[i - 1] != b'~') && b.get(i + 1) != Some(&b'~');
    if !(0..b.len()).any(lone) {
        return (Cow::Borrowed(text), None);
    }
    // Not `;` (entities), `=` or `+` (underlines, lists), `!` (images) or `.` (ordered lists).
    let Some(m) = ['^', '$', '%'].into_iter().find(|&c| !text.contains(c)) else {
        return (Cow::Borrowed(text), None);
    };
    let v = (0..b.len()).map(|i| if lone(i) { m as u8 } else { b[i] }).collect();
    (Cow::Owned(String::from_utf8(v).expect("one ASCII byte for another")), Some(m))
}

// ---------- Caches ----------

thread_local! {
    static FACTS: RefCell<HashMap<u64, Rc<Facts>>> = RefCell::new(HashMap::new());
    static TREE: RefCell<Option<(u64, Rc<Vec<Block>>)>> = const { RefCell::new(None) };
}

fn hash(text: &str) -> u64 {
    let mut h = DefaultHasher::new();
    text.hash(&mut h);
    h.finish()
}

fn keep_facts(h: u64, f: Facts) -> Rc<Facts> {
    let f = Rc::new(f);
    FACTS.with_borrow_mut(|c| {
        if c.len() >= 4096 {
            c.clear();
        }
        c.insert(h, f.clone());
    });
    f
}

fn facts(text: &str) -> Rc<Facts> {
    let h = hash(text);
    if let Some(f) = FACTS.with_borrow(|c| c.get(&h).cloned()) {
        return f;
    }
    keep_facts(h, parse(text).1)
}

/// The blocks of a note. The last note's are kept, so a frame that draws the same text parses
/// nothing.
pub fn tree(text: &str) -> Rc<Vec<Block>> {
    let h = hash(text);
    if let Some(t) = TREE.with_borrow(|c| c.as_ref().filter(|(k, _)| *k == h).map(|(_, t)| t.clone())) {
        return t;
    }
    let (blocks, facts) = parse(text);
    let blocks = Rc::new(blocks);
    TREE.set(Some((h, blocks.clone())));
    keep_facts(h, facts);
    blocks
}

// ---------- Facts ----------

pub fn headings(text: &str) -> Vec<Heading> {
    facts(text).headings.clone()
}

/// Tags across all notes with the number of notes using each, most used first.
pub fn tags(files: &BTreeMap<String, String>) -> Vec<(String, usize)> {
    let mut m: BTreeMap<String, usize> = BTreeMap::new();
    for t in files.values() {
        for tag in &facts(t).tags {
            *m.entry(tag.clone()).or_default() += 1;
        }
    }
    let mut v: Vec<(String, usize)> = m.into_iter().collect();
    v.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    v
}

/// Notes that link to `active`, with the number of links in each.
pub fn backlinks(files: &BTreeMap<String, String>, active: &str) -> Vec<(String, usize)> {
    let b = base(active).to_lowercase();
    let full = strip_md(active).to_lowercase();
    let mut out = Vec::new();
    for (p, t) in files {
        if p == active || !t.contains("[[") {
            continue;
        }
        let n = facts(t).links.iter().filter(|l| {
            let t = l.target.to_lowercase();
            t == b || t == full
        }).count();
        if n > 0 {
            out.push((p.clone(), n));
        }
    }
    out
}

/// Rewrites `[[old]]`, `[[old|alias]]` and `![[old]]` to `new`, case-insensitively, in the text
/// and the front matter but not in code.
pub fn rewrite_links(text: &str, old: &str, new: &str) -> String {
    if !text.contains("[[") {
        return text.to_string();
    }
    let old = old.to_lowercase();
    let f = facts(text);
    let mut spans: Vec<Range<usize>> = f.links.iter().filter_map(|l| l.range.clone()).collect();
    spans.sort_by_key(|r| r.start);
    let mut out = String::with_capacity(text.len());
    let mut last = 0;
    for r in spans {
        if r.start < last {
            continue;
        }
        let s = &text[r.clone()];
        let open = if s.starts_with("![[") { 3 } else { 2 };
        let inner = &s[open..s.len() - 2];
        let mut end = inner.find('|').unwrap_or(inner.len());
        // `[[note\|alias]]` in a table cell.
        if inner[..end].ends_with('\\') {
            end -= 1;
        }
        if link_target(&inner[..end]).to_lowercase() != old {
            continue;
        }
        out.push_str(&text[last..r.start + open]);
        out.push_str(new);
        last = r.start + open + end;
    }
    out.push_str(&text[last..]);
    out
}

/// The text of inlines, as a heading's name in the outline.
pub fn plain(v: &[Inline]) -> String {
    let mut s = String::new();
    for i in v {
        match i {
            Inline::Text(t) | Inline::Code(t) => s.push_str(t),
            Inline::Strong(k) | Inline::Emph(k) | Inline::Strike(k) => s.push_str(&plain(k)),
            Inline::Link { url, kids } if kids.is_empty() => s.push_str(url),
            Inline::Link { kids, .. } => s.push_str(&plain(kids)),
            Inline::Wiki { shown, .. } => s.push_str(shown),
            Inline::Image(img) => s.push_str(&img.alt),
            Inline::Tag(t) => {
                s.push('#');
                s.push_str(t);
            }
            Inline::Break => s.push(' '),
        }
    }
    s
}

// ---------- From comrak's tree ----------

struct Conv<'t> {
    text: &'t str,
    /// What a lone `~` was parsed as (`lone_tildes`).
    mark: Option<char>,
    /// The byte where each line starts.
    starts: Vec<usize>,
    facts: Facts,
}

fn line_of<'a>(n: &'a AstNode<'a>) -> usize {
    n.data.borrow().sourcepos.start.line.saturating_sub(1)
}

impl Conv<'_> {
    fn blocks<'a>(&mut self, parent: &'a AstNode<'a>) -> Vec<Block> {
        let mut out = Vec::new();
        for n in parent.children() {
            self.block(n, &mut out);
        }
        out
    }

    fn block<'a>(&mut self, n: &'a AstNode<'a>, out: &mut Vec<Block>) {
        let d = n.data.borrow();
        let line = d.sourcepos.start.line.saturating_sub(1);
        match &d.value {
            NodeValue::FrontMatter(fm) => {
                let fm = self.own(fm).into_owned();
                let b = self.front_matter(&fm);
                out.push(b);
            }
            NodeValue::Paragraph => {
                let inlines = self.inlines(n, false);
                split_paragraph(line, inlines, out);
            }
            NodeValue::Heading(h) => {
                let inlines = self.inlines(n, false);
                let level = h.level as usize;
                let text = plain(&inlines).trim().to_string();
                if !text.is_empty() {
                    self.facts.headings.push(Heading { level, text, line });
                }
                out.push(Block::Heading { line, level, inlines });
            }
            NodeValue::CodeBlock(c) => {
                let t = self.own(&c.literal);
                out.push(Block::Pre { line, text: t.strip_suffix('\n').unwrap_or(&t).to_string() });
            }
            // Raw HTML shows as text.
            NodeValue::HtmlBlock(h) => out.push(Block::Para { line, inlines: vec![Inline::Text(self.own(h.literal.trim_end()).into_owned())] }),
            NodeValue::ThematicBreak => out.push(Block::Rule { line }),
            NodeValue::BlockQuote => {
                let blocks = self.blocks(n);
                out.push(Block::Quote { line, blocks });
            }
            NodeValue::List(l) => {
                let mut items = Vec::new();
                for c in n.children() {
                    let task = match &c.data.borrow().value {
                        NodeValue::TaskItem(t) => Some(t.symbol.is_some()),
                        _ => None,
                    };
                    let blocks = self.blocks(c);
                    items.push(Item { line: line_of(c), task, blocks });
                }
                out.push(Block::List {
                    line,
                    ordered: matches!(l.list_type, ListType::Ordered),
                    start: l.start,
                    delim: if matches!(l.delimiter, ListDelimType::Paren) { ')' } else { '.' },
                    tight: l.tight,
                    items,
                });
            }
            NodeValue::Table(t) => {
                let align: Vec<Align> = t
                    .alignments
                    .iter()
                    .map(|a| match a {
                        TableAlignment::Center => Align::Center,
                        TableAlignment::Right => Align::Right,
                        _ => Align::Left,
                    })
                    .collect();
                let mut rows = Vec::new();
                for r in n.children() {
                    let mut cells = Vec::new();
                    for c in r.children() {
                        cells.push(self.inlines(c, false));
                    }
                    cells.resize(align.len(), Vec::new());
                    rows.push(cells);
                }
                out.push(Block::Table { line, align, rows });
            }
            _ => {
                for c in n.children() {
                    self.block(c, out);
                }
            }
        }
    }

    /// `s` with each lone `~` back.
    fn own<'s>(&self, s: &'s str) -> Cow<'s, str> {
        match self.mark {
            Some(m) if s.contains(m) => Cow::Owned(s.replace(m, "~")),
            _ => Cow::Borrowed(s),
        }
    }

    /// The inlines under `parent`. Text inside a link holds no tags.
    fn inlines<'a>(&mut self, parent: &'a AstNode<'a>, link: bool) -> Vec<Inline> {
        let mut out = Vec::new();
        // comrak splits text at the characters that could start markup, so runs of text join
        // here before tags are found in them.
        let mut buf = String::new();
        for n in parent.children() {
            let d = n.data.borrow();
            if let NodeValue::Text(t) = &d.value {
                buf.push_str(&self.own(t));
                continue;
            }
            self.flush(&mut buf, &mut out, link);
            match &d.value {
                NodeValue::SoftBreak | NodeValue::LineBreak => out.push(Inline::Break),
                NodeValue::Code(c) => out.push(Inline::Code(self.own(&c.literal).into_owned())),
                NodeValue::HtmlInline(h) => push_text(&mut out, &self.own(h)),
                NodeValue::Emph => {
                    let k = self.inlines(n, link);
                    out.push(Inline::Emph(k));
                }
                NodeValue::Strong => {
                    let k = self.inlines(n, link);
                    out.push(Inline::Strong(k));
                }
                NodeValue::Strikethrough => {
                    let k = self.inlines(n, link);
                    out.push(Inline::Strike(k));
                }
                NodeValue::Link(l) => {
                    let kids = self.inlines(n, true);
                    out.push(Inline::Link { url: self.own(&l.url).into_owned(), kids });
                }
                NodeValue::Image(l) => {
                    let kids = self.inlines(n, true);
                    let url = self.own(&l.url);
                    let src = if url.contains("://") { url.into_owned() } else { percent_decode(&url) };
                    out.push(Inline::Image(Img { alt: plain(&kids), src, embed: false }));
                }
                NodeValue::WikiLink(w) => {
                    let kids = self.inlines(n, true);
                    let url = self.own(&w.url).into_owned();
                    let i = self.wiki(&url, w.embed, d.sourcepos, &kids);
                    out.push(i);
                }
                _ => {
                    let k = self.inlines(n, link);
                    out.extend(k);
                }
            }
        }
        self.flush(&mut buf, &mut out, link);
        out
    }

    fn flush(&mut self, buf: &mut String, out: &mut Vec<Inline>, link: bool) {
        if buf.is_empty() {
            return;
        }
        if link {
            push_text(out, buf);
        } else {
            tagify(buf, out, &mut self.facts.tags);
        }
        buf.clear();
    }

    fn wiki(&mut self, url: &str, embed: bool, at: Sourcepos, kids: &[Inline]) -> Inline {
        // In a table cell, `[[note\|alias]]` leaves the `\` on the target.
        let target = link_target(url.trim_end_matches('\\'));
        let range = self.wiki_range(at);
        self.facts.links.push(WikiRef { target: target.clone(), range });
        if embed && is_image(&target) {
            let name = target.rsplit('/').next().unwrap_or(&target);
            let alt = name.rsplit_once('.').map_or(name, |(b, _)| b).to_string();
            return Inline::Image(Img { alt, src: target, embed: true });
        }
        let shown = plain(kids).trim().to_string();
        Inline::Wiki { shown: if shown.is_empty() { target.clone() } else { shown }, target }
    }

    /// The bytes of the `[[…]]` or `![[…]]` that starts at `at`, when the text there is one.
    fn wiki_range(&self, at: Sourcepos) -> Option<Range<usize>> {
        let a = self.starts.get(at.start.line.checked_sub(1)?)? + at.start.column.checked_sub(1)?;
        let rest = self.text.get(a..)?;
        let open = if rest.starts_with("![[") {
            3
        } else if rest.starts_with("[[") {
            2
        } else {
            return None;
        };
        let line = &rest[..rest.find('\n').unwrap_or(rest.len())];
        let e = line[open..].find("]]")?;
        Some(a..a + open + e + 2)
    }

    /// Front matter as properties. It is YAML, not Markdown, so its links come from the text:
    /// Obsidian counts them as links too.
    fn front_matter(&mut self, fm: &str) -> Block {
        let at = self.text.starts_with(fm);
        for r in wiki_spans(fm) {
            let target = link_target(&fm[r.start + 2..r.end - 2]);
            self.facts.links.push(WikiRef { target, range: at.then_some(r) });
        }
        Block::Props { line: 0, props: props(fm) }
    }
}

fn push_text(out: &mut Vec<Inline>, s: &str) {
    if let Some(Inline::Text(t)) = out.last_mut() {
        t.push_str(s);
    } else {
        out.push(Inline::Text(s.to_string()));
    }
}

/// Text with its `#tags`: a `#` that follows no letter, digit, `_`, `#`, `&` or `/`, then a
/// letter, then letters, digits, `_`, `/` or `-`.
fn tagify(s: &str, out: &mut Vec<Inline>, tags: &mut Vec<String>) {
    let b = s.as_bytes();
    let mut last = 0;
    let mut i = 0;
    while i < b.len() {
        let tag = b[i] == b'#'
            && !prev_char(s, i).is_some_and(|p| is_word(p) || p == '#' || p == '&' || p == '/')
            && b.get(i + 1).is_some_and(u8::is_ascii_alphabetic);
        if !tag {
            i += 1;
            continue;
        }
        let j = scan(b, i + 2, |x| x.is_ascii_alphanumeric() || x == b'_' || x == b'/' || x == b'-');
        if i > last {
            push_text(out, &s[last..i]);
        }
        let t = &s[i + 1..j];
        let low = t.to_lowercase();
        if !tags.contains(&low) {
            tags.push(low);
        }
        out.push(Inline::Tag(t.to_string()));
        last = j;
        i = j;
    }
    if last < s.len() {
        push_text(out, &s[last..]);
    }
}

/// The line breaks inside an inline.
fn breaks(i: &Inline) -> usize {
    match i {
        Inline::Break => 1,
        Inline::Strong(k) | Inline::Emph(k) | Inline::Strike(k) | Inline::Link { kids: k, .. } => k.iter().map(breaks).sum(),
        _ => 0,
    }
}

/// A paragraph as blocks: a line of nothing but images is a block of its own, and the lines
/// around it stay paragraphs.
fn split_paragraph(line: usize, inlines: Vec<Inline>, out: &mut Vec<Block>) {
    // `pretty` for the last line: its last two words stay together.
    let finish = |mut v: Vec<Inline>| {
        let more = v.len() > 1;
        if let Some(Inline::Text(t)) = v.last_mut() {
            let s = t.trim_end();
            *t = match s.rfind(' ') {
                Some(i) if more && !s[..i].trim().is_empty() => format!("{}\u{a0}{}", &s[..i], &s[i + 1..]),
                _ => pretty(t),
            };
        }
        v
    };
    if !inlines.iter().any(|i| matches!(i, Inline::Image(_))) {
        out.push(Block::Para { line, inlines: finish(inlines) });
        return;
    }
    let mut lines: Vec<(usize, Vec<Inline>)> = vec![(line, Vec::new())];
    let mut at = line;
    for i in inlines {
        if i == Inline::Break {
            at += 1;
            lines.push((at, Vec::new()));
            continue;
        }
        at += breaks(&i);
        if let Some(l) = lines.last_mut() {
            l.1.push(i);
        }
    }
    let mut para: Option<(usize, Vec<Inline>)> = None;
    for (ln, l) in lines {
        let only_images = l.iter().any(|i| matches!(i, Inline::Image(_)))
            && l.iter().all(|i| match i {
                Inline::Image(_) => true,
                Inline::Text(t) => t.trim().is_empty(),
                _ => false,
            });
        if only_images {
            if let Some((pl, pi)) = para.take() {
                out.push(Block::Para { line: pl, inlines: finish(pi) });
            }
            let images = l.into_iter().filter_map(|i| if let Inline::Image(m) = i { Some(m) } else { None }).collect();
            out.push(Block::Image { line: ln, images });
        } else if let Some((_, pi)) = &mut para {
            pi.push(Inline::Break);
            pi.extend(l);
        } else {
            para = Some((ln, l));
        }
    }
    if let Some((pl, pi)) = para {
        out.push(Block::Para { line: pl, inlines: finish(pi) });
    }
}

/// The `[[…]]` in a text, each on one line.
fn wiki_spans(s: &str) -> Vec<Range<usize>> {
    let mut out = Vec::new();
    let mut i = 0;
    while let Some(a) = s[i..].find("[[").map(|a| a + i) {
        let after = &s[a + 2..];
        match after.find([']', '\n']) {
            Some(e) if e > 0 && after[e..].starts_with("]]") => {
                out.push(a..a + 2 + e + 2);
                i = a + 2 + e + 2;
            }
            _ => i = a + 2,
        }
    }
    out
}

fn unquote(s: &str) -> &str {
    let s = s.trim();
    let b = s.as_bytes();
    if b.len() >= 2 && (b[0] == b'"' || b[0] == b'\'') && b[b.len() - 1] == b[0] { &s[1..s.len() - 1] } else { s }
}

/// Front matter's properties, read as Obsidian writes them: `key: value`, `key: [a, b]`, a list
/// of `- item` lines, or a value continued on indented lines.
fn props(fm: &str) -> Vec<(String, Vec<Inline>)> {
    let mut out: Vec<(String, Vec<String>)> = Vec::new();
    for l in fm.lines().skip(1) {
        let t = l.trim();
        if t == "---" || t == "..." {
            break;
        }
        if t.is_empty() || t.starts_with('#') {
            continue;
        }
        if let Some(item) = t.strip_prefix("- ").or((t == "-").then_some("")) {
            if let Some(last) = out.last_mut() {
                last.1.push(unquote(item).to_string());
            }
            continue;
        }
        if !l.starts_with([' ', '\t'])
            && let Some((k, v)) = t.split_once(':')
        {
            let v = v.trim();
            let vals = if v.is_empty() || v.starts_with(['|', '>']) {
                Vec::new()
            } else if v.starts_with('[') && !v.starts_with("[[") && v.ends_with(']') {
                v[1..v.len() - 1].split(',').map(unquote).filter(|x| !x.is_empty()).map(str::to_string).collect()
            } else {
                vec![unquote(v).to_string()]
            };
            out.push((unquote(k).to_string(), vals));
            continue;
        }
        // A value continued on the next line.
        if let Some(last) = out.last_mut() {
            match last.1.last_mut() {
                Some(x) => {
                    x.push(' ');
                    x.push_str(unquote(t));
                }
                None => last.1.push(unquote(t).to_string()),
            }
        }
    }
    out.into_iter().map(|(k, vs)| (k, value_inlines(&vs.join(", ")))).collect()
}

/// A property's value: text, with its `[[links]]`.
fn value_inlines(s: &str) -> Vec<Inline> {
    let mut out = Vec::new();
    let mut last = 0;
    for r in wiki_spans(s) {
        if r.start > last {
            out.push(Inline::Text(s[last..r.start].to_string()));
        }
        let inner = &s[r.start + 2..r.end - 2];
        let target = link_target(inner);
        let shown = inner.split_once('|').map_or("", |(_, a)| a.trim());
        out.push(Inline::Wiki { shown: if shown.is_empty() { target.clone() } else { shown.to_string() }, target });
        last = r.end;
    }
    if last < s.len() {
        out.push(Inline::Text(s[last..].to_string()));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn blocks(t: &str) -> Vec<Block> {
        parse(t).0
    }

    /// Inlines as text, with each line break as a newline.
    fn lines(v: &[Inline]) -> String {
        v.iter().map(|i| if *i == Inline::Break { "\n".to_string() } else { plain(std::slice::from_ref(i)) }).collect()
    }

    fn text(s: &str) -> Inline {
        Inline::Text(s.into())
    }

    #[test]
    fn tables_parse_with_alignment() {
        let b = blocks("| A | B | C |\n|:--|:-:|--:|\n| 1 | **2** | [[N\\|n]] |\n| x |");
        match &b[..] {
            [Block::Table { line: 0, align, rows }] => {
                assert_eq!(align, &[Align::Left, Align::Center, Align::Right]);
                assert_eq!(rows.len(), 3);
                assert_eq!(rows[0][0], vec![text("A")]);
                assert_eq!(rows[1][1], vec![Inline::Strong(vec![text("2")])]);
                assert_eq!(rows[1][2], vec![Inline::Wiki { target: "N".into(), shown: "n".into() }]);
                assert_eq!(rows[2].len(), 3, "a short row gets empty cells");
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn front_matter_shows_as_properties() {
        let t = "---\ntags: [a, \"b\"]\nup: \"[[Home|home]]\"\nlist:\n  - one\n  - two\ntext: |\n  long\n  more\n---\n# Title\n";
        let (b, f) = parse(t);
        match &b[..] {
            [Block::Props { line: 0, props }, Block::Heading { line: 10, level: 1, .. }] => {
                let keys: Vec<&str> = props.iter().map(|(k, _)| k.as_str()).collect();
                assert_eq!(keys, ["tags", "up", "list", "text"]);
                assert_eq!(props[0].1, vec![text("a, b")]);
                assert_eq!(props[1].1, vec![Inline::Wiki { target: "Home".into(), shown: "home".into() }]);
                assert_eq!(props[2].1, vec![text("one, two")]);
                assert_eq!(props[3].1, vec![text("long more")]);
            }
            other => panic!("{other:?}"),
        }
        assert_eq!(f.links.len(), 1);
        assert_eq!(&t[f.links[0].range.clone().unwrap()], "[[Home|home]]");
        assert_eq!(f.headings, vec![Heading { level: 1, text: "Title".into(), line: 10 }]);
    }

    #[test]
    fn code_holds_no_tags_or_links() {
        let t = "A #real tag and [[Linked]].\n\n```\n#fake [[Fake]]\n```\n\nInline `#nope [[Nope]]` and [a #tag in a link](https://x.org).\n\n    #indented [[Code]]";
        let (_, f) = parse(t);
        assert_eq!(f.tags, ["real"]);
        let targets: Vec<&str> = f.links.iter().map(|l| l.target.as_str()).collect();
        assert_eq!(targets, ["Linked"]);
    }

    #[test]
    fn tags_follow_one_rule() {
        let (b, f) = parse("#Top starts it, a#b, C# and (#Paren) x/#no #top #a/b-c");
        assert_eq!(f.tags, ["top", "paren", "a/b-c"]);
        match &b[..] {
            [Block::Para { inlines, .. }] => assert_eq!(inlines[0], Inline::Tag("Top".into())),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn links_rewrite_everywhere_but_code() {
        let t = "---\nup: \"[[Old]]\"\n---\nSee [[Old|o]], [[old]], [[Other]] and ![[Old]].\n\n| a |\n|---|\n| [[Old\\|x]] |\n\n- > [[Old]] in a quote in a list\n  - **[[Old#Part]]** [[Old]]\n\n`[[Old]]`\n\n```\n[[Old]]\n```\n";
        let want = "---\nup: \"[[New]]\"\n---\nSee [[New|o]], [[New]], [[Other]] and ![[New]].\n\n| a |\n|---|\n| [[New\\|x]] |\n\n- > [[New]] in a quote in a list\n  - **[[Old#Part]]** [[New]]\n\n`[[Old]]`\n\n```\n[[Old]]\n```\n";
        assert_eq!(rewrite_links(t, "Old", "New"), want);
        let mut files = BTreeMap::new();
        files.insert("a.md".to_string(), t.to_string());
        files.insert("Old.md".to_string(), "[[Old]]".to_string());
        assert_eq!(backlinks(&files, "Old.md"), vec![("a.md".to_string(), 7)]);
    }

    #[test]
    fn lists_nest() {
        let b = blocks("- [ ] open\n  - [x] done\n    ```\n    code\n    ```\n\n3) three\n4) four");
        match &b[..] {
            [Block::List { ordered: false, items, .. }, Block::List { ordered: true, start: 3, delim: ')', items: n, .. }] => {
                assert_eq!(items[0].task, Some(false));
                match &items[0].blocks[..] {
                    [Block::Para { inlines, .. }, Block::List { items: inner, .. }] => {
                        assert_eq!(lines(inlines), "open");
                        assert_eq!((inner[0].task, inner[0].line), (Some(true), 1));
                        assert!(matches!(&inner[0].blocks[1], Block::Pre { line: 2, text } if text == "code"), "{inner:?}");
                    }
                    other => panic!("{other:?}"),
                }
                assert_eq!((n.len(), n[1].line), (2, 7));
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn line_breaks_stay_and_setext_headings_read() {
        let b = blocks("one\ntwo  \nthree\\\nfour\\\n\nTitle\n---\n> q1\n> q2\n\n- a\n  more a\nlazy a\n- b");
        match &b[..] {
            [Block::Para { inlines, .. }, Block::Heading { line: 5, level: 2, .. }, Block::Quote { line: 7, blocks: q }, Block::List { tight: true, items, .. }] => {
                assert_eq!(lines(inlines), "one\ntwo\nthree\nfour\\");
                assert!(matches!(&q[..], [Block::Para { inlines, .. }] if lines(inlines) == "q1\nq2"));
                assert!(matches!(&items[0].blocks[..], [Block::Para { inlines, .. }] if lines(inlines) == "a\nmore a\nlazy\u{a0}a"));
                assert_eq!((items[0].line, items[1].line), (10, 13));
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn bare_urls_link() {
        match &blocks("See https://example.com/a now.")[..] {
            [Block::Para { inlines, .. }] => {
                assert!(matches!(&inlines[1], Inline::Link { url, .. } if url == "https://example.com/a"), "{inlines:?}");
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn only_double_tildes_strike() {
        let t = |s: &str| Inline::Text(s.into());
        let src = "~5 or ~6 nights, **~¥3 (~₱1)** and ~~gone~~ `~/x` [a](http://h/~u) ~[[N~1]]\n\n```\n~ ~\n```\n";
        match &blocks(src)[..] {
            [Block::Para { inlines, .. }, Block::Pre { text, .. }] => {
                assert_eq!(&inlines[..7], &[
                    t("~5 or ~6 nights, "),
                    Inline::Strong(vec![t("~¥3 (~₱1)")]),
                    t(" and "),
                    Inline::Strike(vec![t("gone")]),
                    t(" "),
                    Inline::Code("~/x".into()),
                    t(" "),
                ]);
                assert_eq!(inlines[7], Inline::Link { url: "http://h/~u".into(), kids: vec![t("a")] });
                assert_eq!(inlines[9], Inline::Wiki { target: "N~1".into(), shown: "N~1".into() });
                assert_eq!(text, "~ ~");
            }
            other => panic!("{other:?}"),
        }
        // A note that already uses every mark keeps GFM's reading.
        assert!(matches!(&blocks("^$% ~a~")[..], [Block::Para { inlines, .. }] if matches!(inlines[1], Inline::Strike(_))));
        // Offsets hold, so a rename still finds the link.
        assert_eq!(rewrite_links("~a [[x]] ~b", "x", "y"), "~a [[y]] ~b");
    }

    fn images(src: &str) -> Option<Vec<Img>> {
        match &blocks(src)[..] {
            [Block::Image { images, .. }] => Some(images.clone()),
            _ => None,
        }
    }

    #[test]
    fn image_lines_are_blocks() {
        let one = images("![A cat](attachments/My%20cat.png)").unwrap();
        assert_eq!(one, vec![Img { alt: "A cat".into(), src: "attachments/My cat.png".into(), embed: false }]);
        let two = images("  ![](a.png \"Title\")  ![[Photos/b.jpg|300]]").unwrap();
        assert_eq!(two[0].src, "a.png");
        assert_eq!(two[1], Img { alt: "b".into(), src: "Photos/b.jpg".into(), embed: true });
        assert_eq!(images("![x](<with space.png>)").unwrap()[0].src, "with space.png");
        assert_eq!(images("![x](https://example.com/a%20b.png)").unwrap()[0].src, "https://example.com/a%20b.png");
        assert!(images("text ![x](a.png)").is_none());
        assert!(images("![x](a.png) and text").is_none());
        assert!(images("![[Note]]").is_none(), "an embed of a note is not an image");
        let b = blocks("Text\n![a](a.png)\nmore");
        assert!(matches!(&b[..], [Block::Para { line: 0, .. }, Block::Image { line: 1, .. }, Block::Para { line: 2, .. }]), "{b:?}");
    }

    #[test]
    fn headings_name_their_text() {
        let (_, f) = parse("# One **b** [[x|y]]\n\n```\n# not\n```\n\n> ## Quoted");
        assert_eq!(f.headings, vec![Heading { level: 1, text: "One b y".into(), line: 0 }, Heading { level: 2, text: "Quoted".into(), line: 6 }]);
    }
}
