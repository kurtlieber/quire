package app.quire.notes.ui

import androidx.compose.foundation.gestures.awaitEachGesture
import androidx.compose.foundation.gestures.awaitFirstDown
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.ime
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.BasicText
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.text.contextmenu.builder.item
import androidx.compose.foundation.text.contextmenu.modifier.appendTextContextMenuComponents
import androidx.compose.foundation.text.selection.LocalTextSelectionColors
import androidx.compose.foundation.text.selection.TextSelectionColors
import androidx.compose.foundation.verticalScroll
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.runtime.withFrameNanos
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.input.pointer.PointerEventPass
import androidx.compose.ui.input.pointer.isCtrlPressed
import androidx.compose.ui.input.pointer.isMetaPressed
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.layout.onGloballyPositioned
import androidx.compose.ui.layout.positionInRoot
import androidx.compose.ui.platform.LocalClipboard
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.LocalSoftwareKeyboardController
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.TextLayoutResult
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.OffsetMapping
import androidx.compose.ui.text.input.TransformedText
import androidx.compose.ui.text.input.VisualTransformation
import androidx.compose.ui.text.rememberTextMeasurer
import androidx.compose.ui.text.style.LineHeightStyle
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import app.quire.notes.core.Md
import app.quire.notes.state.QuireViewModel

/** Shows the highlighted text in place of the plain text. The text itself never changes. */
private class HighlightTransformation(private val rich: Rich) : VisualTransformation {
    override fun filter(text: AnnotatedString): TransformedText {
        val out = if (text.text == rich.text.text) rich.text else AnnotatedString(text.text)
        return TransformedText(out, OffsetMapping.Identity)
    }
}

/** The text style of the editor column. The line height matches the prototype's 1.75. */
fun editorStyle(family: androidx.compose.ui.text.font.FontFamily, size: Int, c: QColors) = TextStyle(
    fontFamily = family,
    fontSize = size.sp,
    lineHeight = (size * 1.75f).sp,
    letterSpacing = 0.sp,
    color = c.ink,
    fontFeatureSettings = "liga 0, calt 0",
    lineHeightStyle = LineHeightStyle(LineHeightStyle.Alignment.Center, LineHeightStyle.Trim.None),
)

/** The width of 72 characters ("72ch") in the editor font, for the readable line width. */
@Composable
fun columnMax(vm: QuireViewModel): Dp {
    val set = vm.settings
    if (set.width != "readable") return Dp.Unspecified
    val m = rememberTextMeasurer()
    val d = LocalDensity.current
    val ch = remember(set.font, set.size, d) {
        m.measure("0", TextStyle(fontFamily = QFonts.editor(set.font), fontSize = set.size.sp)).size.width
    }
    return with(d) { (ch * if (vm.focus) 68 else 72).toDp() }
}

/** The side padding of the text column: clamp(20px, 5vw, 56px), or 6vw up to 64px in focus mode. */
fun sidePad(width: Dp, focus: Boolean): Dp =
    ((if (focus) 0.06f else 0.05f) * width.value).coerceIn(20f, if (focus) 64f else 56f).dp

@Composable
fun SourceEditor(vm: QuireViewModel, screenH: Dp) {
    val c = LocalQ.current
    val set = vm.settings
    val density = LocalDensity.current
    val keyboard = LocalSoftwareKeyboardController.current
    val scroll = rememberScrollState()
    val fr = remember { FocusRequester() }
    var layout by remember { mutableStateOf<TextLayoutResult?>(null) }
    var fieldY by remember { mutableStateOf(0f) }
    var fieldH by remember { mutableStateOf(0) }
    val style = editorStyle(QFonts.editor(set.font), set.size, c)
    val value = vm.editor
    val focusMode = vm.focus
    val dim = if (focusMode && set.dimFocus) Md.paraRange(value.text, value.selection.start) else null
    val names = vm.names
    val hit = vm.hit
    val rich = remember(value.text, c, names, dim, hit) { Highlighter.highlight(value.text, c, names, dim, hit) }
    val vt = remember(rich) { HighlightTransformation(rich) }
    val topPad = if (focusMode) screenH * 0.16f else 34.dp
    val bottomPad = screenH * (if (focusMode) 0.48f else 0.38f)
    val colMax = columnMax(vm)
    val imeBottom = WindowInsets.ime.getBottom(density)
    val basePx = with(density) { set.size.sp.toPx() }

    // The text selection menu has "Note actions…", which opens the note menu under the selection.
    // The box and the field keep their places in the root, so the menu can sit at the selection.
    var boxAt by remember { mutableStateOf(Offset.Zero) }
    var fieldAt by remember { mutableStateOf(Offset.Zero) }
    var actionsAt by remember { mutableStateOf<IntOffset?>(null) }
    val openActions = {
        val l = layout
        val r = if (l != null && l.layoutInput.text.length == value.text.length) {
            l.getBoundingBox(value.selection.min.coerceIn(0, maxOf(value.text.length - 1, 0)))
        } else null
        val x = fieldAt.x - boxAt.x + (r?.left ?: 0f)
        val y = fieldAt.y - boxAt.y + (r?.bottom ?: 0f)
        actionsAt = IntOffset(x.toInt(), y.toInt())
    }

    BoxWithConstraints(Modifier.fillMaxSize().onGloballyPositioned { boxAt = it.positionInRoot() }) {
        val viewH = constraints.maxHeight
        val side = sidePad(maxWidth, focusMode)

        // Keep the caret above the keyboard and inside the view, like ensureCaret() in the prototype.
        var lastText by remember { mutableStateOf(value.text) }
        LaunchedEffect(value.text, value.selection, imeBottom, viewH, focusMode) {
            val typed = lastText != value.text
            lastText = value.text
            if (!vm.editorFocused) return@LaunchedEffect
            withFrameNanos { }
            val l = layout ?: return@LaunchedEffect
            if (l.layoutInput.text.length != value.text.length) return@LaunchedEffect
            val r = l.getCursorRect(value.selection.end.coerceIn(0, value.text.length))
            val top = fieldY + r.top
            val bottom = fieldY + r.bottom
            val lead = with(density) { 48.dp.toPx() }
            val head = with(density) { 24.dp.toPx() }
            if (focusMode && set.typewriter) {
                if (typed || imeBottom > 0) scroll.scrollTo((top + r.height / 2 - viewH * 0.42f).toInt().coerceAtLeast(0))
            } else if (bottom > scroll.value + viewH - lead) {
                scroll.scrollTo((bottom - viewH + lead).toInt())
            } else if (top < scroll.value + head) {
                scroll.scrollTo((top - head).toInt().coerceAtLeast(0))
            }
        }

        // Scroll a heading or a search match to a quarter of the view.
        val req = vm.scrollReq
        LaunchedEffect(req, layout) {
            val q = req ?: return@LaunchedEffect
            val l = layout ?: return@LaunchedEffect
            if (l.layoutInput.text.length != value.text.length) return@LaunchedEffect
            var pos = 0
            for (i in 0 until q.line) {
                val n = value.text.indexOf('\n', pos)
                if (n < 0) break
                pos = n + 1
            }
            val y = fieldY + l.getLineTop(l.getLineForOffset(pos.coerceAtMost(value.text.length)))
            scroll.scrollTo((y - viewH * 0.25f).toInt().coerceAtLeast(0))
            vm.scrollReq = null
        }

        LaunchedEffect(vm.pendingEditorFocus) {
            if (vm.pendingEditorFocus) {
                withFrameNanos { }
                try {
                    fr.requestFocus()
                    keyboard?.show()
                } catch (e: IllegalStateException) {
                    // The field is not attached yet.
                }
                vm.pendingEditorFocus = false
            }
        }

        Column(
            Modifier
                .fillMaxSize()
                .verticalScroll(scroll)
                .pointerInput(Unit) {
                    // A tap on the empty column puts the caret at the end, like onColDown() in the prototype.
                    detectTapGestures { p ->
                        val inField = p.y >= fieldY && p.y <= fieldY + fieldH
                        if (!inField) vm.caretToEnd()
                    }
                }
        ) {
            Box(
                Modifier
                    .fillMaxWidth()
                    .padding(start = side, end = side, top = topPad, bottom = bottomPad),
                contentAlignment = Alignment.TopCenter,
            ) {
                val key = vm.docKey
                // The editor selection is a little stronger than the one in fields. The paste
                // clipboard turns formatted copies into Markdown.
                val system = LocalClipboard.current
                val paste = remember(system, vm) { PasteClipboard(system, vm) }
                CompositionLocalProvider(
                    LocalTextSelectionColors provides TextSelectionColors(handleColor = c.acc, backgroundColor = c.selEditor),
                    LocalClipboard provides paste,
                ) {
                    BasicTextField(
                        value = value,
                        onValueChange = { vm.onEditorChange(key, it) },
                        modifier = Modifier
                            .widthIn(max = colMax)
                            .fillMaxWidth()
                            .onGloballyPositioned {
                                // The field is the only child of the padded box, so it starts at the top padding.
                                fieldY = with(density) { topPad.toPx() }
                                fieldH = it.size.height
                                fieldAt = it.positionInRoot()
                            }
                            .appendTextContextMenuComponents {
                                separator()
                                item(NoteActionsKey, "Note actions…") {
                                    close()
                                    openActions()
                                }
                            }
                            .focusRequester(fr)
                            .onFocusChanged { vm.editorFocused = it.isFocused }
                            .drawBehind { layout?.let { drawDecos(it, rich.decos, basePx) } }
                            .linkGestures(vm) { layout },
                        textStyle = style,
                        cursorBrush = SolidColor(c.acc),
                        visualTransformation = vt,
                        onTextLayout = { layout = it },
                        keyboardOptions = KeyboardOptions(
                            capitalization = KeyboardCapitalization.None,
                            autoCorrectEnabled = set.spellcheck,
                            keyboardType = KeyboardType.Text,
                        ),
                        decorationBox = { inner ->
                            Box {
                                if (value.text.isEmpty()) BasicText("Start writing…", style = style.copy(color = c.mute))
                                inner()
                            }
                        },
                    )
                }
            }
        }
        val a = vm.active
        val at = actionsAt
        if (a != null && at != null) NoteMenu(vm, a, onTab = false, at = at) { actionsAt = null }
    }
}

/**
 * Follows a wiki link in source mode. A long-press on a link follows it. A tap on a link follows it
 * when the caret is already in that link, which is when the status bar shows the link hint.
 * A Ctrl-tap with a mouse or a hardware keyboard also follows it.
 */
private fun Modifier.linkGestures(vm: QuireViewModel, layout: () -> TextLayoutResult?): Modifier =
    pointerInput(vm) {
        awaitEachGesture {
            val down = awaitFirstDown(requireUnconsumed = false, pass = PointerEventPass.Initial)
            val before = vm.caretLink()
            val ctrl = currentEvent.keyboardModifiers.isCtrlPressed || currentEvent.keyboardModifiers.isMetaPressed
            val start = down.position
            var moved = false
            var up: Offset? = null
            val finished: Boolean? = withTimeoutOrNull(viewConfiguration.longPressTimeoutMillis) {
                var done = false
                while (!done) {
                    val ev = awaitPointerEvent(PointerEventPass.Initial)
                    val ch = ev.changes.firstOrNull { it.id == down.id }
                    if (ch == null) {
                        done = true
                    } else if ((ch.position - start).getDistance() > viewConfiguration.touchSlop) {
                        moved = true
                        done = true
                    } else if (!ch.pressed) {
                        up = ch.position
                        done = true
                    }
                }
                true
            }
            if (moved) return@awaitEachGesture
            val l = layout() ?: return@awaitEachGesture
            fun linkAt(p: Offset): Pair<IntRange, String>? {
                val line = l.getLineForVerticalPosition(p.y)
                if (p.x > l.getLineRight(line) + 4) return null
                val off = l.getOffsetForPosition(p)
                val text = l.layoutInput.text.text
                val ls = if (off == 0) 0 else text.lastIndexOf('\n', off - 1) + 1
                val le = text.indexOf('\n', off).let { if (it < 0) text.length else it }
                val r = Md.linkAt(text.substring(ls, le), off - ls) ?: return null
                return IntRange(r.first.first + ls, r.first.last + ls) to r.second
            }
            if (finished == null) {
                // Long press.
                linkAt(start)?.let { vm.openLink(it.second) }
                return@awaitEachGesture
            }
            val tapAt = up ?: return@awaitEachGesture
            val link = linkAt(tapAt) ?: return@awaitEachGesture
            if (ctrl || (before != null && before.first == link.first)) vm.openLink(link.second)
        }
    }

/** The key of the "Note actions…" item in the text selection menu. */
private object NoteActionsKey
