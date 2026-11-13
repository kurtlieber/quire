package app.quire.notes.ui

import android.content.ClipData
import androidx.compose.ui.platform.ClipEntry
import androidx.compose.ui.platform.Clipboard
import app.quire.notes.state.QuireViewModel

/**
 * The clipboard of the editor. The context-menu Paste and Ctrl+V both read it. When the copy
 * carries formatted HTML, or is a web address over selected text, the paste reads Markdown in
 * place of the plain text. The text field pastes that as usual, so the paste stays one undo
 * step. Copies go to the system clipboard as they are.
 */
class PasteClipboard(private val real: Clipboard, private val vm: QuireViewModel) : Clipboard {
    override suspend fun getClipEntry(): ClipEntry? {
        val entry = real.getClipEntry() ?: return null
        val md = vm.pasteMarkdown(entry.clipData) ?: return entry
        return ClipEntry(ClipData.newPlainText("Markdown", md))
    }

    override suspend fun setClipEntry(clipEntry: ClipEntry?) = real.setClipEntry(clipEntry)

    override val nativeClipboard: android.content.ClipboardManager get() = real.nativeClipboard
}
