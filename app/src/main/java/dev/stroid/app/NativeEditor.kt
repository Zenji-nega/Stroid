package dev.stroid.app

/**
 * Raw JNI declarations for the Rust editor engine (core/crates/stroid-jni).
 * Use [EditorBuffer] instead of calling these directly: it owns the handle and
 * prevents use after release.
 *
 * All positions are Unicode code point offsets, NOT UTF-16 indices. Convert
 * with String.offsetByCodePoints / codePointCount when mapping to Kotlin
 * string indices.
 */
internal object NativeEditor {
    init {
        System.loadLibrary("stroid_jni")
    }

    @JvmStatic external fun create(): Long
    @JvmStatic external fun open(path: String): Long
    @JvmStatic external fun destroy(handle: Long)
    @JvmStatic external fun save(handle: Long, path: String)

    @JvmStatic external fun isModified(handle: Long): Boolean
    @JvmStatic external fun lenChars(handle: Long): Int
    @JvmStatic external fun lineCount(handle: Long): Int
    @JvmStatic external fun lineLen(handle: Long, line: Int): Int
    @JvmStatic external fun text(handle: Long): String
    @JvmStatic external fun slice(handle: Long, start: Int, end: Int): String
    @JvmStatic external fun lines(handle: Long, first: Int, count: Int): String
    @JvmStatic external fun selectedText(handle: Long): String
    @JvmStatic external fun selections(handle: Long): IntArray
    @JvmStatic external fun posToLineCol(handle: Long, position: Int): IntArray
    @JvmStatic external fun lineColToPos(handle: Long, line: Int, col: Int): Int

    @JvmStatic external fun insert(handle: Long, text: String)
    @JvmStatic external fun insertNewline(handle: Long)
    @JvmStatic external fun deleteBackward(handle: Long)
    @JvmStatic external fun deleteForward(handle: Long)
    @JvmStatic external fun replaceRange(handle: Long, start: Int, end: Int, text: String)
    @JvmStatic external fun undo(handle: Long): Boolean
    @JvmStatic external fun redo(handle: Long): Boolean
    @JvmStatic external fun canUndo(handle: Long): Boolean
    @JvmStatic external fun canRedo(handle: Long): Boolean

    @JvmStatic external fun move(handle: Long, code: Int, arg: Int, extend: Boolean)
    @JvmStatic external fun selectAll(handle: Long)
    @JvmStatic external fun setCursor(handle: Long, position: Int)
    @JvmStatic external fun setSelection(handle: Long, anchor: Int, head: Int)
    @JvmStatic external fun addCursor(handle: Long, position: Int)
    @JvmStatic external fun addCursorVertical(handle: Long, down: Boolean)
    @JvmStatic external fun selectWordAt(handle: Long, position: Int)
    @JvmStatic external fun addNextOccurrence(handle: Long): Boolean

    @JvmStatic external fun findAll(
        handle: Long,
        query: String,
        caseSensitive: Boolean,
        wholeWord: Boolean,
        regex: Boolean,
    ): IntArray

    @JvmStatic external fun replaceAll(
        handle: Long,
        query: String,
        replacement: String,
        caseSensitive: Boolean,
        wholeWord: Boolean,
        regex: Boolean,
    ): Int
}
