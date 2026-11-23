package dev.stroid.app.ui

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.gestures.Orientation
import androidx.compose.foundation.gestures.detectDragGestures
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.gestures.rememberScrollableState
import androidx.compose.foundation.gestures.scrollable
import androidx.compose.foundation.layout.*
import androidx.compose.material3.Text
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clipToBounds
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.layout.onSizeChanged
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.drawText
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.rememberTextMeasurer
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import dev.stroid.app.EditorBuffer
import dev.stroid.app.Movement
import dev.stroid.app.editor.Highlighter
import dev.stroid.app.editor.Language
import dev.stroid.app.editor.Token
import dev.stroid.app.editor.WordCompleter
import dev.stroid.app.input.*
import java.io.File
import kotlin.math.abs
import kotlin.math.ceil
import kotlin.math.max

/** One open document and its view state. Receives keystrokes as an [InputTarget]. */
class EditorState(val buffer: EditorBuffer, val path: String?) : InputTarget {
    var version by mutableIntStateOf(0)
        private set
    var scrollY by mutableFloatStateOf(0f)
    var scrollX by mutableFloatStateOf(0f)
    var viewportW = 0
    var viewportH = 0
    var followCaret = false
    val language: Language = Language.forPath(path)
    val title: String = path?.substringAfterLast('/') ?: "untitled"

    override val kind = TargetKind.CODE

    fun bump(follow: Boolean = true) {
        if (follow) followCaret = true
        version++
    }

    fun caret(): Pair<Int, Int> {
        val head = buffer.selections().firstOrNull()?.head ?: 0
        return buffer.lineColOf(head)
    }

    override val hasSelection: Boolean
        get() = buffer.selections().any { !it.isEmpty }

    private fun codePoints(s: String) = s.codePointCount(0, s.length)

    override fun onKey(action: KeyAction) {
        when (action) {
            is KeyAction.Text -> buffer.insert(action.text)
            is KeyAction.Wrap -> {
                val sel = buffer.selectedText()
                if (sel.isNotEmpty()) {
                    buffer.insert(action.open + sel + action.close)
                } else {
                    buffer.insert(action.open + action.close)
                    repeat(codePoints(action.close)) { buffer.move(Movement.LEFT) }
                }
            }
            is KeyAction.Arrow -> when (action.dir) {
                Dir.LEFT -> buffer.move(Movement.LEFT)
                Dir.RIGHT -> buffer.move(Movement.RIGHT)
                Dir.UP -> buffer.moveVertical(-1)
                Dir.DOWN -> buffer.moveVertical(1)
            }
            is KeyAction.Cursor -> {
                if (action.dy != 0) buffer.moveVertical(action.dy)
                val m = if (action.dx < 0) Movement.LEFT else Movement.RIGHT
                repeat(minOf(abs(action.dx), 40)) { buffer.move(m) }
            }
            KeyAction.Backspace -> buffer.deleteBackward()
            KeyAction.Enter -> buffer.insertNewline()
            KeyAction.Tab -> buffer.insert("    ")
            KeyAction.Escape -> {
                val head = buffer.selections().firstOrNull()?.head ?: 0
                buffer.setCursor(head)
            }
            KeyAction.Undo -> buffer.undo()
            KeyAction.Redo -> buffer.redo()
            KeyAction.SelectAll -> buffer.selectAll()
            else -> {}
        }
        bump()
    }

    private var cachedVersion = -1
    private var cachedSuggestions: List<String> = emptyList()
    private var cachedPrefix = ""

    private fun prefix(): String {
        val (line, col) = caret()
        val text = buffer.lines(line, 1).firstOrNull() ?: return ""
        val u16 = text.offsetByCodePoints(0, minOf(col, codePoints(text)))
        return WordCompleter.prefixAt(text, u16)
    }

    override fun suggestions(): List<String> {
        if (cachedVersion != version) {
            cachedVersion = version
            cachedPrefix = prefix()
            cachedSuggestions =
                if (cachedPrefix.length < 2 || buffer.length > 400_000) {
                    emptyList()
                } else {
                    WordCompleter.suggest(buffer.text(), cachedPrefix)
                }
        }
        return cachedSuggestions
    }

    override fun applySuggestion(word: String) {
        val p = prefix()
        repeat(codePoints(p)) { buffer.deleteBackward() }
        buffer.insert(word)
        bump()
    }

    fun save() {
        val p = path ?: return
        if (buffer.isModified) runCatching { buffer.save(p) }
    }

    fun close() {
        save()
        buffer.close()
    }

    companion object {
        fun open(path: String): EditorState = EditorState(EditorBuffer.open(path), path)
    }
}

@Composable
fun EditorScreen(
    state: EditorState,
    leftHanded: Boolean,
    onFocus: () -> Unit,
) {
    val density = LocalDensity.current
    val measurer = rememberTextMeasurer()
    val style = remember {
        TextStyle(fontFamily = FontFamily.Monospace, fontSize = 14.sp, color = C.text)
    }
    val charW = remember(measurer) { measurer.measure("M", style).size.width.toFloat() }
    val lineH = with(density) { 24.dp.toPx() }
    val gutterPad = with(density) { 12.dp.toPx() }
    val version = state.version

    fun maxScrollY(): Float =
        max(0f, state.buffer.lineCount * lineH - state.viewportH + lineH * 2)

    fun gutterW(): Float = (state.buffer.lineCount.toString().length * charW) + gutterPad * 2

    // Keep the caret visible after edits.
    LaunchedEffect(version) {
        if (!state.followCaret || state.viewportH == 0) return@LaunchedEffect
        state.followCaret = false
        val (l, c) = state.caret()
        val top = l * lineH
        if (top < state.scrollY) state.scrollY = top
        if (top + lineH > state.scrollY + state.viewportH) {
            state.scrollY = top + lineH - state.viewportH
        }
        val cx = c * charW
        val avail = state.viewportW - gutterW() - charW * 2
        if (cx < state.scrollX) state.scrollX = max(0f, cx - charW * 4)
        if (cx > state.scrollX + avail) state.scrollX = cx - avail + charW * 4
    }

    // Autosave shortly after the last edit.
    LaunchedEffect(version) {
        kotlinx.coroutines.delay(900)
        state.save()
    }

    Column(Modifier.fillMaxSize().background(C.bg)) {
        Row(
            Modifier.fillMaxWidth().height(28.dp).padding(horizontal = 16.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Text(state.title, color = C.text, fontSize = 12.sp, fontWeight = FontWeight.Medium)
            if (state.buffer.isModified) Text("  ●", color = C.mint, fontSize = 10.sp)
            Spacer(Modifier.weight(1f))
            val (l, c) = remember(version) { state.caret() }
            Text("${l + 1}:${c + 1}", color = C.muted, fontSize = 12.sp)
        }

        Box(Modifier.weight(1f).fillMaxWidth()) {
            Canvas(
                Modifier
                    .fillMaxSize()
                    .clipToBounds()
                    .onSizeChanged {
                        state.viewportW = it.width
                        state.viewportH = it.height
                    }
                    .scrollable(
                        orientation = Orientation.Vertical,
                        state = rememberScrollableState { d ->
                            val old = state.scrollY
                            state.scrollY = (old - d).coerceIn(0f, maxScrollY())
                            old - state.scrollY
                        },
                    )
                    .scrollable(
                        orientation = Orientation.Horizontal,
                        state = rememberScrollableState { d ->
                            val old = state.scrollX
                            state.scrollX = (old - d).coerceAtLeast(0f)
                            old - state.scrollX
                        },
                    )
                    .pointerInput(state) {
                        detectTapGestures(
                            onTap = { p ->
                                onFocus()
                                val line = ((p.y + state.scrollY) / lineH).toInt()
                                    .coerceIn(0, state.buffer.lineCount - 1)
                                val col = ((p.x - gutterW() + state.scrollX) / charW + 0.5f).toInt()
                                    .coerceIn(0, state.buffer.lineLength(line))
                                state.buffer.setCursor(state.buffer.positionOf(line, col))
                                state.bump(false)
                            },
                            onDoubleTap = { p ->
                                val line = ((p.y + state.scrollY) / lineH).toInt()
                                    .coerceIn(0, state.buffer.lineCount - 1)
                                val col = ((p.x - gutterW() + state.scrollX) / charW).toInt()
                                    .coerceIn(0, state.buffer.lineLength(line))
                                state.buffer.selectWordAt(state.buffer.positionOf(line, col))
                                state.bump(false)
                            },
                        )
                    },
            ) {
                version.hashCode() // subscribe to edits
                val buf = state.buffer
                val first = (state.scrollY / lineH).toInt().coerceAtLeast(0)
                val count = ceil(size.height / lineH).toInt() + 2
                val lines = buf.lines(first, count)
                val gw = gutterW()
                val (cl, cc) = state.caret()
                val sels = buf.selections()
                val lastVisible = first + lines.size - 1

                // Current line highlight
                if (sels.all { it.isEmpty } && cl in first..lastVisible) {
                    drawRect(C.curLine, Offset(0f, cl * lineH - state.scrollY), Size(size.width, lineH))
                }
                // Selections
                for (s in sels) {
                    if (s.isEmpty) continue
                    val (sl, sc) = buf.lineColOf(s.start)
                    val (el, ec) = buf.lineColOf(s.end)
                    for (ln in max(sl, first)..minOf(el, lastVisible)) {
                        val c0 = if (ln == sl) sc else 0
                        val c1 = if (ln == el) ec else buf.lineLength(ln) + 1
                        drawRect(
                            C.selection,
                            Offset(gw + c0 * charW - state.scrollX, ln * lineH - state.scrollY),
                            Size(max(charW * 0.5f, (c1 - c0) * charW), lineH),
                        )
                    }
                }
                // Text
                for ((i, raw) in lines.withIndex()) {
                    val ln = first + i
                    val y = ln * lineH - state.scrollY
                    val line = raw.trimEnd('\n', '\r')
                    if (line.isNotEmpty()) {
                        val spans = Highlighter.highlight(line, state.language)
                        val text: AnnotatedString = buildAnnotatedString {
                            append(line)
                            for (sp in spans) {
                                addStyle(SpanStyle(color = tokenColor(sp.token)), sp.start, sp.end)
                            }
                        }
                        val layout = measurer.measure(text, style, softWrap = false)
                        drawText(
                            layout,
                            topLeft = Offset(
                                gw - state.scrollX,
                                y + (lineH - layout.size.height) / 2f,
                            ),
                        )
                    }
                }
                // Gutter (drawn over text that scrolled left)
                drawRect(C.bg, Offset.Zero, Size(gw - gutterPad * 0.5f, size.height))
                for (i in lines.indices) {
                    val ln = first + i
                    val label = (ln + 1).toString()
                    val layout = measurer.measure(
                        label,
                        style.copy(
                            fontSize = 12.sp,
                            color = if (ln == cl) C.text else C.lineNo,
                        ),
                    )
                    drawText(
                        layout,
                        topLeft = Offset(
                            gw - gutterPad - layout.size.width,
                            ln * lineH - state.scrollY + (lineH - layout.size.height) / 2f,
                        ),
                    )
                }
                // Caret
                for (s in sels) {
                    val (l, c) = buf.lineColOf(s.head)
                    if (l in first..lastVisible) {
                        val x = gw + c * charW - state.scrollX
                        if (x >= gw - 1f) {
                            drawRect(C.mint, Offset(x, l * lineH - state.scrollY + 3f), Size(2.5f, lineH - 6f))
                        }
                    }
                }
                // Scroll thumb
                val total = buf.lineCount * lineH + lineH * 2
                if (total > size.height) {
                    val th = max(36f, size.height * size.height / total)
                    val ty = (state.scrollY / (total - size.height)).coerceIn(0f, 1f) * (size.height - th)
                    val tx = if (leftHanded) 2f else size.width - 6f
                    drawRect(Color(0x44FFFFFF), Offset(tx, ty), Size(4f, th))
                }
            }

            // Edge scrub bar: drag along the thumb edge to jump through the file.
            Box(
                Modifier
                    .align(if (leftHanded) Alignment.CenterStart else Alignment.CenterEnd)
                    .width(26.dp)
                    .fillMaxHeight()
                    .pointerInput(state) {
                        detectDragGestures { change, _ ->
                            change.consume()
                            val f = (change.position.y / size.height).coerceIn(0f, 1f)
                            state.scrollY = f * maxScrollY()
                        }
                    },
            )
        }
    }
}

private fun tokenColor(t: Token): Color = when (t) {
    Token.KEYWORD -> C.keyword
    Token.STRING -> C.string
    Token.COMMENT -> C.comment
    Token.NUMBER -> C.number
    Token.FUNCTION -> C.fn
    Token.TYPE -> C.type
}
