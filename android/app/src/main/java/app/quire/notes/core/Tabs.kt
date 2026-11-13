package app.quire.notes.core

/** The tab rules: what stays open and which note is active after tabs close. */
object Tabs {
    /**
     * The tabs and the active note after the tabs in [closed] close. When the active note
     * closes, the tab at its place becomes active, or the tab before it at the end of the row.
     */
    fun close(tabs: List<String>, active: String?, closed: Set<String>): Pair<List<String>, String?> {
        val left = tabs.filter { it !in closed }
        if (active == null || active !in closed) return left to active
        val i = tabs.indexOf(active)
        // The tabs before the active one that stay open.
        val before = tabs.take(maxOf(i, 0)).count { it !in closed }
        val next = if (left.isEmpty()) null else left[minOf(before, left.size - 1)]
        return left to next
    }

    /** Close other tabs: only [keep] stays open, and it is active. */
    fun closeOthers(tabs: List<String>, keep: String): Pair<List<String>, String?> =
        if (keep in tabs) listOf(keep) to keep else tabs to null
}
