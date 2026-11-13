package app.quire.notes.core

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/** The tests of desktop/src/html2md.rs and of the paste rules in desktop/src/images.rs. */
class Html2MdTest {
    @Test
    fun webPage() {
        val html = "<html><head><title>x</title></head><body><h1>Title <small>sub</small></h1>\n" +
            "            <p>Some <b>bold</b>, <em>italic</em> and <del>gone</del> text with a\n" +
            "            <a href=\"https://example.com/a b\">link</a> &amp; <code>code</code>.</p>\n" +
            "            <p>Line one<br>Line two</p><hr><blockquote><p>Quoted</p><p>Twice</p></blockquote></body></html>"
        assertEquals(
            "# Title sub\n\nSome **bold**, *italic* and ~~gone~~ text with a [link](https://example.com/a%20b) & `code`.\n\nLine one\nLine two\n\n---\n\n> Quoted\n>\n> Twice",
            Html2Md.convert(html),
        )
    }

    @Test
    fun lists() {
        val html = "<ul><li>One<ul><li>Inner</li></ul></li><li><p>Two</p><p>more</p></li></ul><ol start=3><li>Three<li>Four</ol>" +
            "<ul><li><input type=checkbox checked> Done</li><li><input type=checkbox> Open</li></ul>"
        assertEquals("- One\n  - Inner\n- Two\n  more\n\n3. Three\n4. Four\n\n- [x] Done\n- [ ] Open", Html2Md.convert(html))
    }

    @Test
    fun codeAndTables() {
        val html = "<pre><code class=\"language-rust\">fn main() {\n    let a = 1 &lt; 2;\n}</code></pre>" +
            "<table><tr><th>A</th><th>B</th></tr><tr><td>1</td><td>x | y</td></tr></table>"
        assertEquals("```rust\nfn main() {\n    let a = 1 < 2;\n}\n```\n\n| A | B |\n| --- | --- |\n| 1 | x \\| y |", Html2Md.convert(html))
    }

    @Test
    fun googleDocs() {
        val html = """<meta charset="utf-8"><b style="font-weight:normal;" id="docs-internal-guid-1"><p dir="ltr"><span style="font-weight:700;">Heavy</span><span style="font-weight:400;"> and </span><span style="font-style:italic;">slanted</span></p><ul><li dir="ltr"><p dir="ltr"><span>Item</span></p></li></ul></b>"""
        assertEquals("**Heavy** and *slanted*\n\n- Item", Html2Md.convert(html))
        assertTrue(Html2Md.hasFormatting(html))
    }

    @Test
    fun plainCopiesStayPlain() {
        // VS Code: coloured spans in a "white-space: pre" div.
        val vscode = """<div style="color: #d4d4d4;font-family: monospace;white-space: pre;"><div><span style="color: #569cd6;">let</span> a = 1;</div></div>"""
        assertFalse(Html2Md.hasFormatting(vscode))
        assertFalse(Html2Md.hasFormatting("<p>Just a sentence.</p><div>Another one.</div>"))
        assertTrue(Html2Md.hasFormatting("<p>A <a href=\"https://a.b\">link</a></p>"))
        assertTrue(Html2Md.hasFormatting("<h2>Heading</h2>"))
    }

    @Test
    fun oddInput() {
        assertEquals("a <b> AB &unknown; &amp", Html2Md.convert("a &lt;b&gt; &#65;&#x42; &unknown; &amp"))
        assertEquals("unclosed **bold**", Html2Md.convert("<p>unclosed <b>bold"))
        assertEquals("**spaced** word", Html2Md.convert("<b> spaced </b>word"))
        assertEquals("x pic", Html2Md.convert("<a href=\"javascript:alert(1)\">x</a> <img src=\"data:image/png;base64,AA\" alt=\"pic\">"))
        assertEquals("x < y and <3", Html2Md.convert("x < y and <3"))
        assertEquals("", Html2Md.convert(""))
    }

    @Test
    fun codeBlocksAndAddresses() {
        val t = "a\n```\ncode\n```\nb"
        assertFalse(Html2Md.inCodeBlock(t, 1))
        assertTrue(Html2Md.inCodeBlock(t, t.indexOf("code")))
        assertFalse(Html2Md.inCodeBlock(t, t.length))
        assertTrue(Html2Md.isWebAddress("https://example.com/a?b=c"))
        assertFalse(Html2Md.isWebAddress("https://example.com and more"))
        assertFalse(Html2Md.isWebAddress("example.com"))
    }

    @Test
    fun pasteRules() {
        val html = "<p>Some <b>bold</b> text</p>"
        // HTML with formatting becomes Markdown in place of the selection.
        assertEquals("Some **bold** text", Html2Md.pasteText("a b", 1, 1, html, "Some bold text"))
        // Inside a code block, a paste stays plain.
        val code = "```\nx\n"
        assertNull(Html2Md.pasteText(code, code.length, code.length, html, "Some bold text"))
        // HTML without formatting stays plain.
        assertNull(Html2Md.pasteText("a b", 1, 1, "<p>Just text</p>", "Just text"))
        // A web address over text on one line links that text.
        assertEquals("[Quire](https://example.com)", Html2Md.pasteText("see Quire now", 4, 9, null, " https://example.com "))
        // No selection, a selection over two lines, or a selected address: a plain paste.
        assertNull(Html2Md.pasteText("see Quire now", 4, 4, null, "https://example.com"))
        assertNull(Html2Md.pasteText("see\nQuire", 0, 9, null, "https://example.com"))
        assertNull(Html2Md.pasteText("https://a.example", 0, 17, null, "https://example.com"))
        assertNull(Html2Md.pasteText("see Quire now", 4, 9, null, "just text"))
    }
}
