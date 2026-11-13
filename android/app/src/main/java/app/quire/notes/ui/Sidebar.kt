package app.quire.notes.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.gestures.awaitEachGesture
import androidx.compose.foundation.gestures.awaitFirstDown
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicText
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateMapOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.rotate
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Rect
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.hapticfeedback.HapticFeedbackType
import androidx.compose.ui.input.pointer.PointerEventPass
import androidx.compose.ui.input.pointer.changedToUp
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.input.pointer.positionChanged
import androidx.compose.ui.layout.LayoutCoordinates
import androidx.compose.ui.layout.boundsInRoot
import androidx.compose.ui.layout.onGloballyPositioned
import androidx.compose.ui.layout.positionInRoot
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.LocalFocusManager
import androidx.compose.ui.platform.LocalHapticFeedback
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.text.withStyle
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.window.Popup
import androidx.compose.ui.window.PopupProperties
import app.quire.notes.core.Md
import app.quire.notes.core.Pins
import app.quire.notes.state.Panel
import app.quire.notes.state.QuireViewModel

private const val ROW_NOTE = 0
private const val ROW_FOLDER = 1
private const val ROW_INPUT = 2
private const val ROW_PINS = 3
private const val ROW_PINNED = 4

private class TreeRow(
    val name: String,
    val path: String,
    val depth: Int,
    val kind: Int,
    val open: Boolean,
) {
    val folder get() = kind == ROW_FOLDER
}

/**
 * The file tree, like treeRows() in the prototype. [inputIn] adds the new-folder field to that
 * folder. The [pinned] notes show in the Pinned group at the top, and not in their folders.
 */
private fun treeRows(
    folders: Collection<String>,
    paths: Collection<String>,
    expanded: Set<String>,
    inputIn: String?,
    pinned: List<String> = emptyList(),
    pinsOpen: Boolean = true,
): List<TreeRow> {
    class Node(val path: String) {
        val folders = HashMap<String, Node>()
        val files = ArrayList<String>()
    }
    val root = Node("")
    fun add(path: String): Node {
        var n = root
        for (part in path.split('/')) {
            n = n.folders.getOrPut(part) { Node(if (n.path.isNotEmpty()) n.path + "/" + part else part) }
        }
        return n
    }
    folders.forEach { add(it) }
    val pins = pinned.toHashSet()
    paths.forEach { p -> if (p !in pins) { val d = Md.dir(p); (if (d.isNotEmpty()) add(d) else root).files.add(p) } }
    val rows = ArrayList<TreeRow>()
    if (pinned.isNotEmpty()) {
        rows.add(TreeRow("Pinned", "pins", 0, ROW_PINS, pinsOpen))
        if (pinsOpen) pinned.forEach { p -> rows.add(TreeRow(Md.base(p), p, 1, ROW_PINNED, false)) }
    }
    fun walk(n: Node, depth: Int) {
        if (inputIn == n.path) rows.add(TreeRow("", n.path, depth, ROW_INPUT, false))
        n.folders.keys.sortedWith { a, b -> Md.collator.compare(a, b) }.forEach { name ->
            val f = n.folders.getValue(name)
            val open = f.path in expanded
            rows.add(TreeRow(name, f.path, depth, ROW_FOLDER, open))
            if (open) walk(f, depth + 1)
        }
        n.files.sortedWith { a, b -> Md.collator.compare(Md.base(a), Md.base(b)) }.forEach { p ->
            rows.add(TreeRow(Md.base(p), p, depth, ROW_NOTE, false))
        }
    }
    walk(root, 0)
    return rows
}

/** A note that the person drags onto a folder. Positions are in root coordinates. */
private class DragState {
    var path by mutableStateOf<String?>(null)
    var pos by mutableStateOf(Offset.Zero)
    var moved by mutableStateOf(false)

    /** Where the new-folder field is. A touch anywhere else in the drawer cancels the field. */
    var fieldBounds by mutableStateOf<Rect?>(null)

    /** Drop targets: folder path to its bounds. "" is the vault name at the top, which stands for the root. */
    val targets = mutableStateMapOf<String, Rect>()

    /** The folder under the finger, or null. The note's own folder does not count. */
    val target: String?
        get() {
            val p = path ?: return null
            if (!moved) return null
            return targets.entries.firstOrNull { it.value.contains(pos) }?.key?.takeIf { it != Md.dir(p) }
        }

    fun clear() {
        path = null
        moved = false
    }
}

@Composable
fun Sidebar(vm: QuireViewModel) {
    val c = LocalQ.current
    val v = vm.vault ?: return
    val drag = remember { DragState() }
    var origin by remember { mutableStateOf(Offset.Zero) }
    Box(
        Modifier
            .fillMaxSize()
            .onGloballyPositioned { origin = it.positionInRoot() }
            .pointerInput(vm) {
                // A touch outside the new-folder field takes the focus away from it, so the field cancels.
                awaitEachGesture {
                    val down = awaitFirstDown(requireUnconsumed = false, pass = PointerEventPass.Initial)
                    val at = down.position + origin
                    val field = drag.fieldBounds
                    if (vm.newFolderParent != null && (field == null || !field.contains(at))) vm.cancelNewFolder()
                }
            }
    ) {
    Column(Modifier.fillMaxSize()) {
        // The vault name opens the small vault menu. It is also the drop target for the root.
        Box(Modifier.padding(start = 8.dp, end = 8.dp, top = 8.dp, bottom = 6.dp)) {
            Row(
                Modifier
                    .fillMaxWidth()
                    .height(36.dp)
                    .onGloballyPositioned { drag.targets[""] = it.boundsInRoot() }
                    .background(if (drag.target == "") c.hover else Color.Transparent, RoundedCornerShape(7.dp))
                    .tap(RoundedCornerShape(7.dp)) { vm.vaultMenu = !vm.vaultMenu }
                    .padding(horizontal = 10.dp),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(6.dp),
            ) {
                Txt(v.name, 14.sp, c.ink, Modifier.weight(1f), weight = FontWeight.W600, maxLines = 1)
                QIcon(QIcons.CHEVRON_DOWN, 10f, 10.dp, 1.3f, c.mute)
            }
            if (vm.vaultMenu) VaultMenu(vm)
        }
        Row(Modifier.padding(start = 12.dp, end = 12.dp, bottom = 10.dp), horizontalArrangement = Arrangement.spacedBy(2.dp)) {
            listOf(Panel.Files to "files", Panel.Search to "search", Panel.Tags to "tags").forEach { (p, label) ->
                val on = vm.leftPanel == p
                Box(
                    Modifier
                        .height(28.dp)
                        .background(if (on) c.act else Color.Transparent, RoundedCornerShape(6.dp))
                        .tap(RoundedCornerShape(6.dp)) {
                            if (p == Panel.Search) vm.searchFocusPending = true
                            vm.leftPanel = p
                        }
                        .padding(horizontal = 10.dp),
                    contentAlignment = Alignment.Center,
                ) { Mono(label, 11.5.sp, if (on) c.ink else c.mute) }
            }
        }
        Box(Modifier.weight(1f).fillMaxWidth()) {
            when (vm.leftPanel) {
                Panel.Files -> FilesPanel(vm, drag)
                Panel.Search -> SearchPanel(vm)
                Panel.Tags -> TagsPanel(vm)
            }
        }
        Column(Modifier.padding(start = 8.dp, end = 8.dp, top = 6.dp, bottom = 10.dp), verticalArrangement = Arrangement.spacedBy(1.dp)) {
            SideAction("Quick switcher") { vm.openPalette("files") }
            SideAction("All commands") { vm.openPalette("commands") }
            SideAction("New note") {
                vm.drawerOpen = false
                vm.newNote()
            }
            SideAction("New folder") { vm.startNewFolder("") }
            SideAction("Settings") { vm.openSettings() }
        }
    }
        // The note under the finger follows it while it moves.
        val dp = drag.path
        if (dp != null && drag.moved) {
            val lift = with(LocalDensity.current) { 44.dp.roundToPx() }
            Box(
                Modifier
                    .offset { IntOffset((drag.pos.x - origin.x).toInt() + 12, (drag.pos.y - origin.y).toInt() - lift) }
                    .shadow(10.dp, RoundedCornerShape(7.dp), ambientColor = Shadow, spotColor = Shadow)
                    .background(c.bg, RoundedCornerShape(7.dp))
                    .border(1.dp, c.line, RoundedCornerShape(7.dp))
                    .padding(horizontal = 10.dp, vertical = 6.dp)
            ) {
                Txt(Md.base(dp), 13.sp, c.ink, weight = FontWeight.W500, maxLines = 1)
            }
        }
    }
}

@Composable
private fun SideAction(label: String, onClick: () -> Unit) {
    val c = LocalQ.current
    Row(
        Modifier
            .fillMaxWidth()
            .height(36.dp)
            .tap(RoundedCornerShape(6.dp), onClick = onClick)
            .padding(horizontal = 10.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Txt(label, 13.sp, c.ink2, Modifier.weight(1f))
    }
}

@Composable
private fun VaultMenu(vm: QuireViewModel) {
    val c = LocalQ.current
    val v = vm.vault ?: return
    Popup(
        offset = IntOffset(0, with(LocalDensity.current) { 40.dp.roundToPx() }),
        onDismissRequest = { vm.vaultMenu = false },
        properties = PopupProperties(focusable = true),
    ) {
        Column(
            Modifier
                .padding(start = 2.dp)
                .width(240.dp)
                .shadow(14.dp, RoundedCornerShape(9.dp), ambientColor = Shadow, spotColor = Shadow)
                .background(c.bg, RoundedCornerShape(9.dp))
                .border(1.dp, c.line, RoundedCornerShape(9.dp))
                .padding(5.dp)
        ) {
            Mono(v.path, 11.sp, c.mute, Modifier.padding(start = 10.dp, end = 10.dp, top = 7.dp, bottom = 8.dp), lineHeight = 16.sp)
            MenuItem("Switch folder…") { vm.goWelcome() }
            MenuItem("Settings") { vm.openSettings() }
        }
    }
}

@Composable
private fun MenuItem(label: String, modifier: Modifier = Modifier, color: Color = LocalQ.current.ink, onClick: () -> Unit) {
    Box(
        modifier
            .fillMaxWidth()
            .height(36.dp)
            .tap(RoundedCornerShape(6.dp), onClick = onClick)
            .padding(horizontal = 10.dp),
        contentAlignment = Alignment.CenterStart,
    ) { Txt(label, 13.sp, color) }
}

@Composable
private fun FilesPanel(vm: QuireViewModel, drag: DragState) {
    val c = LocalQ.current
    val inputIn = vm.newFolderParent
    val pinned = remember(vm.pinned, vm.files.keys) { Pins.sorted(vm.pinned, vm.files.keys) }
    val rows = remember(vm.folders, vm.files.keys, vm.expanded, inputIn, pinned, vm.pinsOpen) {
        treeRows(vm.folders, vm.files.keys, vm.expanded, inputIn, pinned, vm.pinsOpen)
    }
    var noteMenu by remember { mutableStateOf<String?>(null) }
    var folderMenu by remember { mutableStateOf<String?>(null) }
    if (rows.isEmpty()) {
        Txt("No notes yet.", 13.sp, c.mute, Modifier.padding(horizontal = 22.dp, vertical = 6.dp))
        return
    }
    LazyColumn(Modifier.fillMaxSize()) {
        items(rows, key = { r -> listOf("f:", "d:", "n:", "g:", "p:")[r.kind] + r.path }) { r ->
            when (r.kind) {
                ROW_INPUT -> NewFolderRow(vm, r.depth, drag)
                ROW_PINS -> PinsRow(vm, r, pinned.size)
                ROW_PINNED -> NoteRow(vm, r, drag, noteMenu == "pin:" + r.path, { noteMenu = "pin:" + r.path }, { noteMenu = null }, Pins.folderLabel(r.path))
                ROW_FOLDER -> FolderRow(vm, r, drag, folderMenu == r.path, { folderMenu = r.path }, { folderMenu = null })
                else -> NoteRow(vm, r, drag, noteMenu == r.path, { noteMenu = r.path }, { noteMenu = null })
            }
        }
        item { Spacer(Modifier.height(12.dp)) }
    }
}

@Composable
private fun FolderRow(vm: QuireViewModel, r: TreeRow, drag: DragState, menu: Boolean, openMenu: () -> Unit, closeMenu: () -> Unit) {
    val c = LocalQ.current
    DisposableEffect(r.path) { onDispose { drag.targets.remove(r.path) } }
    Box {
        Row(
            Modifier
                .padding(horizontal = 8.dp)
                .fillMaxWidth()
                .height(32.dp)
                .onGloballyPositioned { drag.targets[r.path] = it.boundsInRoot() }
                .background(if (drag.target == r.path) c.hover else Color.Transparent, RoundedCornerShape(6.dp))
                .tap(RoundedCornerShape(6.dp), onLongClick = openMenu) { vm.toggleFolder(r.path) }
                .padding(start = (8 + r.depth * 20).dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(6.dp),
        ) {
            Box(Modifier.width(12.dp), contentAlignment = Alignment.Center) {
                QIcon(QIcons.CHEVRON_RIGHT, 8f, 8.dp, 1.4f, c.mute, Modifier.rotate(if (r.open) 90f else 0f))
            }
            QIcon(if (r.open) QIcons.FOLDER_OPEN else QIcons.FOLDER_CLOSED, 16f, 15.dp, 1.25f, c.mute)
            Txt(r.name, 13.5.sp, c.ink2, Modifier.weight(1f), weight = FontWeight.W500, maxLines = 1)
            // A new note in this folder, like "New note" but with the folder already picked.
            Box(
                Modifier.size(32.dp).tap(RoundedCornerShape(6.dp)) { vm.newNote(inFolder = r.path) },
                contentAlignment = Alignment.Center,
            ) { QIcon(QIcons.PLUS, 12f, 12.dp, 1.3f, c.mute) }
        }
        if (menu) {
            var asking by remember { mutableStateOf(false) }
            val close = {
                asking = false
                closeMenu()
            }
            RowMenu(close) {
                if (!asking) {
                    MenuItem("New note here") {
                        close()
                        vm.newNote(inFolder = r.path)
                    }
                    MenuItem("New folder here") {
                        close()
                        vm.startNewFolder(r.path)
                    }
                    MenuItem("Delete folder") {
                        // A folder without notes goes at once. A folder with notes asks first.
                        if (vm.notesIn(r.path) == 0) {
                            close()
                            vm.deleteFolder(r.path)
                        } else {
                            asking = true
                        }
                    }
                } else {
                    val n = vm.notesIn(r.path)
                    Txt(
                        "Move ${r.name} and its $n ${if (n == 1) "note" else "notes"} to the trash?",
                        13.sp, c.ink2, Modifier.padding(start = 10.dp, end = 10.dp, top = 8.dp, bottom = 6.dp), lineHeight = 18.sp,
                    )
                    Row {
                        MenuItem("Cancel", Modifier.weight(1f)) { close() }
                        MenuItem("Delete", Modifier.weight(1f), color = c.err) {
                            close()
                            vm.deleteFolder(r.path)
                        }
                    }
                }
            }
        }
    }
}

@Composable
private fun NoteRow(
    vm: QuireViewModel,
    r: TreeRow,
    drag: DragState,
    menu: Boolean,
    openMenu: () -> Unit,
    closeMenu: () -> Unit,
    folderLabel: String = "",
) {
    val c = LocalQ.current
    val on = r.path == vm.active
    val haptics = LocalHapticFeedback.current
    var coords by remember { mutableStateOf<LayoutCoordinates?>(null) }
    var picking by remember { mutableStateOf(false) }
    val lifted = drag.path == r.path && drag.moved
    Box {
        Row(
            Modifier
                .padding(horizontal = 8.dp)
                .fillMaxWidth()
                .height(32.dp)
                .onGloballyPositioned { coords = it }
                .alpha(if (lifted) 0.45f else 1f)
                .background(if (on) c.act else Color.Transparent, RoundedCornerShape(6.dp))
                .pointerInput(r.path) {
                    // A tap opens the note. A long-press lifts it: drag it onto a folder, or let go for the menu.
                    awaitEachGesture {
                        val down = awaitFirstDown()
                        val slop = viewConfiguration.touchSlop
                        val early: Boolean? = withTimeoutOrNull(viewConfiguration.longPressTimeoutMillis) {
                            var result: Boolean? = null
                            while (result == null) {
                                val ev = awaitPointerEvent()
                                val ch = ev.changes.firstOrNull { it.id == down.id }
                                result = when {
                                    ch == null || ch.isConsumed -> false
                                    ch.changedToUp() -> true
                                    (ch.position - down.position).getDistance() > slop -> false
                                    else -> null
                                }
                            }
                            result
                        }
                        if (early == true) {
                            vm.openFile(r.path)
                            return@awaitEachGesture
                        }
                        if (early == false) return@awaitEachGesture
                        haptics.performHapticFeedback(HapticFeedbackType.LongPress)
                        val at = coords
                        drag.path = r.path
                        drag.moved = false
                        drag.pos = at?.localToRoot(down.position) ?: Offset.Zero
                        while (true) {
                            val ev = awaitPointerEvent()
                            val ch = ev.changes.firstOrNull { it.id == down.id } ?: break
                            if (ch.changedToUp()) {
                                ch.consume()
                                break
                            }
                            if (ch.positionChanged()) {
                                ch.consume()
                                if ((ch.position - down.position).getDistance() > slop) drag.moved = true
                                drag.pos = at?.localToRoot(ch.position) ?: drag.pos
                            }
                        }
                        val target = drag.target
                        val moved = drag.moved
                        drag.clear()
                        when {
                            !moved -> openMenu()
                            target != null -> vm.moveNote(r.path, target)
                        }
                    }
                }
                .padding(start = (8 + r.depth * 20).dp, end = 8.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(6.dp),
        ) {
            Spacer(Modifier.width(12.dp))
            Txt(r.name, 13.5.sp, if (on) c.ink else c.ink2, Modifier.weight(1f), weight = if (on) FontWeight.W500 else FontWeight.W400, maxLines = 1)
            // A pinned note from a folder shows the name of the folder at the right.
            if (folderLabel.isNotEmpty()) Mono(folderLabel, 11.sp, c.mute, Modifier.widthIn(max = 120.dp), maxLines = 1)
        }
        if (menu) {
            val close = {
                picking = false
                closeMenu()
            }
            RowMenu(close) {
                Mono(r.path, 11.sp, c.mute, Modifier.padding(start = 10.dp, end = 10.dp, top = 7.dp, bottom = 8.dp), lineHeight = 16.sp)
                if (!picking) {
                    MenuItem(if (vm.isPinned(r.path)) "Unpin" else "Pin") {
                        close()
                        vm.togglePin(r.path)
                    }
                    MenuItem("Move to…") { picking = true }
                } else {
                    val here = Md.dir(r.path)
                    Column(Modifier.heightIn(max = 320.dp).verticalScroll(rememberScrollState())) {
                        (listOf("" to "Folder root") + vm.folderList().map { it to "$it/" }).forEach { (dir, label) ->
                            val current = dir == here
                            Box(
                                Modifier
                                    .fillMaxWidth()
                                    .height(36.dp)
                                    .tap(RoundedCornerShape(6.dp), enabled = !current) {
                                        close()
                                        vm.moveNote(r.path, dir)
                                    }
                                    .padding(horizontal = 10.dp),
                                contentAlignment = Alignment.CenterStart,
                            ) { Mono(if (current) "$label  (here)" else label, 12.sp, if (current) c.mute else c.ink, maxLines = 1) }
                        }
                    }
                }
            }
        }
    }
}

/** The name field for a new folder. Enter makes the folder. Escape or leaving the field cancels. */
@Composable
private fun NewFolderRow(vm: QuireViewModel, depth: Int, drag: DragState) {
    val c = LocalQ.current
    DisposableEffect(Unit) { onDispose { drag.fieldBounds = null } }
    val fr = remember { FocusRequester() }
    var hadFocus by remember { mutableStateOf(false) }
    LaunchedEffect(Unit) {
        try {
            fr.requestFocus()
        } catch (e: IllegalStateException) {
            // Not attached yet.
        }
    }
    Row(
        Modifier
            .padding(horizontal = 8.dp)
            .fillMaxWidth()
            .height(32.dp)
            .onGloballyPositioned { drag.fieldBounds = it.boundsInRoot() }
            .padding(start = (8 + depth * 20).dp, end = 8.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(6.dp),
    ) {
        Spacer(Modifier.width(12.dp))
        QIcon(QIcons.FOLDER_CLOSED, 16f, 15.dp, 1.25f, c.mute)
        BasicTextField(
            value = vm.newFolderName,
            onValueChange = { vm.newFolderName = it },
            singleLine = true,
            textStyle = TextStyle(fontFamily = QFonts.sans, fontSize = 13.5.sp, color = c.ink),
            cursorBrush = SolidColor(c.acc),
            keyboardOptions = KeyboardOptions(imeAction = ImeAction.Done, autoCorrectEnabled = false),
            keyboardActions = KeyboardActions(onDone = { vm.commitNewFolder() }),
            modifier = Modifier
                .weight(1f)
                .background(c.soft, RoundedCornerShape(5.dp))
                .padding(horizontal = 5.dp, vertical = 4.dp)
                .focusRequester(fr)
                .onFocusChanged {
                    if (it.isFocused) hadFocus = true else if (hadFocus) vm.cancelNewFolder()
                },
            decorationBox = { inner ->
                if (vm.newFolderName.text.isEmpty()) Txt("Folder name", 13.5.sp, c.mute)
                inner()
            },
        )
    }
}

/** A small menu under a tree row, styled like the vault menu. */
@Composable
private fun RowMenu(onDismiss: () -> Unit, content: @Composable ColumnScope.() -> Unit) {
    val c = LocalQ.current
    Popup(
        offset = IntOffset(with(LocalDensity.current) { 24.dp.roundToPx() }, with(LocalDensity.current) { 34.dp.roundToPx() }),
        onDismissRequest = onDismiss,
        properties = PopupProperties(focusable = true),
    ) {
        Column(
            Modifier
                .width(220.dp)
                .shadow(14.dp, RoundedCornerShape(9.dp), ambientColor = Shadow, spotColor = Shadow)
                .background(c.bg, RoundedCornerShape(9.dp))
                .border(1.dp, c.line, RoundedCornerShape(9.dp))
                .padding(5.dp),
            content = content,
        )
    }
}

@Composable
private fun SearchPanel(vm: QuireViewModel) {
    val c = LocalQ.current
    val fr = remember { FocusRequester() }
    val focusManager = LocalFocusManager.current
    LaunchedEffect(vm.searchFocusPending) {
        if (vm.searchFocusPending) {
            try {
                fr.requestFocus()
            } catch (e: IllegalStateException) {
                // Not attached yet.
            }
            vm.searchFocusPending = false
        }
    }
    val q = vm.query.trim()
    val results = remember(vm.files, q) { if (q.isEmpty()) emptyList() else Md.search(vm.files, q) }
    val total = results.sumOf { it.hits.size }
    val summary = when {
        q.isEmpty() -> "Searches every note in this folder"
        results.isEmpty() -> "No matches"
        else -> "$total ${if (total == 1) "match" else "matches"} in ${results.size} ${if (results.size == 1) "note" else "notes"}"
    }
    LazyColumn(Modifier.fillMaxSize()) {
        item {
            Column(Modifier.padding(horizontal = 12.dp)) {
                Row(
                    Modifier
                        .fillMaxWidth()
                        .height(36.dp)
                        .border(1.dp, c.line, RoundedCornerShape(7.dp))
                        .background(c.bg, RoundedCornerShape(7.dp))
                        .padding(start = 10.dp, end = 6.dp),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(8.dp),
                ) {
                    QIcon(QIcons.SEARCH, 16f, 13.dp, 1.5f, c.mute)
                    Box(Modifier.weight(1f)) {
                        BasicTextField(
                            value = vm.query,
                            onValueChange = { vm.query = it },
                            singleLine = true,
                            textStyle = TextStyle(fontFamily = QFonts.sans, fontSize = 13.sp, color = c.ink),
                            cursorBrush = SolidColor(c.acc),
                            keyboardOptions = KeyboardOptions(imeAction = ImeAction.Search, autoCorrectEnabled = false),
                            keyboardActions = KeyboardActions(onSearch = { focusManager.clearFocus() }),
                            modifier = Modifier.fillMaxWidth().focusRequester(fr),
                            decorationBox = { inner ->
                                if (vm.query.isEmpty()) Txt("Search notes", 13.sp, c.mute)
                                inner()
                            },
                        )
                    }
                    if (vm.query.isNotEmpty()) {
                        Box(Modifier.size(24.dp).tap(RoundedCornerShape(5.dp)) { vm.query = "" }, contentAlignment = Alignment.Center) {
                            QIcon(QIcons.CLOSE, 8f, 8.dp, 1.3f, c.mute)
                        }
                    }
                }
                Mono(summary, 11.sp, c.mute, Modifier.padding(start = 2.dp, end = 2.dp, top = 10.dp, bottom = 4.dp))
            }
        }
        items(results, key = { it.path }) { r ->
            Column(Modifier.padding(vertical = 6.dp)) {
                val first = r.hits.firstOrNull()
                Row(
                    Modifier
                        .padding(horizontal = 8.dp)
                        .fillMaxWidth()
                        .tap(RoundedCornerShape(6.dp)) {
                            if (first != null) vm.openFile(r.path, app.quire.notes.state.OpenSel(first.pos, first.pos + q.length, first.li, forceSource = true, hit = true))
                            else vm.openFile(r.path)
                        }
                        .padding(horizontal = 12.dp, vertical = 6.dp),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(8.dp),
                ) {
                    Txt(Md.base(r.path), 13.sp, c.ink, Modifier.widthIn(max = 170.dp), weight = FontWeight.W600, maxLines = 1)
                    Mono(Md.dir(r.path), 11.sp, c.mute, Modifier.weight(1f), maxLines = 1)
                    if (r.hits.isNotEmpty()) Mono(r.hits.size.toString(), 11.sp, c.mute)
                }
                r.hits.take(4).forEach { ht ->
                    val a0 = maxOf(0, ht.i - 28)
                    val b0 = minOf(ht.ln.length, ht.i + q.length)
                    val text = buildAnnotatedString {
                        append((if (a0 > 0) "…" else "") + ht.ln.substring(a0, ht.i).trimStart())
                        withStyle(SpanStyle(background = c.soft, color = c.ink)) { append(ht.ln.substring(ht.i, b0)) }
                        append(ht.ln.substring(b0, minOf(ht.ln.length, b0 + 90)))
                    }
                    Box(
                        Modifier
                            .padding(horizontal = 8.dp, vertical = 1.dp)
                            .fillMaxWidth()
                            .tap(RoundedCornerShape(6.dp)) {
                                vm.openFile(r.path, app.quire.notes.state.OpenSel(ht.pos, ht.pos + q.length, ht.li, forceSource = true, hit = true))
                            }
                            .padding(horizontal = 12.dp, vertical = 5.dp)
                    ) {
                        BasicText(
                            text,
                            style = TextStyle(fontFamily = QFonts.sans, fontSize = 12.5.sp, lineHeight = 18.sp, color = c.ink2),
                            overflow = TextOverflow.Clip,
                        )
                    }
                }
                if (r.hits.size > 4) Mono("+ ${r.hits.size - 4} more", 11.sp, c.mute, Modifier.padding(horizontal = 20.dp, vertical = 2.dp))
            }
        }
        item { Spacer(Modifier.height(12.dp)) }
    }
}

@Composable
private fun TagsPanel(vm: QuireViewModel) {
    val c = LocalQ.current
    val tags = remember(vm.files) { Md.tags(vm.files) }
    if (tags.isEmpty()) {
        Txt("No tags yet. Write #something in a note.", 13.sp, c.mute, Modifier.padding(horizontal = 22.dp, vertical = 6.dp))
        return
    }
    LazyColumn(Modifier.fillMaxSize()) {
        items(tags, key = { it.tag }) { t ->
            Row(
                Modifier
                    .padding(horizontal = 8.dp)
                    .fillMaxWidth()
                    .height(32.dp)
                    .tap(RoundedCornerShape(6.dp)) { vm.searchTag(t.tag) }
                    .padding(horizontal = 10.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Mono("#" + t.tag, 12.5.sp, c.acc, Modifier.weight(1f))
                Mono(t.n.toString(), 11.sp, c.mute)
            }
        }
    }
}

/** The head of the Pinned group: a folder row with a pin in place of the folder icon. */
@Composable
private fun PinsRow(vm: QuireViewModel, r: TreeRow, count: Int) {
    val c = LocalQ.current
    val label = "$count pinned ${if (count == 1) "note" else "notes"}"
    Row(
        Modifier
            .padding(horizontal = 8.dp)
            .fillMaxWidth()
            .height(32.dp)
            .tap(RoundedCornerShape(6.dp)) { vm.togglePinsOpen() }
            .semantics { contentDescription = label }
            .padding(start = 8.dp, end = 8.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(6.dp),
    ) {
        Box(Modifier.width(12.dp), contentAlignment = Alignment.Center) {
            QIcon(QIcons.CHEVRON_RIGHT, 8f, 8.dp, 1.4f, c.mute, Modifier.rotate(if (r.open) 90f else 0f))
        }
        QIcon(QIcons.PIN, 16f, 15.dp, 1.25f, c.mute)
        Txt(r.name, 13.5.sp, c.ink2, Modifier.weight(1f), weight = FontWeight.W500, maxLines = 1)
    }
}

