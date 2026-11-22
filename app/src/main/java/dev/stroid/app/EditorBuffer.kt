package dev.stroid.app

import java.io.IOException

/** A selection range in code point offsets. `anchor == head` is a caret. */
data class Selection(val anchor: Int, val head: Int) {
    val start: Int get() = minOf(anchor, head)
    val end: Int get() = maxOf(anchor, head)
    val isEmpty: Boolean get() = anchor == head
}

data class SearchOptions(
    val caseSensitive: Boolean = false,
    val wholeWord: Boolean = false,
    val regex: Boolean = false,
)

enum class Movement(internal val code: Int) {
    LEFT(0),
    RIGHT(1),
    LINE_START(3),
    LINE_END(4),
    WORD_LEFT(5),
    WORD_RIGHT(6),
    DOC_START(7),
    DOC_END(8),
}

/**
 * A text document backed by the Rust editor engine: multiple selections,
 * undo/redo and search. The buffer owns native memory; call [close] when done.
 * All methods are safe to call from any thread (calls are serialized).
 */
class EditorBuffer private constructor(handle: Long) : AutoCloseable {
    private var handle: Long = handle

    private inline fun <T> call(block: (Long) -> T): T = synchronized(this) {
        check(handle != 0L) { "EditorBuffer is closed" }
        block(handle)
    }

    override fun close() {
        synchronized(this) {
            if (handle != 0L) {
                NativeEditor.destroy(handle)
                handle = 0L
            }
        }
    }

    // ---- state
    val isModified: Boolean get() = call { NativeEditor.isModified(it) }
    val length: Int get() = call { NativeEditor.lenChars(it) }
    val lineCount: Int get() = call { NativeEditor.lineCount(it) }
    val canUndo: Boolean get() = call { NativeEditor.canUndo(it) }
    val canRedo: Boolean get() = call { NativeEditor.canRedo(it) }

    fun lineLength(line: Int): Int = call { NativeEditor.lineLen(it, line) }
    fun text(): String = call { NativeEditor.text(it) }
    fun slice(start: Int, end: Int): String = call { NativeEditor.slice(it, start, end) }
    fun selectedText(): String = call { NativeEditor.selectedText(it) }

    /** [count] lines starting at [first], without line breaks (for rendering). */
    fun lines(first: Int, count: Int): List<String> = call {
        if (count <= 0) emptyList() else NativeEditor.lines(it, first, count).split('\n')
    }

    fun selections(): List<Selection> = call {
        val flat = NativeEditor.selections(it)
        List(flat.size / 2) { i -> Selection(flat[2 * i], flat[2 * i + 1]) }
    }

    /** Returns (line, column) of a position. */
    fun lineColOf(position: Int): Pair<Int, Int> = call {
        val lc = NativeEditor.posToLineCol(it, position)
        lc[0] to lc[1]
    }

    fun positionOf(line: Int, col: Int): Int = call { NativeEditor.lineColToPos(it, line, col) }

    // ---- editing
    fun insert(text: String) = call { NativeEditor.insert(it, text) }
    fun insertNewline() = call { NativeEditor.insertNewline(it) }
    fun deleteBackward() = call { NativeEditor.deleteBackward(it) }
    fun deleteForward() = call { NativeEditor.deleteForward(it) }
    fun replaceRange(start: Int, end: Int, text: String) =
        call { NativeEditor.replaceRange(it, start, end, text) }

    fun undo(): Boolean = call { NativeEditor.undo(it) }
    fun redo(): Boolean = call { NativeEditor.redo(it) }

    // ---- selection
    fun move(movement: Movement, extend: Boolean = false) =
        call { NativeEditor.move(it, movement.code, 0, extend) }

    /** Moves by [lines] lines (negative = up); use for arrows and page up/down. */
    fun moveVertical(lines: Int, extend: Boolean = false) =
        call { NativeEditor.move(it, 2, lines, extend) }

    fun selectAll() = call { NativeEditor.selectAll(it) }
    fun setCursor(position: Int) = call { NativeEditor.setCursor(it, position) }
    fun setSelection(anchor: Int, head: Int) = call { NativeEditor.setSelection(it, anchor, head) }
    fun addCursor(position: Int) = call { NativeEditor.addCursor(it, position) }
    fun addCursorVertical(down: Boolean) = call { NativeEditor.addCursorVertical(it, down) }
    fun selectWordAt(position: Int) = call { NativeEditor.selectWordAt(it, position) }
    fun addNextOccurrence(): Boolean = call { NativeEditor.addNextOccurrence(it) }

    // ---- file
    @Throws(IOException::class)
    fun save(path: String) = call { NativeEditor.save(it, path) }

    // ---- search

    /** Matches as (start, end) pairs. Throws [IllegalArgumentException] for a bad regex. */
    fun findAll(query: String, options: SearchOptions = SearchOptions()): List<Pair<Int, Int>> =
        call {
            val flat = NativeEditor.findAll(
                it, query, options.caseSensitive, options.wholeWord, options.regex,
            )
            List(flat.size / 2) { i -> flat[2 * i] to flat[2 * i + 1] }
        }

    /** Replaces all matches as one undo step; returns how many were replaced. */
    fun replaceAll(
        query: String,
        replacement: String,
        options: SearchOptions = SearchOptions(),
    ): Int = call {
        NativeEditor.replaceAll(
            it, query, replacement, options.caseSensitive, options.wholeWord, options.regex,
        )
    }

    companion object {
        fun create(): EditorBuffer = EditorBuffer(NativeEditor.create())

        /** Opens a UTF-8 text file. Throws [IOException] if it cannot be read. */
        @Throws(IOException::class)
        fun open(path: String): EditorBuffer = EditorBuffer(NativeEditor.open(path))
    }
}
