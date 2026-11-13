package app.quire.notes.core

/**
 * Pinned notes. A pinned note shows in the Pinned group at the top of the tree, not in its
 * folder. The file does not move. The pins are kept for each notes folder with the session,
 * in the order they were pinned, and they do not sync. Follows desktop/src/pins.rs.
 */
object Pins {
    /** The pins of notes that exist. Pins of files that are gone drop out. */
    fun filter(pinned: List<String>, files: Collection<String>): List<String> {
        val have = files.toHashSet()
        return pinned.filter { it in have }.distinct()
    }

    /** The pinned notes that exist, sorted by name as the tree sorts notes. */
    fun sorted(pinned: List<String>, files: Collection<String>): List<String> =
        filter(pinned, files).sortedWith { a, b ->
            val c = Md.collator.compare(Md.base(a), Md.base(b))
            if (c != 0) c else a.compareTo(b)
        }

    /** Pins [path] when it has no pin, or else takes the pin away. A new pin goes at the end. */
    fun toggle(pinned: List<String>, path: String): List<String> =
        if (path in pinned) pinned - path else pinned + path

    /** A rename or a move from [old] to [new] keeps the pin, at the same place in the order. */
    fun rename(pinned: List<String>, old: String, new: String): List<String> =
        pinned.map { if (it == old) new else it }

    /** A delete of [path] takes its pin away. */
    fun remove(pinned: List<String>, path: String): List<String> = pinned - path

    /** A delete of the folder [dir] takes away the pins of the notes in it and in its subfolders. */
    fun removeIn(pinned: List<String>, dir: String): List<String> = pinned.filter { !Md.inDir(it, dir) }

    /** The last part of the folder of [path], for the right side of a row in the group. Empty at the root. */
    fun folderLabel(path: String): String = Md.dir(path).substringAfterLast('/')
}
