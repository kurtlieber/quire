package app.quire.notes.core

import org.junit.Assert.assertEquals
import org.junit.Test

/** The pin bookkeeping: toggle, rename, move, delete, folder delete and the filter on load. */
class PinsTest {
    private val files = listOf("Ideas.md", "Drafts/On slow software.md", "Journal/2026-11-12.md", "Projects/Garden plan.md", "apple.md")

    @Test
    fun togglesKeepThePinOrder() {
        var p = Pins.toggle(emptyList(), "Ideas.md")
        p = Pins.toggle(p, "Projects/Garden plan.md")
        assertEquals(listOf("Ideas.md", "Projects/Garden plan.md"), p)
        assertEquals(listOf("Projects/Garden plan.md"), Pins.toggle(p, "Ideas.md"))
    }

    @Test
    fun theGroupSortsByNameWithoutCase() {
        val p = listOf("Projects/Garden plan.md", "Ideas.md", "apple.md", "Journal/2026-11-12.md")
        assertEquals(listOf("Journal/2026-11-12.md", "apple.md", "Projects/Garden plan.md", "Ideas.md"), Pins.sorted(p, files))
        assertEquals("Projects", Pins.folderLabel("Projects/Garden plan.md"))
        assertEquals("Old", Pins.folderLabel("Drafts/Old/Plan.md"))
        assertEquals("", Pins.folderLabel("Ideas.md"))
    }

    @Test
    fun renameAndMoveKeepThePin() {
        val p = listOf("Ideas.md", "Drafts/On slow software.md")
        assertEquals(listOf("Ideas.md", "Drafts/On fast software.md"), Pins.rename(p, "Drafts/On slow software.md", "Drafts/On fast software.md"))
        // A move is a rename into another folder.
        assertEquals(listOf("Archive/Ideas.md", "Drafts/On slow software.md"), Pins.rename(p, "Ideas.md", "Archive/Ideas.md"))
        assertEquals(p, Pins.rename(p, "Other.md", "New.md"))
    }

    @Test
    fun deletesTakeThePinAway() {
        val p = listOf("Ideas.md", "Drafts/Old/A.md", "Drafts/Older.md", "Drafts/B.md")
        assertEquals(listOf("Drafts/Old/A.md", "Drafts/Older.md", "Drafts/B.md"), Pins.remove(p, "Ideas.md"))
        assertEquals(listOf("Ideas.md", "Drafts/Older.md", "Drafts/B.md"), Pins.removeIn(p, "Drafts/Old"))
        assertEquals(listOf("Ideas.md"), Pins.removeIn(p, "Drafts"))
    }

    @Test
    fun pinsOfMissingFilesDropOnLoad() {
        val saved = listOf("Ideas.md", "Gone.md", "Projects/Garden plan.md", "Ideas.md")
        assertEquals(listOf("Ideas.md", "Projects/Garden plan.md"), Pins.filter(saved, files))
        assertEquals(emptyList<String>(), Pins.sorted(listOf("Gone.md"), files))
    }
}
