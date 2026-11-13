package app.quire.notes.ui

import androidx.compose.foundation.gestures.awaitEachGesture
import androidx.compose.foundation.gestures.awaitFirstDown
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.input.pointer.PointerEventPass
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import app.quire.notes.core.Md
import app.quire.notes.state.QuireViewModel

/**
 * The actions of one note. A long-press on a note in the drawer, on a tab or on the preview
 * opens it, and so does "Note actions…" in the text selection menu. An item that does not apply
 * to the note stays out. [onTab] adds the tab items. [at] puts the menu at a point.
 */
@Composable
internal fun NoteMenu(vm: QuireViewModel, path: String, onTab: Boolean, at: IntOffset? = null, onDismiss: () -> Unit) {
    val c = LocalQ.current
    var picking by remember(path) { mutableStateOf(false) }
    var asking by remember(path) { mutableStateOf(false) }
    val close = {
        picking = false
        asking = false
        onDismiss()
    }
    RowMenu(close, at) {
        Mono(path, 11.sp, c.mute, Modifier.padding(start = 10.dp, end = 10.dp, top = 7.dp, bottom = 8.dp), lineHeight = 16.sp)
        when {
            picking -> {
                val here = Md.dir(path)
                Column(Modifier.heightIn(max = 320.dp).verticalScroll(rememberScrollState())) {
                    (listOf("" to "Folder root") + vm.folderList().map { it to "$it/" }).forEach { (dir, label) ->
                        val current = dir == here
                        Box(
                            Modifier
                                .fillMaxWidth()
                                .height(36.dp)
                                .tap(RoundedCornerShape(6.dp), enabled = !current) {
                                    close()
                                    vm.moveNote(path, dir)
                                }
                                .padding(horizontal = 10.dp),
                            contentAlignment = Alignment.CenterStart,
                        ) { Mono(if (current) "$label  (here)" else label, 12.sp, if (current) c.mute else c.ink, maxLines = 1) }
                    }
                }
            }
            asking -> {
                // A delete asks first, as the folder delete does.
                Txt(
                    "Move ${Md.base(path)} to the trash?",
                    13.sp, c.ink2, Modifier.padding(start = 10.dp, end = 10.dp, top = 2.dp, bottom = 6.dp), lineHeight = 18.sp,
                )
                Row {
                    MenuItem("Cancel", Modifier.weight(1f)) { close() }
                    MenuItem("Delete", Modifier.weight(1f), color = c.err) {
                        close()
                        vm.deleteNote(path)
                    }
                }
            }
            else -> {
                MenuItem(if (vm.isPinned(path)) "Unpin" else "Pin") {
                    close()
                    vm.togglePin(path)
                }
                MenuItem("Rename") {
                    close()
                    vm.renameNote(path)
                }
                MenuItem("Move to…") { picking = true }
                // A locked note shares only while it is unlocked.
                if (!vm.noteLocked(path)) {
                    MenuItem("Share") {
                        close()
                        vm.shareNote(path)
                    }
                }
                if (path in vm.unlocked) {
                    MenuItem("Lock now") {
                        close()
                        vm.lockNow(path)
                    }
                    MenuItem("Remove password") {
                        close()
                        vm.removePasswordOf(path)
                    }
                } else if (!vm.noteLocked(path)) {
                    MenuItem("Lock with a password…") {
                        close()
                        vm.lockNoteAsk(path)
                    }
                }
                if (onTab) {
                    MenuItem("Close tab") {
                        close()
                        vm.closeTab(path)
                    }
                    if (vm.tabs.size > 1) {
                        MenuItem("Close other tabs") {
                            close()
                            vm.closeOtherTabs(path)
                        }
                    }
                }
                MenuItem("Delete note", color = c.err) { asking = true }
            }
        }
    }
}

/**
 * Calls [onLongPress] with the press point after a long-press that does not move. The detector
 * only watches the events, so taps on links, tags, task boxes and images still work, and a
 * scroll or a fling never opens the menu. After a long-press, it takes the rest of the gesture,
 * so the release taps nothing.
 */
internal fun Modifier.longPressAt(onLongPress: (Offset) -> Unit): Modifier = pointerInput(Unit) {
    awaitEachGesture {
        val down = awaitFirstDown(requireUnconsumed = false, pass = PointerEventPass.Initial)
        val slop = viewConfiguration.touchSlop
        // A release, a second finger or a move ends the wait. Null means the press held on.
        var ended = false
        val waited = withTimeoutOrNull(viewConfiguration.longPressTimeoutMillis) {
            while (!ended) {
                val ev = awaitPointerEvent(PointerEventPass.Initial)
                val ch = ev.changes.firstOrNull { it.id == down.id }
                if (ch == null || !ch.pressed || ev.changes.size > 1 || (ch.position - down.position).getDistance() > slop) ended = true
            }
        }
        if (waited == null && !ended) {
            onLongPress(down.position)
            while (true) {
                val ev = awaitPointerEvent(PointerEventPass.Initial)
                ev.changes.forEach { it.consume() }
                if (ev.changes.none { it.pressed }) break
            }
        }
    }
}

