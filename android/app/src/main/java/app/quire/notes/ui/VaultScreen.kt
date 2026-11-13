package app.quire.notes.ui

import androidx.activity.compose.BackHandler
import androidx.activity.compose.LocalActivity
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.PickVisualMediaRequest
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.draganddrop.dragAndDropTarget
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.WindowInsetsSides
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.ime
import androidx.compose.foundation.layout.navigationBars
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.only
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawing
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.layout.union
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicText
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.DrawerValue
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.ModalDrawerSheet
import androidx.compose.material3.ModalNavigationDrawer
import androidx.compose.material3.rememberDrawerState
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.key
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.runtime.snapshotFlow
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draganddrop.DragAndDropEvent
import androidx.compose.ui.draganddrop.DragAndDropTarget
import androidx.compose.ui.draganddrop.mimeTypes
import androidx.compose.ui.draganddrop.toAndroidDragEvent
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Rect
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.RectangleShape
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.layout.Layout
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.LocalFocusManager
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.style.TextDecoration
import androidx.compose.ui.unit.Constraints
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import app.quire.notes.core.Md
import app.quire.notes.state.Mode
import app.quire.notes.state.QuireViewModel
import java.text.NumberFormat
import kotlin.math.max
import kotlin.math.roundToInt

@Composable
fun VaultScreen(vm: QuireViewModel) {
    val c = LocalQ.current
    val drawer = rememberDrawerState(DrawerValue.Closed)
    LaunchedEffect(vm.drawerOpen) { if (vm.drawerOpen) drawer.open() else drawer.close() }
    LaunchedEffect(drawer) {
        snapshotFlow { drawer.currentValue }.collect { if (it == DrawerValue.Closed && vm.drawerOpen) vm.drawerOpen = false }
    }
    BackHandler(enabled = vm.focus || vm.renaming || vm.drawerOpen || vm.vaultMenu || vm.outlineOpen) { vm.onBack() }

    // Insert image opens the photo picker of Android, for images only.
    val picker = rememberLauncherForActivityResult(ActivityResultContracts.PickVisualMedia()) { uri ->
        if (uri != null) vm.insertImages(listOf(uri))
    }
    LaunchedEffect(vm.pickImage) {
        if (vm.pickImage) {
            vm.pickImage = false
            picker.launch(PickVisualMediaRequest(ActivityResultContracts.PickVisualMedia.ImageOnly))
        }
    }

    // A layer on top of the editor takes the focus away, so the keyboard and the caret handles close.
    val focusManager = LocalFocusManager.current
    LaunchedEffect(vm.settingsOpen, vm.outlineOpen) {
        if (vm.settingsOpen || vm.outlineOpen) focusManager.clearFocus(force = true)
    }

    BoxWithConstraints(Modifier.fillMaxSize().background(c.bg)) {
        val screenH = maxHeight
        ModalNavigationDrawer(
            drawerState = drawer,
            gesturesEnabled = drawer.isOpen,
            scrimColor = c.scrim,
            drawerContent = {
                ModalDrawerSheet(
                    modifier = Modifier.width(300.dp).drawBehind {
                        drawLine(c.line, Offset(size.width - 0.5f, 0f), Offset(size.width - 0.5f, size.height), 1.dp.toPx())
                    },
                    drawerShape = RectangleShape,
                    drawerContainerColor = c.side,
                    drawerContentColor = c.ink,
                    drawerTonalElevation = 0.dp,
                ) {
                    // Compose the panels only while the drawer shows, so typing stays fast.
                    if (drawer.isOpen || drawer.isAnimationRunning || vm.drawerOpen) Sidebar(vm)
                }
            },
        ) {
            Column(
                Modifier
                    .fillMaxSize()
                    .background(c.bg)
                    .windowInsetsPadding(WindowInsets.safeDrawing.only(WindowInsetsSides.Horizontal))
                    .windowInsetsPadding(WindowInsets.navigationBars.union(WindowInsets.ime).only(WindowInsetsSides.Bottom))
            ) {
                if (!vm.focus) {
                    TopBar(vm)
                    TabStrip(vm)
                }
                Box(Modifier.weight(1f).fillMaxWidth().background(c.bg).imageDrop(vm)) {
                    EditorArea(vm, screenH)
                }
                if (vm.active != null && !vm.focus) StatusBar(vm)
                // The Markdown toolbar sits on the keyboard while the editor has the focus.
                val keyboard = WindowInsets.ime.getBottom(LocalDensity.current) > 0
                if (vm.active != null && vm.mode == Mode.Source && vm.editorFocused && keyboard) MarkdownBar(vm)
            }
        }
        if (vm.outlineOpen && vm.active != null) OutlineSheet(vm)
        if (vm.lockDialog && vm.active != null) LockDialog(vm)
    }
}

@Composable
private fun EditorArea(vm: QuireViewModel, screenH: Dp) {
    val c = LocalQ.current
    val active = vm.active
    when {
        // A locked note shows the lock panel in place of the editor and the preview.
        active != null && vm.noteLocked(active) -> key(active) { LockPanel(vm) }
        active != null && vm.mode == Mode.Source -> key(vm.docKey) { SourceEditor(vm, screenH) }
        active != null -> key(vm.docKey) { PreviewPane(vm, screenH) }
        vm.files.isEmpty() -> EmptyVault(vm)
        else -> NoTabHints(vm)
    }
    if (vm.focus && active != null) {
        val wc = remember(vm.editor.text) { Md.words(vm.editor.text) }
        Box(Modifier.fillMaxSize().padding(bottom = 14.dp), contentAlignment = Alignment.BottomCenter) {
            Box(
                Modifier
                    .background(c.bg, RoundedCornerShape(6.dp))
                    .tap(RoundedCornerShape(6.dp)) { vm.leaveFocus() }
                    .padding(horizontal = 10.dp, vertical = 4.dp)
            ) {
                Mono("${num(wc)} words · tap to leave focus", 11.5.sp, c.mute)
            }
        }
    }
}

private fun num(n: Int): String = NumberFormat.getIntegerInstance().format(n)

@Composable
private fun TopBar(vm: QuireViewModel) {
    val c = LocalQ.current
    val active = vm.active
    Row(
        Modifier
            .fillMaxWidth()
            .background(c.side)
            .statusBarsPadding()
            .height(48.dp)
            .padding(start = 6.dp, end = 8.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(4.dp),
    ) {
        val focusManager = LocalFocusManager.current
        IconBtn(QIcons.SIDEBAR_LEFT) {
            focusManager.clearFocus(force = true)
            vm.drawerOpen = true
        }
        Box(Modifier.weight(1f).padding(start = 4.dp)) {
            if (active != null) Crumb(vm, active) else Mono(vm.vault?.name ?: "", 12.sp, c.mute, maxLines = 1)
        }
        if (vm.showUpdate()) UpdatePill(vm, Modifier.padding(end = 4.dp))
        IconBtn(QIcons.SEARCH, stroke = 1.5f, iconSize = 15.dp) { vm.openPalette("files") }
        if (active != null) {
            // One button flips between source and preview. Its icon shows where it goes.
            IconBtn(if (vm.mode == Mode.Source) QIcons.EYE else QIcons.CODE) {
                vm.setModeTo(if (vm.mode == Mode.Source) Mode.Preview else Mode.Source)
            }
            IconBtn(QIcons.SIDEBAR_RIGHT) { vm.outlineOpen = true }
        }
    }
}

@Composable
private fun Crumb(vm: QuireViewModel, active: String) {
    val c = LocalQ.current
    val folder = Md.dir(active)
    if (vm.renaming) {
        Row(verticalAlignment = Alignment.CenterVertically) { RenameField(vm, Modifier) }
        return
    }
    // The note name wins the space. The folder shows only when enough room is left for it.
    Layout(
        content = {
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                Mono(folder, 12.sp, c.mute, Modifier.weight(1f, fill = false), maxLines = 1)
                Mono("/", 12.sp, c.mark)
            }
            Row(
                Modifier
                    .tap(RoundedCornerShape(5.dp)) { vm.startRename() }
                    .padding(horizontal = 5.dp, vertical = 4.dp)
            ) {
                Mono(Md.base(active), 12.sp, c.ink2, Modifier.weight(1f, fill = false), maxLines = 1)
                Mono(".md", 12.sp, c.mark)
            }
        },
    ) { m, cs ->
        val w = cs.maxWidth
        val name = m[1].measure(Constraints(maxWidth = w))
        val gap = 6.dp.roundToPx()
        val room = w - name.width - gap
        val dir = if (folder.isNotEmpty() && room >= 40.dp.roundToPx()) m[0].measure(Constraints(maxWidth = room)) else null
        val h = maxOf(name.height, dir?.height ?: 0)
        layout(w, h) {
            var x = 0
            if (dir != null) {
                dir.place(0, (h - dir.height) / 2)
                x = dir.width + gap
            }
            name.place(x, (h - name.height) / 2)
        }
    }
}

@Composable
private fun RenameField(vm: QuireViewModel, modifier: Modifier) {
    val c = LocalQ.current
    val fr = remember { FocusRequester() }
    var hadFocus by remember { mutableStateOf(false) }
    LaunchedEffect(Unit) {
        try {
            fr.requestFocus()
        } catch (e: IllegalStateException) {
            // Not attached yet.
        }
    }
    Row(modifier, verticalAlignment = Alignment.CenterVertically) {
        BasicTextField(
            value = vm.renameVal,
            onValueChange = { vm.renameVal = it },
            singleLine = true,
            textStyle = TextStyle(fontFamily = QFonts.plex, fontSize = 12.sp, color = c.ink),
            cursorBrush = SolidColor(c.acc),
            keyboardOptions = KeyboardOptions(imeAction = ImeAction.Done, autoCorrectEnabled = false),
            keyboardActions = KeyboardActions(onDone = { vm.commitRename() }),
            modifier = Modifier
                .weight(1f, fill = false)
                .widthIn(min = 64.dp)
                .background(c.soft, RoundedCornerShape(5.dp))
                .padding(horizontal = 5.dp, vertical = 4.dp)
                .focusRequester(fr)
                .onFocusChanged {
                    if (it.isFocused) hadFocus = true else if (hadFocus) vm.commitRename()
                },
        )
        Mono(".md", 12.sp, c.mark, Modifier.padding(start = 5.dp))
    }
}

@Composable
private fun TabStrip(vm: QuireViewModel) {
    val c = LocalQ.current
    val state = rememberLazyListState()
    val tabs = vm.tabs.filter { it in vm.files }
    val active = vm.active
    LaunchedEffect(active, tabs.size) {
        val i = tabs.indexOf(active)
        if (i >= 0) {
            val info = state.layoutInfo
            val item = info.visibleItemsInfo.find { it.index == i }
            val fits = item != null && item.offset >= 0 && item.offset + item.size <= info.viewportEndOffset
            if (!fits) state.animateScrollToItem(i)
        }
    }
    Box(
        Modifier
            .fillMaxWidth()
            .height(39.dp)
            .background(c.side)
            .drawBehind {
                val w = 1.dp.toPx()
                drawLine(c.line, Offset(0f, size.height - w / 2), Offset(size.width, size.height - w / 2), w)
            }
    ) {
        LazyRow(
            state = state,
            modifier = Modifier.fillMaxSize(),
            contentPadding = PaddingValues(start = 6.dp, end = 6.dp),
            horizontalArrangement = Arrangement.spacedBy(2.dp),
            verticalAlignment = Alignment.Bottom,
        ) {
            itemsIndexed(tabs, key = { _, p -> p }) { _, p -> Tab(vm, p, p == active) }
            item {
                Box(Modifier.padding(start = 2.dp, bottom = 4.dp)) {
                    IconBtn(QIcons.PLUS, viewport = 12f, iconSize = 12.dp, size = 30.dp) { vm.newNote() }
                }
            }
        }
    }
}

@Composable
private fun Tab(vm: QuireViewModel, path: String, on: Boolean) {
    val c = LocalQ.current
    Row(
        Modifier
            .height(35.dp)
            .widthIn(min = 72.dp, max = 190.dp)
            .then(
                if (on) Modifier.drawBehind {
                    val r = 9.dp.toPx()
                    val sw = 1.dp.toPx()
                    val w = size.width
                    val h = size.height
                    val fill = Path().apply {
                        moveTo(0f, h)
                        lineTo(0f, r)
                        arcTo(Rect(0f, 0f, 2 * r, 2 * r), 180f, 90f, false)
                        lineTo(w - r, 0f)
                        arcTo(Rect(w - 2 * r, 0f, w, 2 * r), 270f, 90f, false)
                        lineTo(w, h)
                        close()
                    }
                    drawPath(fill, c.bg)
                    val o = sw / 2
                    val edge = Path().apply {
                        moveTo(o, h)
                        lineTo(o, r)
                        arcTo(Rect(o, o, 2 * r - o, 2 * r - o), 180f, 90f, false)
                        lineTo(w - r, o)
                        arcTo(Rect(w - 2 * r + o, o, w - o, 2 * r - o), 270f, 90f, false)
                        lineTo(w - o, h)
                    }
                    drawPath(edge, c.line, style = Stroke(sw))
                } else Modifier
            )
            .tap(RoundedCornerShape(topStart = 9.dp, topEnd = 9.dp), pressed = Color.Transparent) { if (!on) vm.openFile(path) }
            .padding(start = 14.dp, end = 6.dp, bottom = 1.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(6.dp),
    ) {
        Txt(Md.base(path), 13.sp, if (on) c.ink else c.mute, Modifier.weight(1f, fill = false), maxLines = 1)
        Box(
            Modifier
                .size(24.dp)
                .alpha(if (on) 1f else 0.55f)
                .tap(RoundedCornerShape(5.dp)) { vm.closeTab(path) },
            contentAlignment = Alignment.Center,
        ) { QIcon(QIcons.CLOSE, 8f, 8.dp, 1.3f, c.mute) }
    }
}

@Composable
private fun StatusBar(vm: QuireViewModel) {
    val c = LocalQ.current
    val doc = vm.editor.text
    val preview = vm.mode == Mode.Preview
    val sel = vm.editor.selection
    val pos = minOf(sel.start, doc.length)
    var line = 0
    var ls = 0
    for (i in 0 until pos) if (doc[i] == '\n') {
        line++
        ls = i + 1
    }
    val col = pos - ls
    val selText = if (sel.end > sel.start) doc.substring(sel.start, minOf(sel.end, doc.length)) else ""
    val wc = remember(doc) { Md.words(doc) }
    val swc = if (selText.isNotEmpty()) Md.words(selText) else 0
    val link = if (!preview) vm.caretLink() else null
    val hint = link?.let { (if (Md.resolve(vm.names, it.second) != null) "tap to open “" else "tap to create “") + it.second + "”" }
    Row(
        Modifier
            .fillMaxWidth()
            .height(28.dp)
            .drawBehind { drawLine(c.line, Offset(0f, 0.5f), Offset(size.width, 0.5f), 1.dp.toPx()) }
            .padding(horizontal = 16.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(18.dp),
    ) {
        Mono(
            if (preview) "preview" else if (selText.isNotEmpty()) "${num(selText.length)} selected" else "Ln ${line + 1}, Col ${col + 1}",
            11.5.sp, c.mute, maxLines = 1,
        )
        when {
            hint != null -> Txt(hint, 11.5.sp, c.acc, Modifier.weight(1f), family = QFonts.plex, maxLines = 1)
            else -> Spacer(Modifier.weight(1f))
        }
        Mono(
            if (selText.isNotEmpty()) "${num(swc)} of ${num(wc)} words" else "${num(wc)} ${if (wc == 1) "word" else "words"}",
            11.5.sp, c.mute, maxLines = 1,
        )
        val sync = vm.syncLabel()
        if (sync != null) {
            // The phone has no room for both, so the sync state takes the place of the reading time.
            val (label, busy, failed) = sync
            Row(
                Modifier.fillMaxHeight().tap(RoundedCornerShape(4.dp)) { vm.openSyncSettings() },
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(6.dp),
            ) {
                Box(Modifier.size(6.dp).background(if (failed) c.err else if (busy) c.acc else c.mark, CircleShape))
                Mono(label, 11.5.sp, if (failed) c.err else c.mute, maxLines = 1)
            }
        } else if (hint == null) {
            Mono("${max(1, (wc / 230f).roundToInt())} min read", 11.5.sp, c.mute, maxLines = 1)
        }
    }
}

@Composable
private fun NoTabHints(vm: QuireViewModel) {
    val c = LocalQ.current
    Box(Modifier.fillMaxSize().padding(40.dp), contentAlignment = Alignment.Center) {
        Column(Modifier.width(260.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
            listOf<Pair<String, () -> Unit>>(
                "Open a note" to { vm.openPalette("files") },
                "New note" to { vm.newNote() },
                "All commands" to { vm.openPalette("commands") },
                "Search" to { vm.openSearch() },
            ).forEach { (label, run) ->
                Box(Modifier.fillMaxWidth().tap(RoundedCornerShape(6.dp), onClick = run).padding(vertical = 6.dp)) {
                    Mono(label, 12.5.sp, c.ink2)
                }
            }
        }
    }
}

@Composable
private fun EmptyVault(vm: QuireViewModel) {
    val c = LocalQ.current
    Box(Modifier.fillMaxSize().verticalScroll(rememberScrollState()).padding(40.dp), contentAlignment = Alignment.Center) {
        Column(Modifier.widthIn(max = 380.dp), verticalArrangement = Arrangement.spacedBy(14.dp)) {
            Txt("Nothing here yet.", 32.sp, c.ink, family = QFonts.serifHead, lineHeight = 37.sp)
            Txt("Notes you write here are saved as plain .md files in ${vm.vault?.path ?: ""}.", 14.sp, c.ink2, lineHeight = 22.sp)
            PrimaryButton("Create your first note", Modifier.padding(top = 6.dp), height = 36.dp, size = 13.5.sp) { vm.newNote() }
        }
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun OutlineSheet(vm: QuireViewModel) {
    val c = LocalQ.current
    val active = vm.active ?: return
    val sheet = rememberModalBottomSheetState(skipPartiallyExpanded = false)
    val doc = vm.editor.text
    val heads = remember(doc) { Md.headings(doc) }
    val backlinks = remember(vm.files, active) { Md.backlinks(vm.files, active) }
    val caretLine = remember(doc, vm.editor.selection) {
        var n = 0
        val p = minOf(vm.editor.selection.start, doc.length)
        for (i in 0 until p) if (doc[i] == '\n') n++
        n
    }
    var cur = -1
    if (vm.mode == Mode.Source) heads.forEachIndexed { i, h -> if (h.line <= caretLine) cur = i } else cur = minOf(vm.prevHead, heads.size - 1)
    val minLv = heads.minOfOrNull { it.level } ?: 1
    ModalBottomSheet(
        onDismissRequest = { vm.outlineOpen = false },
        sheetState = sheet,
        shape = RoundedCornerShape(topStart = 12.dp, topEnd = 12.dp),
        containerColor = c.bg,
        contentColor = c.ink,
        scrimColor = c.scrim,
        tonalElevation = 0.dp,
        modifier = Modifier.border(1.dp, c.line, RoundedCornerShape(topStart = 12.dp, topEnd = 12.dp)),
        dragHandle = {
            Box(Modifier.padding(top = 10.dp, bottom = 6.dp).size(36.dp, 4.dp).background(c.mark, CircleShape))
        },
    ) {
        Column(
            Modifier
                .fillMaxWidth()
                .verticalScroll(rememberScrollState())
                .padding(start = 10.dp, end = 10.dp, top = 10.dp, bottom = 24.dp)
                .navigationBarsPadding()
        ) {
            Label("outline", Modifier.padding(start = 10.dp, end = 10.dp, bottom = 8.dp))
            heads.forEachIndexed { i, h ->
                val on = i == cur
                Row(
                    Modifier
                        .fillMaxWidth()
                        .tap(RoundedCornerShape(6.dp)) { vm.jump(h.line) }
                        .padding(start = (8 + (h.level - minLv) * 14).dp, end = 8.dp, top = 8.dp, bottom = 8.dp),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(8.dp),
                ) {
                    Box(Modifier.size(5.dp).background(if (on) c.acc else Color.Transparent, CircleShape))
                    Txt(h.text, 14.sp, if (on) c.ink else c.ink2, weight = if (on) FontWeight.W500 else FontWeight.W400, maxLines = 1)
                }
            }
            if (heads.isEmpty()) Txt("No headings in this note.", 13.sp, c.mute, Modifier.padding(horizontal = 10.dp, vertical = 2.dp))
            Label("linked from", Modifier.padding(start = 10.dp, end = 10.dp, top = 26.dp, bottom = 8.dp))
            backlinks.forEach { b ->
                Row(
                    Modifier
                        .fillMaxWidth()
                        .tap(RoundedCornerShape(6.dp)) { vm.openFile(b.path) }
                        .padding(horizontal = 10.dp, vertical = 8.dp),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(8.dp),
                ) {
                    Txt(Md.base(b.path), 14.sp, c.ink2, Modifier.weight(1f, fill = false), maxLines = 1)
                    Mono(Md.dir(b.path), 10.5.sp, c.mute, Modifier.weight(1f), maxLines = 1)
                }
            }
            if (backlinks.isEmpty()) Txt("No notes link here yet.", 13.sp, c.mute, Modifier.padding(horizontal = 10.dp, vertical = 2.dp))
        }
    }
}

/**
 * Takes images that another app drops on the note, in split screen or in desktop mode. They go
 * into attachments/ and get a link at the caret.
 */
@Composable
private fun Modifier.imageDrop(vm: QuireViewModel): Modifier {
    val activity = LocalActivity.current
    val target = remember(vm, activity) {
        object : DragAndDropTarget {
            override fun onDrop(event: DragAndDropEvent): Boolean {
                val drag = event.toAndroidDragEvent()
                // Files from another app need this permission while Quire reads them.
                activity?.requestDragAndDropPermissions(drag)
                val clip = drag.clipData ?: return false
                val uris = (0 until clip.itemCount).mapNotNull { clip.getItemAt(it).uri }
                if (uris.isEmpty()) return false
                vm.insertImages(uris)
                return true
            }
        }
    }
    return dragAndDropTarget(
        shouldStartDragAndDrop = { e -> vm.active != null && e.mimeTypes().any { it.startsWith("image/") } },
        target = target,
    )
}

/** One row of Markdown actions. It scrolls sideways when it does not fit. */
@Composable
private fun MarkdownBar(vm: QuireViewModel) {
    val c = LocalQ.current
    Row(
        Modifier
            .fillMaxWidth()
            .height(42.dp)
            .background(c.side)
            .drawBehind { drawLine(c.line, Offset(0f, 0.5f), Offset(size.width, 0.5f), 1.dp.toPx()) }
            .horizontalScroll(rememberScrollState())
            .padding(horizontal = 6.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        val unlocked = vm.active?.let { it in vm.unlocked } == true
        vm.toolbar.forEach { a ->
            val label = toolLabel(a)
            // The Lock button shows as active while the note is unlocked. A tap locks it again.
            // The Pin button shows as active while the note is pinned.
            val on = (a == "lock" && unlocked) || (a == "pin" && vm.active?.let { vm.isPinned(it) } == true)
            Box(
                Modifier
                    .size(42.dp, 38.dp)
                    .background(if (on) c.act else Color.Transparent, RoundedCornerShape(6.dp))
                    .tap(RoundedCornerShape(6.dp)) { vm.format(a) }
                    .semantics { contentDescription = label },
                contentAlignment = Alignment.Center,
            ) { ToolGlyph(a, c, on) }
        }
    }
}

private fun toolLabel(a: String) = when (a) {
    "bold" -> "Bold"
    "italic" -> "Italic"
    "strike" -> "Strikethrough"
    "heading" -> "Heading"
    "link" -> "Link"
    "noteLink" -> "Note link"
    "bullet" -> "Bulleted list"
    "number" -> "Numbered list"
    "task" -> "Task"
    "quote" -> "Quote"
    "code" -> "Code"
    "lock" -> "Lock"
    "pin" -> "Pin"
    else -> "Image"
}

@Composable
private fun ToolGlyph(a: String, c: QColors, on: Boolean = false) {
    val ink = c.ink2
    when (a) {
        "bold" -> Txt("B", 16.sp, ink, weight = FontWeight.W700)
        "italic" -> Txt("I", 17.sp, ink, family = QFonts.serifText, italic = true)
        "strike" -> BasicText("S", style = TextStyle(fontFamily = QFonts.sans, fontSize = 16.sp, color = ink, textDecoration = TextDecoration.LineThrough))
        "heading" -> Txt("H", 16.sp, ink, weight = FontWeight.W600)
        "link" -> QIcon(QIcons.LINK, 16f, 16.dp, 1.3f, ink)
        "noteLink" -> Mono("[[ ]]", 12.sp, ink)
        "bullet" -> QIcon(QIcons.LIST, 16f, 16.dp, 1.3f, ink)
        "number" -> Mono("1.", 13.sp, ink)
        "task" -> QIcon(QIcons.TASK, 16f, 16.dp, 1.3f, ink)
        "quote" -> QIcon(QIcons.QUOTE, 16f, 16.dp, 1.3f, ink)
        "code" -> QIcon(QIcons.CODE, 16f, 16.dp, 1.3f, ink)
        "lock" -> QIcon(QIcons.LOCK, 16f, 16.dp, 1.3f, lockColor(c, on))
        "pin" -> QIcon(QIcons.PIN, 16f, 16.dp, 1.25f, lockColor(c, on))
        else -> QIcon(QIcons.IMAGE, 16f, 16.dp, 1.25f, ink)
    }
}

/**
 * The Update button: a small pill in the accent. A press downloads the newer release and opens
 * the installer. During the download it shows "Updating…" and takes no press.
 */
@Composable
internal fun UpdatePill(vm: QuireViewModel, modifier: Modifier = Modifier) {
    val c = LocalQ.current
    val v = vm.update?.version ?: return
    val help = "Quire $v is out. Select to update."
    Box(
        modifier
            .height(24.dp)
            .clip(RoundedCornerShape(12.dp))
            .background(c.button)
            .tap(RoundedCornerShape(12.dp), pressed = c.buttonPressed, enabled = !vm.updating) { vm.startUpdate() }
            .semantics { contentDescription = help }
            .padding(horizontal = 10.dp),
        contentAlignment = Alignment.Center,
    ) {
        Txt(if (vm.updating) "Updating…" else "Update", 12.5.sp, c.onAcc, weight = FontWeight.W500, maxLines = 1)
    }
}

