package app.quire.notes.core

import org.junit.Assert.assertEquals
import org.junit.Test

/** The tab rules of the note menu: close a tab, close other tabs, and delete a note. */
class TabsTest {
    private val tabs = listOf("A.md", "B.md", "C.md", "D.md")

    @Test
    fun closingTheActiveTabActivatesTheTabAtItsPlace() {
        assertEquals(listOf("A.md", "C.md", "D.md") to "C.md", Tabs.close(tabs, "B.md", setOf("B.md")))
        // At the end of the row, the tab before it becomes active.
        assertEquals(listOf("A.md", "B.md", "C.md") to "C.md", Tabs.close(tabs, "D.md", setOf("D.md")))
        assertEquals(emptyList<String>() to null, Tabs.close(listOf("A.md"), "A.md", setOf("A.md")))
    }

    @Test
    fun closingAnotherTabKeepsTheActiveNote() {
        assertEquals(listOf("A.md", "B.md", "D.md") to "B.md", Tabs.close(tabs, "B.md", setOf("C.md")))
        // A note with no tab, for example one deleted from the drawer, changes no tab.
        assertEquals(tabs to "B.md", Tabs.close(tabs, "B.md", setOf("Other.md")))
    }

    @Test
    fun closeOtherTabsKeepsOneTabAndMakesItActive() {
        assertEquals(listOf("C.md") to "C.md", Tabs.closeOthers(tabs, "C.md"))
        assertEquals(listOf("A.md") to "A.md", Tabs.closeOthers(listOf("A.md"), "A.md"))
        // A note without a tab changes nothing.
        assertEquals(tabs to null, Tabs.closeOthers(tabs, "Other.md"))
    }
}
