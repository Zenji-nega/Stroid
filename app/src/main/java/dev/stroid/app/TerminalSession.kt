package dev.stroid.app

import java.io.IOException

/** Non-character keys, matching the codes understood by the native side. */
enum class TerminalKey(internal val code: Int) {
    UP(0), DOWN(1), LEFT(2), RIGHT(3),
    HOME(4), END(5), PAGE_UP(6), PAGE_DOWN(7),
    INSERT(8), DELETE(9),
    ENTER(10), TAB(11), BACKSPACE(12), ESCAPE(13),
    F1(101), F2(102), F3(103), F4(104), F5(105), F6(106),
    F7(107), F8(108), F9(109), F10(110), F11(111), F12(112),
}

/** Modifier bit flags for [TerminalSession.sendKey] and [TerminalSession.sendChar]. */
object TerminalMods {
    const val NONE = 0
    const val SHIFT = 1
    const val ALT = 2
    const val CTRL = 4
}

/** Screen metadata. */
data class TerminalInfo(
    val rows: Int,
    val cols: Int,
    val cursorRow: Int,
    val cursorCol: Int,
    val cursorVisible: Boolean,
    /** Lines of history that can be scrolled back into. */
    val scrollbackLines: Int,
    val altScreen: Boolean,
    val appCursorKeys: Boolean,
    val bracketedPaste: Boolean,
    val bellCount: Int,
)

/**
 * The visible grid as flat ints, 4 per cell in row-major order:
 * `[codePoint, fg, bg, flags]`.
 *
 * Colors: `-1` default, `0..255` palette index, otherwise `0x01RRGGBB`.
 * Flags: 1 bold, 2 dim, 4 italic, 8 underline, 16 inverse, 32 strike,
 * 64 = second half of a double-width character (skip when drawing).
 */
class TerminalSnapshot(val rows: Int, val cols: Int, val cells: IntArray) {
    fun codePoint(row: Int, col: Int) = cells[(row * cols + col) * 4]
    fun fg(row: Int, col: Int) = cells[(row * cols + col) * 4 + 1]
    fun bg(row: Int, col: Int) = cells[(row * cols + col) * 4 + 2]
    fun flags(row: Int, col: Int) = cells[(row * cols + col) * 4 + 3]
}

/**
 * A program running in a real pseudo-terminal. Owns native resources: call
 * [close] to kill the process and release them. Calls are serialized, so any
 * thread may use it.
 */
class TerminalSession private constructor(handle: Long) : AutoCloseable {
    private var handle: Long = handle

    private inline fun <T> call(block: (Long) -> T): T = synchronized(this) {
        check(handle != 0L) { "TerminalSession is closed" }
        block(handle)
    }

    override fun close() {
        synchronized(this) {
            if (handle != 0L) {
                NativeTerminal.destroy(handle)
                handle = 0L
            }
        }
    }

    /** Sends raw bytes to the program. */
    fun write(bytes: ByteArray) = call { NativeTerminal.write(it, bytes) }

    fun sendKey(key: TerminalKey, mods: Int = TerminalMods.NONE) =
        call { NativeTerminal.sendKey(it, key.code, mods) }

    fun sendChar(codePoint: Int, mods: Int = TerminalMods.NONE) =
        call { NativeTerminal.sendChar(it, codePoint, mods) }

    /** Pastes text (bracketed if the running program enabled it). */
    fun paste(text: String) = call { NativeTerminal.paste(it, text) }

    fun resize(rows: Int, cols: Int) = call { NativeTerminal.resize(it, rows, cols) }

    /** Changes whenever the screen may have changed; poll it to decide when to redraw. */
    val generation: Long get() = call { NativeTerminal.generation(it) }

    /** True once the program has closed its output (normally: it exited). */
    val isClosed: Boolean get() = call { NativeTerminal.isClosed(it) }

    /** Exit code, or null while the process is still running. */
    val exitCode: Int?
        get() = call { NativeTerminal.exitCode(it) }.takeIf { it != NativeTerminal.STILL_RUNNING }

    val title: String get() = call { NativeTerminal.title(it) }

    fun info(): TerminalInfo = call {
        val i = NativeTerminal.info(it)
        TerminalInfo(
            rows = i[0], cols = i[1],
            cursorRow = i[2], cursorCol = i[3], cursorVisible = i[4] != 0,
            scrollbackLines = i[5], altScreen = i[6] != 0,
            appCursorKeys = i[7] != 0, bracketedPaste = i[8] != 0, bellCount = i[9],
        )
    }

    /** The viewport, scrolled back [scrollOffset] lines into history. */
    fun snapshot(scrollOffset: Int = 0): TerminalSnapshot = call {
        val info = NativeTerminal.info(it)
        TerminalSnapshot(info[0], info[1], NativeTerminal.snapshot(it, scrollOffset))
    }

    companion object {
        /**
         * Starts [program] in a new terminal. [environment] entries are
         * `KEY=VALUE`. Throws [IOException] if the process cannot be started.
         */
        @Throws(IOException::class)
        fun start(
            program: String,
            args: List<String> = emptyList(),
            environment: List<String> = emptyList(),
            cwd: String? = null,
            rows: Int = 24,
            cols: Int = 80,
        ): TerminalSession = TerminalSession(
            NativeTerminal.create(
                program, args.toTypedArray(), environment.toTypedArray(), cwd, rows, cols,
            ),
        )
    }
}
