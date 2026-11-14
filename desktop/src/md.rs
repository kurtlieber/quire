//! Markdown analysis shared by the editor, the preview, the panels and the MCP server.
//! The preview, the outline, tags, backlinks and the rewrite on rename read the tree in `gfm`.
//! The line-by-line helpers here serve the editor's source mode and come from the design
//! prototype.

use std::collections::BTreeMap;

mod gfm;
pub use gfm::{Align, Block, Inline, backlinks, headings, plain, rewrite_links, tags, tree};

/// The file name of a note without its folder and without `.md`.
pub fn base(p: &str) -> &str {
    let name = p.rsplit('/').next().unwrap_or(p);
    strip_md(name)
}

/// The folder part of a note path, or an empty string for a root note.
pub fn dir(p: &str) -> &str {
    p.rfind('/').map_or("", |i| &p[..i])
}

pub fn join(d: &str, n: &str) -> String {
    if d.is_empty() { n.to_string() } else { format!("{d}/{n}") }
}

pub fn strip_md(p: &str) -> &str {
    if p.len() >= 3 && p[p.len() - 3..].eq_ignore_ascii_case(".md") { &p[..p.len() - 3] } else { p }
}

/// Makes a name safe as a file name: path separators and colons become dashes.
pub fn clean_name(s: &str) -> String {
    s.chars().map(|c| if matches!(c, '\\' | '/' | ':') { '-' } else { c }).collect::<String>().trim().to_string()
}

fn is_word(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

/// Lowercased lookup from a note name (or a path without `.md`) to its path.
#[derive(Default, Clone)]
pub struct Names(BTreeMap<String, String>);

impl Names {
    pub fn build(files: &BTreeMap<String, String>) -> Names {
        let mut m = BTreeMap::new();
        for p in files.keys() {
            m.entry(base(p).to_lowercase()).or_insert_with(|| p.clone());
            m.insert(strip_md(p).to_lowercase(), p.clone());
        }
        Names(m)
    }
    pub fn resolve(&self, name: &str) -> Option<&str> {
        self.0.get(&name.trim().to_lowercase()).map(String::as_str)
    }
}

/// The target of a `[[link|alias]]`, without the alias.
pub fn link_target(inner: &str) -> String {
    inner.split('|').next().unwrap_or("").trim().replace('\u{a0}', " ")
}

/// CSS `text-wrap: pretty` keeps a paragraph from ending on one word. Joining the last two
/// words with a no-break space has the same effect on the last line.
pub fn pretty(t: &str) -> String {
    let t = t.trim_end();
    match t.rfind(' ') {
        Some(i) if t[..i].trim_end().contains(' ') => format!("{}\u{a0}{}", &t[..i], &t[i + 1..]),
        _ => t.to_string(),
    }
}

// ---------- Inline tokens ----------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tok {
    Code,
    Wiki,
    Bold,
    Italic,
    Strike,
    Link,
    Tag,
}

/// Finds inline Markdown tokens in one line, as the prototype's `inlineRe` does: the leftmost
/// match wins, and at one position the first kind in this order.
pub fn inline_tokens(s: &str) -> Vec<(Tok, std::ops::Range<usize>)> {
    let b = s.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        if let Some((t, end)) = token_at(s, i) {
            out.push((t, i..end));
            i = end;
        } else {
            i += s[i..].chars().next().map_or(1, char::len_utf8);
        }
    }
    out
}

fn prev_char(s: &str, i: usize) -> Option<char> {
    s[..i].chars().next_back()
}

/// Index of the first byte at or after `from` that is `stop`, scanning while `ok` holds.
fn scan(b: &[u8], from: usize, ok: impl Fn(u8) -> bool) -> usize {
    let mut j = from;
    while j < b.len() && ok(b[j]) {
        j += 1;
    }
    j
}

fn token_at(s: &str, i: usize) -> Option<(Tok, usize)> {
    let b = s.as_bytes();
    let at = |k: usize| b.get(k).copied();
    let c = b[i];
    // `code`
    if c == b'`' {
        let j = scan(b, i + 1, |x| x != b'`' && x != b'\n');
        if j > i + 1 && at(j) == Some(b'`') {
            return Some((Tok::Code, j + 1));
        }
    }
    // [[wiki]]
    if c == b'[' && at(i + 1) == Some(b'[') {
        let j = scan(b, i + 2, |x| x != b']' && x != b'\n');
        if j > i + 2 && at(j) == Some(b']') && at(j + 1) == Some(b']') {
            return Some((Tok::Wiki, j + 2));
        }
    }
    // **bold** or __bold__
    if (c == b'*' || c == b'_') && at(i + 1) == Some(c) {
        let j = scan(b, i + 2, |x| x != c && x != b'\n');
        if j > i + 2 && at(j) == Some(c) && at(j + 1) == Some(c) {
            return Some((Tok::Bold, j + 2));
        }
    }
    // *italic*
    if c == b'*' {
        if let Some(n) = at(i + 1) {
            if n != b'*' && !(n as char).is_whitespace() {
                let j = scan(b, i + 2, |x| x != b'*' && x != b'\n');
                if at(j) == Some(b'*') {
                    return Some((Tok::Italic, j + 1));
                }
            }
        }
    }
    // _italic_ with no word character on either side
    if c == b'_' && !prev_char(s, i).is_some_and(is_word) {
        if let Some(n) = at(i + 1) {
            if n != b'_' && !(n as char).is_whitespace() {
                let j = scan(b, i + 2, |x| x != b'_' && x != b'\n');
                if at(j) == Some(b'_') && !s[j + 1..].chars().next().is_some_and(is_word) {
                    return Some((Tok::Italic, j + 1));
                }
            }
        }
    }
    // ~~strike~~
    if c == b'~' && at(i + 1) == Some(b'~') {
        let j = scan(b, i + 2, |x| x != b'~' && x != b'\n');
        if j > i + 2 && at(j) == Some(b'~') && at(j + 1) == Some(b'~') {
            return Some((Tok::Strike, j + 2));
        }
    }
    // [text](url)
    if c == b'[' {
        let j = scan(b, i + 1, |x| x != b']' && x != b'\n');
        if j > i + 1 && at(j) == Some(b']') && at(j + 1) == Some(b'(') {
            let k = scan(b, j + 2, |x| x != b')' && !(x as char).is_whitespace());
            if k > j + 2 && at(k) == Some(b')') {
                return Some((Tok::Link, k + 1));
            }
        }
    }
    // #tag
    if c == b'#' && !prev_char(s, i).is_some_and(|p| is_word(p) || p == '#' || p == '&' || p == '/') {
        if at(i + 1).is_some_and(|x| x.is_ascii_alphabetic()) {
            let j = scan(b, i + 2, |x| x.is_ascii_alphanumeric() || x == b'_' || x == b'/' || x == b'-');
            return Some((Tok::Tag, j));
        }
    }
    None
}

// ---------- Line kinds (source highlighting and list continuation) ----------

/// `^(#{1,6})(\s+)(.*)$`: the marker with its spaces, and the heading text.
pub fn heading_line(ln: &str) -> Option<(usize, usize)> {
    let h = ln.bytes().take_while(|&c| c == b'#').count();
    if !(1..=6).contains(&h) {
        return None;
    }
    let sp = ln[h..].chars().take_while(|c| c.is_whitespace()).map(char::len_utf8).sum::<usize>();
    if sp == 0 {
        return None;
    }
    Some((h, h + sp))
}

/// `^\s*([-*_])(\s*\1){2,}\s*$`
pub fn is_hr(ln: &str) -> bool {
    let t: Vec<char> = ln.chars().filter(|c| !c.is_whitespace()).collect();
    t.len() >= 3 && matches!(t[0], '-' | '*' | '_') && t.iter().all(|&c| c == t[0])
}

/// `^(\s*>+\s?)`: the length of the quote prefix.
pub fn quote_prefix(ln: &str) -> Option<usize> {
    let ws = ln.len() - ln.trim_start().len();
    let rest = &ln[ws..];
    let g = rest.bytes().take_while(|&c| c == b'>').count();
    if g == 0 {
        return None;
    }
    let mut end = ws + g;
    if let Some(c) = ln[end..].chars().next() {
        if c.is_whitespace() {
            end += c.len_utf8();
        }
    }
    Some(end)
}

/// A list line: `^(\s*)([-*+]|\d+[.)])(\s+)(\[[ xX]\]\s)?(.*)$`.
#[derive(Clone, Debug)]
pub struct ListLine {
    pub indent: usize,
    /// The marker's byte range.
    pub marker: std::ops::Range<usize>,
    /// The end of the spaces after the marker.
    pub after: usize,
    /// The task box `[ ] ` byte range, with one trailing space.
    pub task: Option<std::ops::Range<usize>>,
    pub done: bool,
    /// The number of an ordered marker and its closing `.` or `)`.
    pub number: Option<(u64, char)>,
}

pub fn list_line(ln: &str) -> Option<ListLine> {
    let b = ln.as_bytes();
    let indent = ln.len() - ln.trim_start().len();
    let mut i = indent;
    let start = i;
    let mut number = None;
    match b.get(i) {
        Some(b'-' | b'*' | b'+') => i += 1,
        Some(c) if c.is_ascii_digit() => {
            let j = scan(b, i, |x| x.is_ascii_digit());
            match b.get(j) {
                Some(&p @ (b'.' | b')')) => {
                    number = Some((ln[i..j].parse::<u64>().unwrap_or(0), p as char));
                    i = j + 1;
                }
                _ => return None,
            }
        }
        _ => return None,
    }
    let marker = start..i;
    let sp = ln[i..].chars().take_while(|c| c.is_whitespace()).map(char::len_utf8).sum::<usize>();
    if sp == 0 {
        return None;
    }
    let after = i + sp;
    let mut task = None;
    let mut done = false;
    if b.get(after) == Some(&b'[') && b.get(after + 2) == Some(&b']') {
        if let Some(&m) = b.get(after + 1) {
            if matches!(m, b' ' | b'x' | b'X') {
                if let Some(c) = ln[after + 3..].chars().next() {
                    if c.is_whitespace() {
                        task = Some(after..after + 3 + c.len_utf8());
                        done = m != b' ';
                    }
                }
            }
        }
    }
    Some(ListLine { indent, marker, after, task, done, number })
}

/// A fence line opens or closes a code block: `^\s*(```|~~~)`.
pub fn is_fence(ln: &str) -> bool {
    let t = ln.trim_start();
    t.starts_with("```") || t.starts_with("~~~")
}

/// Counts words as the prototype does: letter or number runs, fenced code left out.
pub fn words(t: &str) -> usize {
    let mut text = String::with_capacity(t.len());
    let mut fence = false;
    let mut lines = t.split('\n').peekable();
    while let Some(ln) = lines.next() {
        if ln.starts_with("```") || (fence && ln.contains("```")) {
            fence = !fence;
            text.push(' ');
            continue;
        }
        if !fence {
            text.push_str(ln);
        }
        text.push('\n');
    }
    let mut n = 0;
    let mut in_word = false;
    for c in text.chars() {
        let starts = c.is_alphanumeric();
        let cont = starts || c == '\'' || c == '’' || c == '-';
        if in_word {
            if !cont {
                in_word = false;
            }
        } else if starts {
            in_word = true;
            n += 1;
        }
    }
    n
}

/// The paragraph (run of non-blank lines) around byte `pos`, as first and last line index.
pub fn para_range(text: &str, pos: usize) -> (usize, usize) {
    let lines: Vec<&str> = text.split('\n').collect();
    let mut li = 0;
    let mut acc = 0;
    while li < lines.len() - 1 {
        if pos <= acc + lines[li].len() {
            break;
        }
        acc += lines[li].len() + 1;
        li += 1;
    }
    if lines[li].trim().is_empty() {
        return (li, li);
    }
    let (mut a, mut b) = (li, li);
    while a > 0 && !lines[a - 1].trim().is_empty() {
        a -= 1;
    }
    while b < lines.len() - 1 && !lines[b + 1].trim().is_empty() {
        b += 1;
    }
    (a, b)
}

/// Fuzzy match of `q` in `s`: a score and the matched character indices.
pub fn fuzzy(q: &str, s: &str) -> Option<(f32, Vec<usize>)> {
    let ql: Vec<char> = q.to_lowercase().chars().collect();
    let sl: Vec<char> = s.to_lowercase().chars().collect();
    if ql.is_empty() {
        return Some((0.0, vec![]));
    }
    let slen = s.chars().count() as f32;
    if let Some(sub) = sl.windows(ql.len()).position(|w| w == ql.as_slice()) {
        let idx = (sub..sub + ql.len()).collect();
        let score = 1000.0 - sub as f32 * 2.0 - slen * 0.1 + if sub == 0 { 50.0 } else { 0.0 };
        return Some((score, idx));
    }
    let (mut j, mut prev, mut score) = (0usize, -2i64, 0.0f32);
    let mut idx = Vec::new();
    for (i, &c) in sl.iter().enumerate() {
        if j >= ql.len() {
            break;
        }
        if c == ql[j] {
            idx.push(i);
            let boundary = i == 0 || matches!(sl[i - 1], ' ' | '\t' | '/' | '_' | '-');
            score += if i as i64 == prev + 1 { 4.0 } else { 1.0 } + if boundary { 3.0 } else { 0.0 };
            prev = i as i64;
            j += 1;
        }
    }
    (j == ql.len()).then_some((score, idx))
}

#[derive(Clone, Debug)]
pub struct Hit {
    pub line: usize,
    /// Byte offset of the match in its line.
    pub col: usize,
    /// Byte offset of the match in the note.
    pub pos: usize,
}

#[derive(Clone, Debug)]
pub struct SearchResult {
    pub path: String,
    pub hits: Vec<Hit>,
    pub name_hit: bool,
}

/// Case-insensitive full-text search, sorted as the prototype sorts.
pub fn search(files: &BTreeMap<String, String>, q: &str) -> Vec<SearchResult> {
    let ql = q.to_lowercase();
    let mut out = Vec::new();
    if ql.is_empty() {
        return out;
    }
    for (p, t) in files {
        // A locked note's text is sealed, so search has nothing to find in it.
        if t.starts_with(crate::lock::MAGIC) {
            continue;
        }
        let mut hits = Vec::new();
        let mut off = 0;
        for (li, ln) in t.split('\n').enumerate() {
            let low = ln.to_lowercase();
            // Lowercasing can change byte lengths; only trust offsets when it does not.
            if low.len() == ln.len() {
                let mut from = 0;
                while let Some(i) = low[from..].find(&ql) {
                    let col = from + i;
                    hits.push(Hit { line: li, col, pos: off + col });
                    from = col + ql.len().max(1);
                    if from >= low.len() {
                        break;
                    }
                }
            } else if low.contains(&ql) {
                hits.push(Hit { line: li, col: 0, pos: off });
            }
            off += ln.len() + 1;
        }
        let name_hit = base(p).to_lowercase().contains(&ql);
        if !hits.is_empty() || name_hit {
            out.push(SearchResult { path: p.clone(), hits, name_hit });
        }
    }
    out.sort_by(|a, b| {
        (b.name_hit as u8).cmp(&(a.name_hit as u8)).then(b.hits.len().cmp(&a.hits.len())).then_with(|| a.path.cmp(&b.path))
    });
    out
}

/// Line index and byte offset of the line start for byte `pos`.
pub fn line_col(text: &str, pos: usize) -> (usize, usize) {
    let pos = pos.min(text.len());
    let mut line = 0;
    let mut ls = 0;
    for (i, b) in text.as_bytes()[..pos].iter().enumerate() {
        if *b == b'\n' {
            line += 1;
            ls = i + 1;
        }
    }
    (line, ls)
}

/// Byte offset where line `line` starts.
pub fn line_start(text: &str, line: usize) -> usize {
    let mut pos = 0;
    for _ in 0..line {
        match text[pos..].find('\n') {
            Some(n) => pos += n + 1,
            None => break,
        }
    }
    pos
}

/// Flips the first task box on line `line`.
pub fn toggle_task(text: &str, line: usize) -> String {
    let mut lines: Vec<String> = text.split('\n').map(str::to_string).collect();
    if let Some(l) = lines.get_mut(line) {
        if let Some(i) = l.find('[') {
            let b = l.as_bytes();
            if b.get(i + 2) == Some(&b']') {
                match b.get(i + 1) {
                    Some(b' ') => l.replace_range(i + 1..i + 2, "x"),
                    Some(b'x' | b'X') => l.replace_range(i + 1..i + 2, " "),
                    _ => {}
                }
            }
        }
    }
    lines.join("\n")
}

/// The file types the preview shows as images, and that sync carries next to the notes.
pub const IMAGE_EXT: [&str; 8] = ["png", "jpg", "jpeg", "gif", "webp", "bmp", "svg", "avif"];

/// Whether a path names an image file, by its extension.
pub fn is_image(path: &str) -> bool {
    path.rsplit_once('.').is_some_and(|(_, e)| IMAGE_EXT.iter().any(|x| x.eq_ignore_ascii_case(e)))
}

/// An image in a note: `![alt](src)`, or `![[name]]` as other Markdown apps write an embed.
#[derive(Clone, Debug, PartialEq)]
pub struct Img {
    pub alt: String,
    pub src: String,
    /// Written as `![[name]]`: `src` is a file name to find anywhere in the folder.
    pub embed: bool,
}

fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() && b[i + 1].is_ascii_hexdigit() && b[i + 2].is_ascii_hexdigit() {
            let hexval = |c: u8| (c as char).to_digit(16).unwrap_or(0) as u8;
            out.push(hexval(b[i + 1]) * 16 + hexval(b[i + 2]));
            i += 3;
            continue;
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8(out).unwrap_or_else(|_| s.to_string())
}

/// A path for a Markdown link: spaces and the characters that end a link become escapes.
pub fn link_path(p: &str) -> String {
    let mut out = String::with_capacity(p.len());
    for c in p.chars() {
        match c {
            ' ' => out.push_str("%20"),
            '(' => out.push_str("%28"),
            ')' => out.push_str("%29"),
            '<' => out.push_str("%3C"),
            '>' => out.push_str("%3E"),
            _ => out.push(c),
        }
    }
    out
}

/// `to` written relative to the folder `from_dir`, both relative to the notes folder, with
/// `../` to go up.
pub fn relative(from_dir: &str, to: &str) -> String {
    let a: Vec<&str> = from_dir.split('/').filter(|s| !s.is_empty()).collect();
    let b: Vec<&str> = to.split('/').filter(|s| !s.is_empty()).collect();
    let same = a.iter().zip(&b).take_while(|(x, y)| x == y).count();
    let mut parts: Vec<&str> = vec![".."; a.len() - same];
    parts.extend(&b[same..]);
    parts.join("/")
}

/// `rel` inside the folder `dir`, with `.` and `..` resolved. None when it leaves the notes
/// folder.
pub fn resolve(dir: &str, rel: &str) -> Option<String> {
    let mut parts: Vec<&str> = if rel.starts_with('/') { Vec::new() } else { dir.split('/').filter(|s| !s.is_empty()).collect() };
    for seg in rel.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            s => parts.push(s),
        }
    }
    Some(parts.join("/"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens_follow_the_prototype_order() {
        let s = "See [[Ideas]] and **bold** with *em* and #tag, `code` ~~x~~ [a](b)";
        let t: Vec<Tok> = inline_tokens(s).into_iter().map(|x| x.0).collect();
        assert_eq!(t, vec![Tok::Wiki, Tok::Bold, Tok::Italic, Tok::Tag, Tok::Code, Tok::Strike, Tok::Link]);
        assert!(inline_tokens("a#b").is_empty());
        assert!(inline_tokens("snake_case_name").is_empty());
    }

    #[test]
    fn list_lines_parse_tasks_and_numbers() {
        let l = list_line("  - [x] done").unwrap();
        assert!(l.done && l.task.is_some());
        assert_eq!(list_line("12. twelve").unwrap().number, Some((12, '.')));
        assert!(list_line("-nope").is_none());
    }

    #[test]
    fn words_and_paragraphs() {
        assert_eq!(words("Hello world, it's 2026.\n```\nskip me\n```\nend"), 5);
        assert_eq!(para_range("a\nb\n\nc", 0), (0, 1));
    }

    #[test]
    fn links_rewrite_with_alias() {
        assert_eq!(rewrite_links("x [[Old|o]] [[old]] [[Other]]", "Old", "New"), "x [[New|o]] [[New]] [[Other]]");
    }

    #[test]
    fn fuzzy_prefers_substrings() {
        let (a, idx) = fuzzy("slo", "On slow software").unwrap();
        assert_eq!(idx, vec![3, 4, 5]);
        let (b, _) = fuzzy("oss", "On slow software").unwrap();
        assert!(a > b);
        assert!(fuzzy("zz", "abc").is_none());
    }
}

#[cfg(test)]
mod path_tests {
    use super::*;

    #[test]
    fn paths_and_escapes() {
        assert_eq!(percent_decode("%E2%9C%93 100%"), "✓ 100%");
        assert_eq!(percent_decode("%é"), "%é");
        assert_eq!(relative("Drafts/Old", "attachments/a.png"), "../../attachments/a.png");
        assert_eq!(relative("", "attachments/a.png"), "attachments/a.png");
        assert_eq!(resolve("Drafts", "../attachments/a.png").as_deref(), Some("attachments/a.png"));
        assert_eq!(resolve("", "../a.png"), None);
        assert_eq!(link_path("My cat (1).png"), "My%20cat%20%281%29.png");
        assert!(is_image("x/Y.JPG") && !is_image("x.md"));
    }
}
