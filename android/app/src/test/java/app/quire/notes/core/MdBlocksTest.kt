package app.quire.notes.core

import org.junit.Assert.assertEquals
import org.junit.Test

/** Line breaks in the preview blocks. A port of line_breaks_stay in desktop/src/md.rs. */
class MdBlocksTest {
    @Test
    fun lineBreaksStay() {
        val b = Md.parseBlocks("one\ntwo  \nthree\\\nfour\\\n\n- a\n  more a\nlazy a\n- b\n\n> q1\n> q2")
        assertEquals(listOf(BPara::class, BList::class, BQuote::class), b.map { it::class })
        assertEquals("one\ntwo\nthree\nfour\\", (b[0] as BPara).text)
        val items = (b[1] as BList).items
        assertEquals(2, items.size)
        assertEquals("a\nmore a\nlazy a", items[0].text)
        assertEquals(5 to 8, items[0].line to items[1].line)
        assertEquals("b", items[1].text)
        assertEquals("q1\nq2", (b[2] as BQuote).text)
        // A heading, a quote or a new item ends a list item.
        val c = Md.parseBlocks("- a\n# H\n- b\n> q")
        assertEquals(listOf(BList::class, BHead::class, BList::class, BQuote::class), c.map { it::class })
    }

    @Test
    fun anEscapedBackslashStays() {
        assertEquals("a\\\\\nb", Md.joinLines(listOf("a\\\\", "b")))
        assertEquals("a\nb", Md.joinLines(listOf("  a \\  ", " b ")))
    }
}
