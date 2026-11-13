package app.quire.notes.ui

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.collectIsPressedAsState
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicText
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.PathEffect
import androidx.compose.ui.graphics.RectangleShape
import androidx.compose.ui.graphics.Shape
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.StrokeJoin
import androidx.compose.ui.graphics.drawscope.DrawScope
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.drawscope.scale
import androidx.compose.ui.graphics.vector.PathParser
import androidx.compose.ui.text.TextLayoutResult
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.TextUnit
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlin.math.max
import kotlin.math.min

/** Stroke icons, copied from the SVG paths in the prototype. */
object QIcons {
    const val SIDEBAR_LEFT = "M4 3h8a2 2 0 0 1 2 2v6a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2zM6 3v10"
    const val SIDEBAR_RIGHT = "M4 3h8a2 2 0 0 1 2 2v6a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2zM10 3v10"
    const val PLUS = "M6 1.5v9M1.5 6h9"
    const val CLOSE = "M1 1l6 6M7 1L1 7"
    const val CHEVRON_DOWN = "M2 3.5l3 3 3-3"
    const val CHEVRON_RIGHT = "M2.5 1l3 3-3 3"
    const val FOLDER_CLOSED = "M2 4.5A1.5 1.5 0 0 1 3.5 3h2.6l1.5 1.6h4.9A1.5 1.5 0 0 1 14 6.1v5.4a1.5 1.5 0 0 1-1.5 1.5h-9A1.5 1.5 0 0 1 2 11.5z"
    const val FOLDER_OPEN = "M2 11.5v-7A1.5 1.5 0 0 1 3.5 3h2.6l1.5 1.6h3.9A1.5 1.5 0 0 1 13 6.1V7M2.3 12.3l1.5-4.1A1.5 1.5 0 0 1 5.2 7.2h8.2a.9.9 0 0 1 .85 1.2l-1.3 3.6a1.5 1.5 0 0 1-1.4 1H3.4a1.2 1.2 0 0 1-1.1-.7z"
    const val SEARCH = "M2.5 7a4.5 4.5 0 1 0 9 0a4.5 4.5 0 1 0-9 0M10.5 10.5L14 14"
    const val CHECK = "M1.5 5.2l2.4 2.4L8.5 2.6"
    /** Shown in source mode: the button switches to preview. */
    const val EYE = "M1.5 8s2.4-4.5 6.5-4.5S14.5 8 14.5 8s-2.4 4.5-6.5 4.5S1.5 8 1.5 8zM6 8a2 2 0 1 0 4 0a2 2 0 1 0-4 0"
    /** Shown in preview mode: the button switches to source. */
    const val CODE = "M5.5 4.5L2 8l3.5 3.5M10.5 4.5L14 8l-3.5 3.5"
    const val LINK = "M6.8 9.2a2.8 2.8 0 0 0 4 0l2-2a2.8 2.8 0 0 0-4-4l-.8.8M9.2 6.8a2.8 2.8 0 0 0-4 0l-2 2a2.8 2.8 0 0 0 4 4l.8-.8"
    const val LIST = "M6 4h8M6 8h8M6 12h8M2.6 4h.01M2.6 8h.01M2.6 12h.01"
    const val TASK = "M3.5 2.5h9a1 1 0 0 1 1 1v9a1 1 0 0 1-1 1h-9a1 1 0 0 1-1-1v-9a1 1 0 0 1 1-1zM5.3 8.2l1.9 1.9 3.6-4"
    const val QUOTE = "M3 4v8M6.5 5h7M6.5 8h7M6.5 11h4.5"
    /** A pin, for the Pinned group and the pin button. The path of the desktop app. */
    const val PIN = "M5.5 2.5h5M6.5 2.5v3.9L4.5 9h7l-2-2.6V2.5M8 9v4.5"
    /** A padlock, for locked notes. */
    const val LOCK = "M5 7h6a1.6 1.6 0 0 1 1.6 1.6v3.4a1.6 1.6 0 0 1-1.6 1.6H5a1.6 1.6 0 0 1-1.6-1.6V8.6A1.6 1.6 0 0 1 5 7zM5.4 7V5.2a2.6 2.6 0 0 1 5.2 0V7"
    /** Insert image: a frame with a hill and a sun. */
    const val IMAGE = "M3.5 2.5h9a1.5 1.5 0 0 1 1.5 1.5v8a1.5 1.5 0 0 1-1.5 1.5h-9A1.5 1.5 0 0 1 2 12V4a1.5 1.5 0 0 1 1.5-1.5zM2.3 11.2l3.4-3.4 2.8 2.8 1.8-1.8 3.4 3.4M9.6 5.8a1.1 1.1 0 1 0 2.2 0a1.1 1.1 0 1 0-2.2 0"
}

@Composable
fun QIcon(d: String, viewport: Float, size: Dp, stroke: Float, color: Color, modifier: Modifier = Modifier) {
    val path = remember(d) { PathParser().parsePathString(d).toPath() }
    Canvas(modifier.size(size)) {
        val s = this.size.width / viewport
        scale(s, s, pivot = Offset.Zero) {
            drawPath(path, color, style = Stroke(width = stroke, cap = StrokeCap.Round, join = StrokeJoin.Round))
        }
    }
}

/** A click target that shows the design's hover color while pressed, with no ripple. */
@OptIn(ExperimentalFoundationApi::class)
@Composable
fun Modifier.tap(
    shape: Shape = RectangleShape,
    pressed: Color = LocalQ.current.hover,
    enabled: Boolean = true,
    onLongClick: (() -> Unit)? = null,
    onClick: () -> Unit,
): Modifier {
    val src = remember { MutableInteractionSource() }
    val isPressed by src.collectIsPressedAsState()
    return this
        .clip(shape)
        .background(if (isPressed) pressed else Color.Transparent)
        .combinedClickable(interactionSource = src, indication = null, enabled = enabled, onLongClick = onLongClick, onClick = onClick)
}

@Composable
fun Txt(
    text: String,
    size: TextUnit,
    color: Color,
    modifier: Modifier = Modifier,
    weight: FontWeight = FontWeight.W400,
    family: FontFamily = QFonts.sans,
    lineHeight: TextUnit = TextUnit.Unspecified,
    italic: Boolean = false,
    maxLines: Int = Int.MAX_VALUE,
    align: TextAlign? = null,
    letterSpacing: TextUnit = TextUnit.Unspecified,
) {
    BasicText(
        text = text,
        modifier = modifier,
        style = TextStyle(
            color = color, fontSize = size, fontWeight = weight, fontFamily = family, lineHeight = lineHeight,
            fontStyle = if (italic) FontStyle.Italic else FontStyle.Normal, textAlign = align ?: TextAlign.Unspecified,
            letterSpacing = letterSpacing,
        ),
        maxLines = maxLines,
        overflow = if (maxLines == Int.MAX_VALUE) TextOverflow.Clip else TextOverflow.Ellipsis,
    )
}

@Composable
fun Mono(text: String, size: TextUnit, color: Color, modifier: Modifier = Modifier, maxLines: Int = Int.MAX_VALUE, lineHeight: TextUnit = TextUnit.Unspecified) =
    Txt(text, size, color, modifier, family = QFonts.plex, maxLines = maxLines, lineHeight = lineHeight)

/** A 28 dp icon button from the title bar of the prototype. */
@Composable
fun IconBtn(d: String, viewport: Float = 16f, iconSize: Dp = 16.dp, stroke: Float = 1.3f, size: Dp = 32.dp, color: Color = LocalQ.current.mute, onClick: () -> Unit) {
    Box(Modifier.size(size).tap(RoundedCornerShape(6.dp), onClick = onClick), contentAlignment = Alignment.Center) {
        QIcon(d, viewport, iconSize, stroke, color)
    }
}

class SegOpt(val label: String, val on: Boolean, val onClick: () -> Unit)

/** The segmented control. [small] is the source and preview switch in the title bar. */
@Composable
fun Segmented(opts: List<SegOpt>, small: Boolean = false, modifier: Modifier = Modifier) {
    val c = LocalQ.current
    Row(
        modifier
            .clip(RoundedCornerShape(if (small) 7.dp else 8.dp))
            .background(if (small) c.hover else c.side)
            .padding(2.dp),
        horizontalArrangement = Arrangement.spacedBy(2.dp),
    ) {
        opts.forEach { o ->
            val shape = RoundedCornerShape(if (small) 5.dp else 6.dp)
            Box(
                Modifier
                    .height(if (small) 24.dp else 28.dp)
                    .then(if (o.on) Modifier.shadow(1.dp, shape, ambientColor = Shadow, spotColor = Shadow).background(c.bg, shape) else Modifier)
                    .tap(shape, pressed = Color.Transparent, onClick = o.onClick)
                    .padding(horizontal = if (small) 9.dp else 12.dp),
                contentAlignment = Alignment.Center,
            ) {
                if (small) {
                    Mono(o.label, 11.sp, if (o.on) c.ink else c.mute)
                } else {
                    Txt(o.label, 12.5.sp, if (o.on) c.ink else c.mute, maxLines = 1)
                }
            }
        }
    }
}

/** The on and off switch from the settings panel. */
@Composable
fun Toggle(on: Boolean, onClick: () -> Unit) {
    val c = LocalQ.current
    Box(
        Modifier
            .size(36.dp, 20.dp)
            .clip(RoundedCornerShape(10.dp))
            .background(if (on) c.button else c.mark)
            .tap(RoundedCornerShape(10.dp), pressed = Color.Transparent, onClick = onClick)
            .padding(2.dp),
        contentAlignment = if (on) Alignment.CenterEnd else Alignment.CenterStart,
    ) {
        Box(Modifier.size(16.dp).shadow(1.dp, CircleShape).background(Color.White, CircleShape))
    }
}

@Composable
fun PrimaryButton(label: String, modifier: Modifier = Modifier, height: Dp = 32.dp, hPad: Dp = 14.dp, size: TextUnit = 13.sp, trailing: (@Composable RowScope.() -> Unit)? = null, onClick: () -> Unit) {
    val src = remember { MutableInteractionSource() }
    val pressed by src.collectIsPressedAsState()
    Row(
        modifier
            .height(height)
            .clip(RoundedCornerShape(8.dp))
            .background(if (pressed) LocalQ.current.buttonPressed else LocalQ.current.button)
            .combinedClickable(interactionSource = src, indication = null, onClick = onClick)
            .padding(horizontal = hPad),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        Txt(label, size, LocalQ.current.onAcc, weight = FontWeight.W500, maxLines = 1)
        trailing?.invoke(this)
    }
}

@Composable
fun OutlineButton(label: String, modifier: Modifier = Modifier, height: Dp = 30.dp, hPad: Dp = 12.dp, size: TextUnit = 12.5.sp, color: Color = LocalQ.current.ink, onClick: () -> Unit) {
    val c = LocalQ.current
    Box(
        modifier
            .height(height)
            .border(1.dp, c.line, RoundedCornerShape(7.dp))
            .tap(RoundedCornerShape(7.dp), onClick = onClick)
            .padding(horizontal = hPad),
        contentAlignment = Alignment.Center,
    ) {
        Txt(label, size, color, maxLines = 1)
    }
}

/** A small mono label such as "outline" or "recent". */
@Composable
fun Label(text: String, modifier: Modifier = Modifier) = Mono(text, 11.sp, LocalQ.current.mute, modifier)

@Composable
fun Hairline(modifier: Modifier = Modifier) {
    Box(modifier.height(1.dp).background(LocalQ.current.line))
}

@Composable
fun VLine(modifier: Modifier = Modifier) {
    Box(modifier.width(1.dp).background(LocalQ.current.line))
}

// ---- Text decorations ------------------------------------------------------------------------

enum class DecoKind { Underline, Box, LineBg }

/**
 * A decoration that a span style can't draw: a link underline with an offset and its own color,
 * a rounded background behind a tag or inline code, or a full-width background behind a code line.
 * [em] is the font size of the decorated text, relative to the base text style.
 */
class Deco(
    val start: Int,
    val end: Int,
    val color: Color,
    val kind: DecoKind,
    val dashed: Boolean = false,
    val radius: Dp = 0.dp,
    val em: Float = 1f,
    val padV: Float = 0f,
)

class Rich(val text: androidx.compose.ui.text.AnnotatedString, val decos: List<Deco>)

fun DrawScope.drawDecos(l: TextLayoutResult, decos: List<Deco>, basePx: Float) {
    val len = l.layoutInput.text.length
    val one = 1.dp.toPx()
    for (d in decos) {
        if (d.start > len || d.end > len || d.end < d.start) continue
        when (d.kind) {
            DecoKind.LineBg -> {
                val l0 = l.getLineForOffset(d.start)
                val l1 = l.getLineForOffset(max(d.start, d.end - 1))
                val top = l.getLineTop(l0)
                val bottom = l.getLineBottom(l1)
                drawRect(d.color, Offset(0f, top), Size(size.width, bottom - top))
            }
            else -> {
                if (d.end <= d.start) continue
                val l0 = l.getLineForOffset(d.start)
                val l1 = l.getLineForOffset(d.end - 1)
                val fs = basePx * d.em
                for (li in l0..l1) {
                    val s = max(d.start, l.getLineStart(li))
                    val e = min(d.end, l.getLineEnd(li, visibleEnd = true))
                    if (e <= s) continue
                    val x0 = l.getBoundingBox(s).left
                    val x1 = l.getBoundingBox(e - 1).right
                    val base = l.getLineBaseline(li)
                    if (d.kind == DecoKind.Underline) {
                        val y = base + 3 * one
                        drawLine(
                            d.color, Offset(x0, y), Offset(x1, y), strokeWidth = one,
                            pathEffect = if (d.dashed) PathEffect.dashPathEffect(floatArrayOf(3 * one, 2 * one)) else null,
                        )
                    } else {
                        val pad = fs * d.padV
                        val top = base - fs * 0.98f - pad
                        val bottom = base + fs * 0.3f + pad
                        val r = d.radius.toPx()
                        drawRoundRect(d.color, Offset(x0, top), Size(x1 - x0, bottom - top), CornerRadius(r, r))
                    }
                }
            }
        }
    }
}

