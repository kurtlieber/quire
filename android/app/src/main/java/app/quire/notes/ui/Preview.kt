package app.quire.notes.ui

import android.content.ActivityNotFoundException
import android.content.Intent
import android.net.Uri
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.IntrinsicSize
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicText
import androidx.compose.foundation.verticalScroll
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateMapOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.runtime.snapshotFlow
import androidx.compose.runtime.withFrameNanos
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.hapticfeedback.HapticFeedbackType
import androidx.compose.ui.layout.onGloballyPositioned
import androidx.compose.ui.layout.positionInParent
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.LocalHapticFeedback
import androidx.compose.ui.text.TextLayoutResult
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.LineHeightStyle
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.TextUnit
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.em
import androidx.compose.ui.unit.sp
import app.quire.notes.core.BCode
import app.quire.notes.core.BHead
import app.quire.notes.core.BHr
import app.quire.notes.core.BImage
import app.quire.notes.core.BList
import app.quire.notes.core.BPara
import app.quire.notes.core.BQuote
import app.quire.notes.core.Block
import app.quire.notes.core.LItem
import app.quire.notes.core.Md
import app.quire.notes.state.QuireViewModel
import kotlin.math.max

/** Text with decorations drawn behind it. */
@Composable
fun RichText(rich: Rich, style: TextStyle, modifier: Modifier = Modifier) {
    var layout by remember { mutableStateOf<TextLayoutResult?>(null) }
    val d = LocalDensity.current
    val basePx = with(d) { if (style.fontSize == TextUnit.Unspecified) 16.sp.toPx() else style.fontSize.toPx() }
    BasicText(
        text = rich.text,
        modifier = modifier.drawBehind { layout?.let { drawDecos(it, rich.decos, basePx) } },
        style = style,
        onTextLayout = { layout = it },
    )
}

/** Renders a note like renderPreview() and pStyles() in the prototype. */
@Composable
fun PreviewPane(vm: QuireViewModel, screenH: Dp) {
    val c = LocalQ.current
    val set = vm.settings
    val ctx = LocalContext.current
    val density = LocalDensity.current
    // An unlocked note gives its plain text. The folder copy stays sealed.
    val text = vm.activeText()
    val blocks = remember(text) { Md.parseBlocks(text) }
    val serif = set.previewFont == "serif"
    val rootSize = set.size + if (serif) 3 else 1
    val mono = QFonts.editor(set.font)
    val names = vm.names
    val scroll = rememberScrollState()
    val positions = remember { mutableStateMapOf<Int, Float>() }
    val act = remember(vm) {
        object : PreviewActions {
            override fun openLink(target: String) = vm.openLink(target)
            override fun searchTag(tag: String) = vm.searchTag(tag)
            override fun openUrl(url: String) {
                try {
                    ctx.startActivity(Intent(Intent.ACTION_VIEW, Uri.parse(url)))
                } catch (e: ActivityNotFoundException) {
                    vm.flash("No app can open $url.")
                } catch (e: Exception) {
                    vm.flash("Quire can't open $url.")
                }
            }
        }
    }
    val root = TextStyle(
        fontFamily = if (serif) QFonts.serifText else QFonts.sans,
        fontSize = rootSize.sp,
        lineHeight = (if (serif) 1.62f else 1.65f).em,
        color = c.ink,
        lineHeightStyle = LineHeightStyle(LineHeightStyle.Alignment.Center, LineHeightStyle.Trim.None),
    )
    val rPx = with(density) { rootSize.sp.toDp() }
    fun em(x: Float): Dp = rPx * x

    // Scroll to a heading from the outline.
    val req = vm.scrollReq
    LaunchedEffect(req) {
        val q = req ?: return@LaunchedEffect
        withFrameNanos { }
        val y = positions[q.line]
        if (y != null) scroll.animateScrollTo(max(0, (y - with(density) { 32.dp.toPx() }).toInt()))
        vm.scrollReq = null
    }
    // Track the heading at the top for the outline, like onScroll() in the prototype.
    LaunchedEffect(blocks) {
        snapshotFlow { scroll.value }.collect { v ->
            val top = v + with(density) { 100.dp.toPx() }
            var cur = 0
            blocks.forEach { b -> if (b is BHead && (positions[b.line] ?: Float.MAX_VALUE) <= top) cur = b.index }
            if (cur != vm.prevHead) vm.prevHead = cur
        }
    }

    // A long-press anywhere on the note opens the note menu at the press point.
    var actionsAt by remember { mutableStateOf<IntOffset?>(null) }
    val haptics = LocalHapticFeedback.current
    BoxWithConstraints(
        Modifier.fillMaxSize().longPressAt { p ->
            haptics.performHapticFeedback(HapticFeedbackType.LongPress)
            actionsAt = IntOffset(p.x.toInt(), p.y.toInt())
        }
    ) {
        val side = sidePad(maxWidth, vm.focus)
        val colMax = columnMax(vm)
        Column(Modifier.fillMaxSize().verticalScroll(scroll), horizontalAlignment = Alignment.CenterHorizontally) {
            Column(
                Modifier
                    .widthIn(max = if (colMax == Dp.Unspecified) Dp.Infinity else colMax + side * 2)
                    .fillMaxWidth()
                    .padding(
                        start = side, end = side,
                        top = if (vm.focus) screenH * 0.16f else 34.dp,
                        bottom = screenH * (if (vm.focus) 0.48f else 0.38f),
                    )
            ) {
                var prevBottom = 0.dp
                blocks.forEachIndexed { i, b ->
                    val (mt, mb) = margins(b)
                    val gap = if (i == 0) em(mt) else max(prevBottom.value, em(mt).value).dp
                    Spacer(Modifier.height(gap))
                    Box(Modifier.fillMaxWidth().onGloballyPositioned { positions[b.line] = it.positionInParent().y }) {
                        BlockView(b, root, serif, mono, c, names, act, vm, ::em)
                    }
                    prevBottom = em(mb)
                }
                Spacer(Modifier.height(prevBottom))
            }
        }
        val a = vm.active
        val at = actionsAt
        if (a != null && at != null) NoteMenu(vm, a, onTab = false, at = at) { actionsAt = null }
    }
}

/** Top and bottom margins in root em, with the element font size already applied. */
private fun margins(b: Block): Pair<Float, Float> = when (b) {
    is BHead -> when (b.level) {
        1 -> 0f to 0.7f * 1.72f
        2 -> 1.7f * 1.3f to 0.55f * 1.3f
        else -> 1.5f * 1.08f to 0.45f * 1.08f
    }
    is BPara -> 0f to 0.95f
    is BQuote -> 1.3f to 1.3f
    is BCode -> 1.2f * 0.74f to 1.3f * 0.74f
    is BHr -> 2.2f to 2.2f
    is BList -> 0.25f to 1f
    is BImage -> 0.4f to 0.95f
}

@Composable
private fun BlockView(
    b: Block,
    root: TextStyle,
    serif: Boolean,
    mono: androidx.compose.ui.text.font.FontFamily,
    c: QColors,
    names: Map<String, String>,
    act: PreviewActions,
    vm: QuireViewModel,
    em: (Float) -> Dp,
) {
    fun rich(s: String) = Inline.build(s, c, names, mono, act)
    when (b) {
        is BHead -> {
            val (size, lh, w) = when (b.level) {
                1 -> Triple(1.72f, 1.15f, if (serif) FontWeight.W500 else FontWeight.W600)
                2 -> Triple(1.3f, 1.25f, FontWeight.W600)
                else -> Triple(1.08f, 1.3f, FontWeight.W600)
            }
            val st = root.copy(
                fontFamily = if (serif) (if (b.level <= 2) QFonts.serifHead else QFonts.serifText) else QFonts.sans,
                fontSize = root.fontSize * size,
                lineHeight = lh.em,
                fontWeight = w,
                letterSpacing = if (b.level == 1) (-0.012f).em else TextUnit.Unspecified,
            )
            RichText(remember(b, c, names) { rich(b.text) }, st)
        }
        is BPara -> RichText(remember(b, c, names) { rich(b.text) }, root)
        is BQuote -> Row(Modifier.height(IntrinsicSize.Min)) {
            Box(Modifier.width(2.dp).fillMaxHeight().background(c.mark))
            Box(Modifier.padding(start = em(1.1f), top = em(0.1f), bottom = em(0.1f))) {
                RichText(remember(b, c, names) { rich(b.text) }, root.copy(color = c.ink2, fontStyle = FontStyle.Italic))
            }
        }
        is BCode -> Box(
            Modifier
                .fillMaxWidth()
                .background(c.code, RoundedCornerShape(8.dp))
                .horizontalScroll(rememberScrollState())
                .padding(horizontal = 18.dp, vertical = 14.dp)
        ) {
            BasicText(
                b.code,
                style = TextStyle(
                    fontFamily = mono, fontSize = root.fontSize * 0.74f, lineHeight = 1.65.em, color = c.ink2,
                    lineHeightStyle = LineHeightStyle(LineHeightStyle.Alignment.Center, LineHeightStyle.Trim.None),
                ),
                softWrap = false,
            )
        }
        is BHr -> Box(Modifier.fillMaxWidth().height(1.dp).background(c.line))
        is BImage -> Column(verticalArrangement = Arrangement.spacedBy(em(0.6f))) {
            b.images.forEach { img -> Picture(vm, img, c, root.fontSize, em) }
        }
        is BList -> Column(verticalArrangement = Arrangement.spacedBy(em(0.25f))) {
            b.items.forEach { item -> ListRow(item, root, mono, c, vm, em) { s -> remember(s, c, names) { rich(s) } } }
        }
    }
}

@Composable
private fun ListRow(
    item: LItem,
    root: TextStyle,
    mono: androidx.compose.ui.text.font.FontFamily,
    c: QColors,
    vm: QuireViewModel,
    em: (Float) -> Dp,
    rich: @Composable (String) -> Rich,
) {
    val density = LocalDensity.current
    Row(Modifier.padding(start = em((item.ind / 2) * 1.5f))) {
        when {
            item.task -> {
                val tenth = with(density) { em(0.1f).roundToPx() }
                val vpad = 4.dp
                Box(
                    Modifier
                        .alignBy { m -> m.measuredHeight - tenth - with(density) { vpad.roundToPx() } }
                        .tap(onClick = { vm.toggleTask(item.line) }, pressed = Color.Transparent)
                        .padding(top = vpad, bottom = vpad, end = em(0.65f))
                ) {
                    val box = em(0.85f)
                    Box(
                        Modifier
                            .size(box)
                            .then(
                                if (item.done) Modifier.background(c.acc, RoundedCornerShape(3.dp))
                                else Modifier.border(1.5.dp, c.mark, RoundedCornerShape(3.dp))
                            ),
                        contentAlignment = Alignment.Center,
                    ) {
                        if (item.done) QIcon(QIcons.CHECK, 10f, 9.dp, 1.8f, c.onAcc)
                    }
                }
            }
            item.mk.any { ch -> ch.isDigit() } -> Box(Modifier.alignByBaseline().padding(end = em(0.65f))) {
                BasicText(item.mk, Modifier.widthIn(min = em(1.3f)), style = TextStyle(fontFamily = mono, fontSize = root.fontSize * 0.78f, color = c.mute))
            }
            else -> Box(Modifier.alignByBaseline().padding(end = em(0.65f))) {
                BasicText("•", Modifier.width(em(1f)), style = root.copy(color = c.acc, textAlign = androidx.compose.ui.text.style.TextAlign.Center))
            }
        }
        val st = if (item.done) root.copy(color = c.mute, textDecoration = androidx.compose.ui.text.style.TextDecoration.LineThrough) else root
        RichText(rich(item.text), st, Modifier.weight(1f).alignByBaseline())
    }
}
