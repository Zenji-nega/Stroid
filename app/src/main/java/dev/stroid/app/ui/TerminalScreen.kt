package dev.stroid.app.ui

import android.graphics.Paint
import android.graphics.Typeface
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.gestures.detectVerticalDragGestures
import androidx.compose.foundation.layout.*
import androidx.compose.material3.Text
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clipToBounds
import androidx.compose.ui.graphics.drawscope.drawIntoCanvas
import androidx.compose.ui.graphics.nativeCanvas
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.layout.onSizeChanged
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.unit.IntSize
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import dev.stroid.app.Bootstrap
import dev.stroid.app.TerminalKey
import dev.stroid.app.TerminalMods
import dev.stroid.app.TerminalSession
import dev.stroid.app.input.*
import kotlinx.coroutines.delay

/** A running shell. Receives keystrokes as an [InputTarget]. */
class TerminalState(private val boot: Bootstrap) : InputTarget {
    var session: TerminalSession? = null
        private set
    var error by mutableStateOf<String?>(null)
    var tick by mutableIntStateOf(0)
    var scrollOffset by mutableIntStateOf(0)
    var rows = 24
    var cols = 80
    private var ctrl by mutableStateOf(false)

    override val kind = TargetKind.TERMINAL
    override val ctrlActive: Boolean get() = ctrl

    override fun toggleCtrl() {
        ctrl = !ctrl
    }

    fun ensureStarted() {
        if (session != null && session?.isClosed == false) return
        runCatching {
            boot.ensureLayout()
            val prefix = boot.prefix.absolutePath
            val env = listOf(
                "HOME=${boot.home.absolutePath}",
                "PREFIX=$prefix",
                "TMPDIR=$prefix/tmp",
                "TERM=xterm-256color",
                "LANG=en_US.UTF-8",
                "PATH=$prefix/bin:/system/bin:/system/xbin",
                "PS1=\\w \\$ ",
            )
            session?.close()
            session = TerminalSession.start(
                "/system/bin/sh", emptyList(), env, boot.home.absolutePath, rows, cols,
            )
            error = null
        }.onFailure { error = it.message ?: "failed to start shell" }
        tick++
    }

    /** Runs a command line in the shell. */
    fun run(command: String) {
        ensureStarted()
        session?.paste(command)
        session?.sendKey(TerminalKey.ENTER)
        scrollOffset = 0
        tick++
    }

    fun resize(r: Int, c: Int) {
        if (r < 2 || c < 2 || (r == rows && c == cols && session != null)) return
        rows = r
        cols = c
        session?.resize(r, c)
        tick++
    }

    override fun onKey(action: KeyAction) {
        val s = session ?: return
        scrollOffset = 0
        when (action) {
            is KeyAction.Text -> {
                val t = action.text
                var i = 0
                while (i < t.length) {
                    val cp = t.codePointAt(i)
                    i += Character.charCount(cp)
                    if (ctrl) {
                        s.sendChar(cp, TerminalMods.CTRL)
                        ctrl = false
                    } else {
                        s.sendChar(cp, TerminalMods.NONE)
                    }
                }
            }
            is KeyAction.Wrap -> s.paste(action.open + action.close)
            is KeyAction.Arrow -> s.sendKey(
                when (action.dir) {
                    Dir.UP -> TerminalKey.UP
                    Dir.DOWN -> TerminalKey.DOWN
                    Dir.LEFT -> TerminalKey.LEFT
                    Dir.RIGHT -> TerminalKey.RIGHT
                },
            )
            is KeyAction.Cursor -> {
                val h = if (action.dx < 0) TerminalKey.LEFT else TerminalKey.RIGHT
                repeat(minOf(kotlin.math.abs(action.dx), 30)) { s.sendKey(h) }
                val v = if (action.dy < 0) TerminalKey.UP else TerminalKey.DOWN
                repeat(minOf(kotlin.math.abs(action.dy), 10)) { s.sendKey(v) }
            }
            KeyAction.Backspace -> s.sendKey(TerminalKey.BACKSPACE)
            KeyAction.Enter -> s.sendKey(TerminalKey.ENTER)
            KeyAction.Tab -> s.sendKey(TerminalKey.TAB)
            KeyAction.Escape -> s.sendKey(TerminalKey.ESCAPE)
            else -> {}
        }
        tick++
    }

    fun close() {
        session?.close()
        session = null
    }
}

private val ansi16 = intArrayOf(
    0x1B1E24, 0xE06C75, 0x98C379, 0xE5C07B, 0x61AFEF, 0xC678DD, 0x56B6C2, 0xABB2BF,
    0x5C6370, 0xFF7B86, 0xA9DC8A, 0xF0D08C, 0x7DBBFF, 0xD68AEE, 0x6FD0DC, 0xFFFFFF,
)

private fun paletteColor(i: Int): Int {
    if (i < 16) return 0xFF000000.toInt() or ansi16[i]
    if (i < 232) {
        val n = i - 16
        fun lvl(v: Int) = if (v == 0) 0 else 55 + v * 40
        return 0xFF000000.toInt() or (lvl(n / 36) shl 16) or (lvl((n / 6) % 6) shl 8) or lvl(n % 6)
    }
    val g = 8 + (i - 232) * 10
    return 0xFF000000.toInt() or (g shl 16) or (g shl 8) or g
}

private fun resolve(c: Int, default: Int): Int = when {
    c == -1 -> default
    c in 0..255 -> paletteColor(c)
    else -> 0xFF000000.toInt() or (c and 0xFFFFFF)
}

@Composable
fun TerminalScreen(state: TerminalState, onFocus: () -> Unit) {
    val density = LocalDensity.current
    val paint = remember(density) {
        Paint(Paint.ANTI_ALIAS_FLAG).apply {
            typeface = Typeface.MONOSPACE
            textSize = with(density) { 13.sp.toPx() }
        }
    }
    val bgPaint = remember { Paint() }
    val cellW = remember(paint) { paint.measureText("M") }
    val cellH = remember(paint) { paint.fontMetrics.let { it.descent - it.ascent } + 3f }
    val defFg = 0xFFD4D8DE.toInt()
    val defBg = 0xFF121418.toInt()
    var size by remember { mutableStateOf(IntSize.Zero) }

    LaunchedEffect(Unit) { state.ensureStarted() }
    LaunchedEffect(size) {
        if (size.width > 0) {
            val pad = with(density) { 16.dp.toPx() }
            state.resize(
                ((size.height - pad) / cellH).toInt(),
                ((size.width - pad) / cellW).toInt(),
            )
        }
    }
    // Poll the native side for new output.
    LaunchedEffect(Unit) {
        var last = -1L
        while (true) {
            val s = state.session
            if (s != null) {
                val g = runCatching { s.generation }.getOrDefault(last)
                if (g != last) {
                    last = g
                    state.tick++
                }
                if (s.isClosed && state.error == null) {
                    state.error = "shell exited (${s.exitCode ?: "?"}) — tap to restart"
                }
            }
            delay(33)
        }
    }

    Column(Modifier.fillMaxSize().background(C.termBg)) {
        Row(
            Modifier.fillMaxWidth().height(28.dp).padding(horizontal = 16.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Text("bash", color = C.text, fontSize = 12.sp)
            Text("  ›  ", color = C.muted, fontSize = 12.sp)
            Text(
                state.session?.title?.takeIf { it.isNotBlank() } ?: "~",
                color = C.muted,
                fontSize = 12.sp,
                maxLines = 1,
            )
        }
        Canvas(
            Modifier
                .weight(1f)
                .fillMaxWidth()
                .clipToBounds()
                .onSizeChanged { size = it }
                .pointerInput(state) {
                    detectTapGestures(onTap = {
                        onFocus()
                        if (state.session?.isClosed == true) state.ensureStarted()
                    })
                }
                .pointerInput(state) {
                    var acc = 0f
                    detectVerticalDragGestures { change, d ->
                        change.consume()
                        acc += d
                        val rows = (acc / cellH).toInt()
                        if (rows != 0) {
                            acc -= rows * cellH
                            val max = state.session?.info()?.scrollbackLines ?: 0
                            state.scrollOffset = (state.scrollOffset + rows).coerceIn(0, max)
                            state.tick++
                        }
                    }
                },
        ) {
            state.tick.hashCode()
            val s = state.session ?: return@Canvas
            val snap = runCatching { s.snapshot(state.scrollOffset) }.getOrNull() ?: return@Canvas
            val info = s.info()
            val ox = with(density) { 8.dp.toPx() }
            val oy = with(density) { 4.dp.toPx() }
            drawIntoCanvas { cv ->
                val c = cv.nativeCanvas
                val asc = -paint.fontMetrics.ascent
                for (r in 0 until snap.rows) {
                    for (col in 0 until snap.cols) {
                        val flags = snap.flags(r, col)
                        if (flags and 64 != 0) continue
                        var f = resolve(snap.fg(r, col), defFg)
                        var b = resolve(snap.bg(r, col), defBg)
                        if (flags and 16 != 0) {
                            val t = f; f = b; b = t
                        }
                        val x = ox + col * cellW
                        val y = oy + r * cellH
                        if (b != defBg) {
                            bgPaint.color = b
                            c.drawRect(x, y, x + cellW * if (flags and 128 != 0) 2 else 1, y + cellH, bgPaint)
                        }
                        val cp = snap.codePoint(r, col)
                        if (cp > 32) {
                            paint.color = f
                            paint.isFakeBoldText = flags and 1 != 0
                            c.drawText(String(Character.toChars(cp)), x, y + asc + 1.5f, paint)
                        }
                    }
                }
                if (info.cursorVisible && state.scrollOffset == 0) {
                    bgPaint.color = 0xFF7DD3C0.toInt()
                    val x = ox + info.cursorCol * cellW
                    val y = oy + info.cursorRow * cellH
                    c.drawRect(x, y, x + cellW, y + cellH, bgPaint.apply { alpha = 150 })
                    bgPaint.alpha = 255
                }
            }
        }
        state.error?.let {
            Text(it, color = C.muted, fontSize = 12.sp, modifier = Modifier.padding(horizontal = 16.dp, vertical = 4.dp))
        }
    }
}
