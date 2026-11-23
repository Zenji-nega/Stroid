package dev.stroid.app.input

enum class Dir { UP, DOWN, LEFT, RIGHT }

/** What a key on the in-app keyboard asks the focused screen to do. */
sealed interface KeyAction {
    data class Text(val text: String) : KeyAction

    /** Inserts [open] + [close] with the caret between them (wraps a selection). */
    data class Wrap(val open: String, val close: String) : KeyAction

    data class Arrow(val dir: Dir) : KeyAction

    /** Cursor-pad drag: [dx] columns and [dy] lines (negative = left / up). */
    data class Cursor(val dx: Int, val dy: Int) : KeyAction

    data object Backspace : KeyAction
    data object Enter : KeyAction
    data object Tab : KeyAction
    data object Escape : KeyAction
    data object Undo : KeyAction
    data object Redo : KeyAction
    data object Copy : KeyAction
    data object Cut : KeyAction
    data object Paste : KeyAction
    data object SelectAll : KeyAction
    data object ToggleComment : KeyAction
}

enum class TargetKind { CODE, TERMINAL, FIELD }

/** Something that receives keystrokes from the in-app keyboard. */
interface InputTarget {
    val kind: TargetKind

    fun onKey(action: KeyAction)

    /** Completion candidates for the word at the caret, best first. */
    fun suggestions(): List<String> = emptyList()

    fun applySuggestion(word: String) {}

    val hasSelection: Boolean get() = false

    /** Terminal only: the sticky Ctrl modifier is armed. */
    val ctrlActive: Boolean get() = false

    fun toggleCtrl() {}
}
