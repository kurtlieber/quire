package app.quire.notes.core

/**
 * Pasted HTML to Markdown. A copy from a web page, a document or a mail app carries HTML next
 * to its plain text. Quire turns that HTML into Markdown, so headings, lists, links and emphasis
 * stay. HTML with no formatting, such as a copy from a code editor that only colours its text,
 * stays plain text. A port of desktop/src/html2md.rs. The output is the same for the same input.
 */
object Html2Md {
    /** A node of the parsed HTML. */
    private sealed class Node
    private class Text(val text: String) : Node()
    private class El(val tag: String, val attrs: List<Pair<String, String>>, val kids: List<Node>) : Node()

    private val VOID = setOf("br", "hr", "img", "input", "meta", "link", "wbr", "col", "source", "area", "base", "embed", "param", "track")
    private val BLOCK = setOf(
        "p", "div", "section", "article", "header", "footer", "main", "aside", "nav", "figure", "figcaption", "address", "center", "h1", "h2", "h3", "h4",
        "h5", "h6", "ul", "ol", "li", "blockquote", "pre", "hr", "table", "thead", "tbody", "tfoot", "tr", "dl", "dt", "dd",
    )

    /** Elements whose content never shows as text. */
    private val SKIP = setOf(
        "head", "title", "script", "style", "template", "noscript", "button", "select", "option", "textarea", "svg", "canvas", "iframe", "object", "video",
        "audio", "math",
    )

    // ---- Text helpers with the rules of Rust strings, so the output is the same. ----

    /** Unicode White_Space, as char::is_whitespace in Rust. */
    private fun ws(c: Char): Boolean = when (c.code) {
        in 0x09..0x0D, 0x20, 0x85, 0xA0, 0x1680, in 0x2000..0x200A, 0x2028, 0x2029, 0x202F, 0x205F, 0x3000 -> true
        else -> false
    }

    private fun asciiWs(c: Char) = c == ' ' || c == '\t' || c == '\n' || c == '\u000C' || c == '\r'
    private fun asciiAlnum(c: Char) = c in 'a'..'z' || c in 'A'..'Z' || c in '0'..'9'
    private fun lowerAscii(s: String) = buildString(s.length) { for (c in s) append(if (c in 'A'..'Z') c + 32 else c) }
    private fun trim(s: String) = s.trim { ws(it) }
    private fun trimStart(s: String) = s.trimStart { ws(it) }
    private fun trimEnd(s: String) = s.trimEnd { ws(it) }

    /** The lines of [s] as Rust str::lines gives them: no last empty line, and no "\r" at a line end. */
    private fun lines(s: String): List<String> {
        if (s.isEmpty()) return emptyList()
        val parts = s.split('\n').toMutableList()
        if (parts.last().isEmpty()) parts.removeAt(parts.size - 1)
        return parts.map { it.removeSuffix("\r") }
    }

    private fun splitWs(s: String): List<String> {
        val out = ArrayList<String>()
        val cur = StringBuilder()
        for (c in s) {
            if (ws(c)) {
                if (cur.isNotEmpty()) out.add(cur.toString())
                cur.clear()
            } else {
                cur.append(c)
            }
        }
        if (cur.isNotEmpty()) out.add(cur.toString())
        return out
    }

    private fun attr(attrs: List<Pair<String, String>>, name: String): String? = attrs.firstOrNull { it.first == name }?.second

    /** The value of one CSS property in a style attribute, in lower case. */
    private fun styleOf(attrs: List<Pair<String, String>>, prop: String): String? {
        val style = attr(attrs, "style") ?: return null
        for (d in style.split(';')) {
            val i = d.indexOf(':')
            if (i < 0) continue
            if (trim(d.substring(0, i)).equals(prop, ignoreCase = true) && trim(d.substring(0, i)).all { it.code < 128 }) {
                return lowerAscii(trim(d.substring(i + 1)))
            }
        }
        return null
    }

    private fun boldWeight(v: String): Boolean? {
        val first = splitWs(v).firstOrNull() ?: return null
        return when (first) {
            "bold", "bolder" -> true
            "normal", "lighter" -> false
            else -> first.toUIntOrNull()?.let { it >= 600u }
        }
    }

    private val NAMED = mapOf(
        "amp" to '&', "lt" to '<', "gt" to '>', "quot" to '"', "apos" to '\'', "nbsp" to ' ', "mdash" to '—', "ndash" to '–',
        "hellip" to '…', "lsquo" to '‘', "rsquo" to '’', "ldquo" to '“', "rdquo" to '”', "laquo" to '«', "raquo" to '»', "bull" to '•',
        "middot" to '·', "times" to '×', "copy" to '©', "reg" to '®', "trade" to '™', "euro" to '€', "deg" to '°',
    )

    /** A number as Rust parses an u32: digits with an optional "+". */
    private fun u32(s: String, radix: Int): Long? {
        val t = if (s.startsWith("+")) s.substring(1) else s
        if (t.isEmpty() || t.any { Character.digit(it, radix) < 0 || it.code >= 128 }) return null
        val v = t.toBigInteger(radix)
        return if (v > 0xFFFFFFFFL.toBigInteger()) null else v.toLong()
    }

    /** The character entities of HTML, decoded. An unknown entity stays as it is. */
    private fun decode(s: String): String {
        val out = StringBuilder(s.length)
        var rest = s
        while (true) {
            val i = rest.indexOf('&')
            if (i < 0) break
            out.append(rest, 0, i)
            rest = rest.substring(i)
            // The ";" must come within 12 bytes, as in the desktop app.
            val semi = rest.indexOf(';', 1)
            val j = if (semi >= 0 && rest.substring(0, semi).toByteArray(Charsets.UTF_8).size <= 12) semi else -1
            var cp: Int? = null
            if (j > 0) {
                val name = rest.substring(1, j)
                if (name.startsWith("#")) {
                    val num = name.substring(1)
                    val n = if (num.startsWith("x") || num.startsWith("X")) u32(num.substring(1), 16) else u32(num, 10)
                    if (n != null && n <= 0x10FFFF && n !in 0xD800..0xDFFF) cp = n.toInt()
                } else {
                    cp = NAMED[name]?.code
                }
            }
            if (cp != null) {
                if (cp == 0xA0) out.append(' ') else out.appendCodePoint(cp)
                rest = rest.substring(j + 1)
            } else {
                out.append('&')
                rest = rest.substring(1)
            }
        }
        out.append(rest)
        return out.toString()
    }

    private class Tag(val name: String, val attrs: List<Pair<String, String>>, val len: Int, val selfClosed: Boolean)

    /** Reads a tag after its "<": the name, the attributes and the length up to and with ">". */
    private fun readTag(s: String): Tag? {
        var i = 0
        while (i < s.length && (asciiAlnum(s[i]) || s[i] == '-' || s[i] == ':')) i++
        if (i == 0) return null
        val name = lowerAscii(s.substring(0, i))
        val attrs = ArrayList<Pair<String, String>>()
        var selfClosed = false
        while (true) {
            while (i < s.length && asciiWs(s[i])) i++
            if (i >= s.length) return null
            when (s[i]) {
                '>' -> return Tag(name, attrs, i + 1, selfClosed)
                '/' -> {
                    selfClosed = true
                    i++
                    continue
                }
            }
            val k0 = i
            while (i < s.length && !asciiWs(s[i]) && s[i] != '=' && s[i] != '>' && s[i] != '/') i++
            val key = lowerAscii(s.substring(k0, i))
            while (i < s.length && asciiWs(s[i])) i++
            var v = ""
            if (i < s.length && s[i] == '=') {
                i++
                while (i < s.length && asciiWs(s[i])) i++
                if (i < s.length && (s[i] == '"' || s[i] == '\'')) {
                    val q = s[i]
                    val v0 = i + 1
                    val close = s.indexOf(q, v0)
                    if (close < 0) return null
                    v = decode(s.substring(v0, close))
                    i = close + 1
                } else {
                    val v0 = i
                    while (i < s.length && !asciiWs(s[i]) && s[i] != '>') i++
                    v = decode(s.substring(v0, i))
                }
            }
            if (key.isNotEmpty()) attrs.add(key to v)
            selfClosed = false
        }
    }

    private class Open(val tag: String, val attrs: List<Pair<String, String>>, val kids: ArrayList<Node> = ArrayList())

    private fun parse(html: String): List<Node> {
        // The open elements, each with its children so far. The first entry is the root.
        val stack = arrayListOf(Open("", emptyList()))
        fun close() {
            if (stack.size > 1) {
                val o = stack.removeAt(stack.size - 1)
                stack.last().kids.add(El(o.tag, o.attrs, o.kids))
            }
        }
        var i = 0
        while (i < html.length) {
            val lt = html.indexOf('<', i)
            if (lt < 0) {
                stack.last().kids.add(Text(decode(html.substring(i))))
                break
            }
            if (lt > i) stack.last().kids.add(Text(decode(html.substring(i, lt))))
            val rest = html.substring(lt + 1)
            if (rest.startsWith("!--")) {
                val e = rest.indexOf("-->")
                i = if (e < 0) html.length else lt + 1 + e + 3
            } else if (rest.startsWith("!") || rest.startsWith("?")) {
                val e = rest.indexOf('>')
                i = if (e < 0) html.length else lt + 1 + e + 1
            } else if (rest.startsWith("/")) {
                val t = readTag(rest.substring(1))
                if (t != null) {
                    val at = stack.indexOfLast { it.tag == t.name }
                    if (at > 0) while (stack.size > at) close()
                    i = lt + 2 + t.len
                } else {
                    stack.last().kids.add(Text("<"))
                    i = lt + 1
                }
            } else {
                val t = readTag(rest)
                if (t != null) {
                    i = lt + 1 + t.len
                    val name = t.name
                    // Elements that end the open one of their kind, as a browser reads them.
                    val ends = when (name) {
                        "p" -> setOf("p")
                        "li" -> setOf("li")
                        "dt", "dd" -> setOf("dt", "dd")
                        "tr" -> setOf("tr")
                        "td", "th" -> setOf("td", "th")
                        else -> emptySet()
                    }
                    val fence = when (name) {
                        "li" -> setOf("ul", "ol")
                        "td", "th", "tr" -> setOf("table")
                        else -> emptySet()
                    }
                    val at = stack.indexOfLast { it.tag in ends || it.tag in fence }
                    if (at > 0 && stack[at].tag in ends) while (stack.size > at) close()
                    if (name in BLOCK && name != "li" && stack.last().tag == "p") close()
                    if (name in VOID || t.selfClosed) {
                        stack.last().kids.add(El(name, t.attrs, emptyList()))
                    } else {
                        stack.add(Open(name, t.attrs))
                    }
                } else {
                    stack.last().kids.add(Text("<"))
                    i = lt + 1
                }
            }
        }
        while (stack.size > 1) close()
        return stack.last().kids
    }

    private data class Cx(val bold: Boolean = false, val italic: Boolean = false, val strike: Boolean = false)

    private fun isBlock(tag: String) = tag in BLOCK

    private fun hasBlock(kids: List<Node>): Boolean = kids.any { it is El && (isBlock(it.tag) || hasBlock(it.kids)) }

    /** The style that an element adds: bold, italic or struck through. */
    private fun styled(tag: String, attrs: List<Pair<String, String>>, cx: Cx): Cx {
        var c = cx
        when (tag) {
            "strong", "b" -> c = c.copy(bold = true)
            "em", "i", "cite", "dfn" -> c = c.copy(italic = true)
            "s", "del", "strike" -> c = c.copy(strike = true)
        }
        // Google Docs wraps a whole copy in <b style="font-weight:normal">.
        styleOf(attrs, "font-weight")?.let { boldWeight(it) }?.let { c = c.copy(bold = it) }
        styleOf(attrs, "font-style")?.let { c = c.copy(italic = it.startsWith("italic") || it.startsWith("oblique")) }
        if ((styleOf(attrs, "text-decoration") ?: styleOf(attrs, "text-decoration-line"))?.contains("line-through") == true) c = c.copy(strike = true)
        return c
    }

    /** Wraps [inner] in a mark, with the spaces at its ends outside the mark. */
    private fun wrap(inner: String, mark: String): String {
        val t = trim(inner)
        if (t.isEmpty()) return inner
        val lead = inner.substring(0, inner.length - trimStart(inner).length)
        val trail = inner.substring(trimEnd(inner).length)
        return "$lead$mark$t$mark$trail"
    }

    private fun url(href: String) = trim(href).replace(" ", "%20").replace("(", "%28").replace(")", "%29")

    /** The text of a node and its children as they are, for code. */
    private fun raw(n: Node, out: StringBuilder) {
        when (n) {
            is Text -> out.append(n.text)
            is El -> {
                if (n.tag == "br") {
                    out.append('\n')
                } else if (n.tag !in SKIP) {
                    for (k in n.kids) raw(k, out)
                    if (n.tag in setOf("div", "p", "li", "tr") && !out.endsWith("\n")) out.append('\n')
                }
            }
        }
    }

    private fun inline(nodes: List<Node>, cx: Cx): String = buildString { for (n in nodes) append(inlineOne(n, cx)) }

    private fun inlineOne(n: Node, cx: Cx): String {
        if (n is Text) return buildString(n.text.length) { for (c in n.text) append(if (ws(c)) ' ' else c) }
        val el = n as El
        val tag = el.tag
        val attrs = el.attrs
        val kids = el.kids
        if (tag in SKIP) return ""
        when (tag) {
            "br" -> return "\n"
            "img" -> {
                val src = attr(attrs, "src") ?: ""
                val alt = (attr(attrs, "alt") ?: "").replace('[', ' ').replace(']', ' ').replace('\n', ' ')
                return if (src.startsWith("http://") || src.startsWith("https://")) "![${trim(alt)}](${url(src)})" else alt
            }
            "input" -> return ""
            "code", "kbd", "samp", "tt" -> {
                val sb = StringBuilder()
                for (k in kids) raw(k, sb)
                val t = sb.toString().replace('\n', ' ')
                if (trim(t).isEmpty()) return t
                val fence = if (t.contains('`')) "``" else "`"
                val pad = if (t.startsWith("`") || t.endsWith("`")) " " else ""
                return "$fence$pad$t$pad$fence"
            }
            "a" -> {
                val inner = inline(kids, cx)
                val href = trim(attr(attrs, "href") ?: "")
                val lower = lowerAscii(href)
                if (trim(inner).isEmpty() || href.isEmpty() || href.startsWith("#") || lower.startsWith("javascript:") || lower.startsWith("data:")) return inner
                val lead = inner.substring(0, inner.length - trimStart(inner).length)
                val trail = inner.substring(trimEnd(inner).length)
                return "$lead[${trim(inner).replace('\n', ' ')}](${url(href)})$trail"
            }
        }
        val c = styled(tag, attrs, cx)
        var inner = inline(kids, c)
        // Each mark opens where the style starts, so a nested bold adds no second mark.
        if (c.strike && !cx.strike) inner = wrap(inner, "~~")
        if (c.italic && !cx.italic) inner = wrap(inner, "*")
        if (c.bold && !cx.bold) inner = wrap(inner, "**")
        return if (isBlock(tag)) "\n$inner\n" else inner
    }

    /** Runs of inline content as lines: spaces collapse, and each line is trimmed. */
    private fun tidy(s: String): String {
        val ls = s.split('\n').map { l -> l.split(' ').filter { it.isNotEmpty() }.joinToString(" ") }
        val a = ls.indexOfFirst { it.isNotEmpty() }.let { if (it < 0) ls.size else it }
        val b = ls.indexOfLast { it.isNotEmpty() }.let { if (it < 0) a else it + 1 }
        return ls.subList(a, b).joinToString("\n")
    }

    /** Block content: each block is one entry. Inline runs between blocks become paragraphs. */
    private fun blocks(nodes: List<Node>, cx: Cx, out: MutableList<String>) {
        val run = StringBuilder()
        fun flush() {
            val t = tidy(run.toString())
            if (t.isNotEmpty()) out.add(t)
            run.clear()
        }
        for (n in nodes) {
            if (n !is El) {
                run.append(inlineOne(n, cx))
                continue
            }
            val tag = n.tag
            val attrs = n.attrs
            val kids = n.kids
            if (tag in SKIP) continue
            if (!isBlock(tag)) {
                if (hasBlock(kids)) {
                    // An inline element around blocks, such as the <b> of Google Docs around the copy.
                    flush()
                    blocks(kids, styled(tag, attrs, cx), out)
                } else {
                    run.append(inlineOne(n, cx))
                }
                continue
            }
            flush()
            when (tag) {
                "h1", "h2", "h3", "h4", "h5", "h6" -> {
                    val level = tag[1] - '0'
                    val t = tidy(inline(kids, cx.copy(bold = true))).replace('\n', ' ')
                    if (t.isNotEmpty()) out.add("#".repeat(level) + " " + t)
                }
                "hr" -> out.add("---")
                "pre" -> {
                    val sb = StringBuilder()
                    for (k in kids) raw(k, sb)
                    val classes = listOf(attrs) + kids.filterIsInstance<El>().filter { it.tag == "code" }.map { it.attrs }
                    val lang = classes.mapNotNull { attr(it, "class") }.flatMap { splitWs(it) }.firstNotNullOfOrNull { c ->
                        when {
                            c.startsWith("language-") -> c.removePrefix("language-")
                            c.startsWith("lang-") -> c.removePrefix("lang-")
                            else -> null
                        }
                    } ?: ""
                    val t = sb.toString().trim('\n')
                    val fence = if (t.contains("```")) "~~~~" else "```"
                    out.add("$fence$lang\n$t\n$fence")
                }
                "ul", "ol" -> {
                    val start = attr(attrs, "start")?.let { trim(it).toLongOrNull() } ?: 1L
                    val items = ArrayList<String>()
                    var num = start
                    for (k in kids) {
                        if (k !is El) continue
                        if (k.tag != "li") {
                            // A list right inside a list belongs to the item before it.
                            if (k.tag == "ul" || k.tag == "ol") {
                                val sub = ArrayList<String>()
                                blocks(listOf(k), cx, sub)
                                if (items.isNotEmpty()) {
                                    val sb = StringBuilder(items.last())
                                    for (l in lines(sub.joinToString("\n"))) sb.append("\n  ").append(l)
                                    items[items.size - 1] = sb.toString()
                                }
                            }
                            continue
                        }
                        val marker = if (tag == "ol") "$num. " else "- "
                        num++
                        val task = when (firstCheckbox(k.kids)) {
                            true -> "[x] "
                            false -> "[ ] "
                            null -> ""
                        }
                        val parts = ArrayList<String>()
                        blocks(k.kids, cx, parts)
                        val body = parts.joinToString("\n")
                        val pad = " ".repeat(marker.length)
                        val item = StringBuilder(marker + task)
                        lines(body).forEachIndexed { i, l ->
                            if (i > 0) {
                                item.append('\n')
                                if (l.isNotEmpty()) item.append(pad)
                            }
                            item.append(l)
                        }
                        items.add(trimEnd(item.toString()))
                    }
                    if (items.isNotEmpty()) out.add(items.joinToString("\n"))
                }
                "li" -> {
                    val parts = ArrayList<String>()
                    blocks(kids, cx, parts)
                    out.add("- " + parts.joinToString("\n").replace("\n", "\n  "))
                }
                "blockquote" -> {
                    val parts = ArrayList<String>()
                    blocks(kids, cx, parts)
                    val q = lines(parts.joinToString("\n\n")).map { if (it.isEmpty()) ">" else "> $it" }
                    if (q.isNotEmpty()) out.add(q.joinToString("\n"))
                }
                "table", "thead", "tbody", "tfoot", "tr" -> table(tableRows(n))?.let { out.add(it) }
                "dt" -> {
                    val t = tidy(inline(kids, cx))
                    if (t.isNotEmpty()) out.add(wrap(t, "**"))
                }
                else -> blocks(kids, styled(tag, attrs, cx), out)
            }
        }
        flush()
    }

    /** True or false when a list item starts with a checkbox, ticked or not. Null when it has none. */
    private fun firstCheckbox(kids: List<Node>): Boolean? {
        for (k in kids) {
            when {
                k is Text && trim(k.text).isEmpty() -> continue
                k is El && k.tag == "input" -> {
                    val box = attr(k.attrs, "type")?.equals("checkbox", ignoreCase = true) == true
                    return if (box) attr(k.attrs, "checked") != null else null
                }
                k is El && k.tag in setOf("p", "span", "label", "div") -> return firstCheckbox(k.kids)
                else -> return null
            }
        }
        return null
    }

    private fun tableRows(n: Node): List<List<String>> {
        val rows = ArrayList<List<String>>()
        fun walk(x: Node) {
            if (x !is El) return
            if (x.tag == "tr") {
                val cells = x.kids.filterIsInstance<El>().filter { it.tag == "td" || it.tag == "th" }
                    .map { tidy(inline(it.kids, Cx())).replace('\n', ' ').replace("|", "\\|") }
                if (cells.isNotEmpty()) rows.add(cells)
            } else if (x.tag != "table" || rows.isEmpty()) {
                for (k in x.kids) walk(k)
            }
        }
        walk(n)
        return rows
    }

    private fun table(rows: List<List<String>>): String? {
        val cols = rows.maxOfOrNull { it.size } ?: return null
        fun line(r: List<String>) = "| " + (r + List(cols - r.size) { "" }).joinToString(" | ") + " |"
        val out = arrayListOf(line(rows[0]), "|" + " --- |".repeat(cols))
        rows.drop(1).forEach { out.add(line(it)) }
        return out.joinToString("\n")
    }

    private val MARKS = setOf("h1", "h2", "h3", "h4", "h5", "h6", "ul", "ol", "blockquote", "pre", "code", "table", "hr", "em", "i", "s", "del", "strike")

    /**
     * True when the HTML carries formatting that Markdown can keep. A copy from a code editor
     * only colours spans, so it pastes better as its plain text.
     */
    fun hasFormatting(html: String): Boolean {
        fun walk(nodes: List<Node>): Boolean = nodes.any { n ->
            if (n !is El) return@any false
            val t = n.tag
            val a = n.attrs
            val weight = styleOf(a, "font-weight")?.let { boldWeight(it) }
            val marks = t in MARKS ||
                (t == "a" && attr(a, "href")?.let { trim(it).isNotEmpty() && !it.startsWith("#") } == true) ||
                (t == "img" && attr(a, "src")?.startsWith("http") == true) ||
                ((t == "b" || t == "strong") && weight != false)
            val styled = weight == true || styleOf(a, "font-style")?.startsWith("italic") == true
            // Code editors set "white-space: pre" and colour each token.
            val code = styleOf(a, "white-space")?.startsWith("pre") == true
            !code && (marks || (styled && t != "b" && t != "strong") || walk(n.kids))
        }
        return walk(parse(html))
    }

    /** Pasted HTML as Markdown. */
    fun convert(html: String): String {
        val out = ArrayList<String>()
        blocks(parse(html), Cx(), out)
        return out.joinToString("\n\n")
    }

    /**
     * True when [at] is inside a fenced code block of [text]. The line of the caret counts
     * only when it ends before the caret. Follows in_code_block in desktop/src/images.rs.
     */
    fun inCodeBlock(text: String, at: Int): Boolean {
        val before = text.substring(0, at.coerceIn(0, text.length))
        val fences = before.split('\n').count { l ->
            val t = trimStart(l)
            t.startsWith("```") || t.startsWith("~~~")
        }
        return fences % 2 == 1
    }

    /** One web address and nothing else. */
    fun isWebAddress(s: String): Boolean =
        (s.startsWith("https://") || s.startsWith("http://")) && s.toByteArray(Charsets.UTF_8).size > 8 && s.none { ws(it) }

    /**
     * What a paste puts in place of the selection [a] to [b] of [text], or null for a plain
     * paste. Inside a code block, a paste is plain. HTML with formatting becomes Markdown. A web
     * address pasted over text on one line links that text. Follows paste_content in
     * desktop/src/images.rs.
     */
    fun pasteText(text: String, a0: Int, b0: Int, html: String?, plain: String): String? {
        val a = minOf(a0, b0).coerceIn(0, text.length)
        val b = maxOf(a0, b0).coerceIn(0, text.length)
        if (inCodeBlock(text, a)) return null
        val insert = if (html != null && hasFormatting(html)) {
            convert(html)
        } else {
            val url = trim(plain)
            val sel = text.substring(a, b)
            if (!isWebAddress(url) || a == b || sel.contains('\n') || isWebAddress(trim(sel))) return null
            "[$sel]($url)"
        }
        return if (trim(insert).isEmpty()) null else insert
    }
}
