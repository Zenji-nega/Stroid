package dev.stroid.app

/**
 * Raw JNI declarations for terminal sessions (core/crates/stroid-terminal).
 * Use [TerminalSession] instead of calling these directly.
 */
internal object NativeTerminal {
    init {
        System.loadLibrary("stroid_jni")
    }

    /** Value returned by [exitCode] while the process is still running. */
    const val STILL_RUNNING: Int = Int.MIN_VALUE

    @JvmStatic external fun create(
        program: String,
        args: Array<String>,
        environment: Array<String>,
        cwd: String?,
        rows: Int,
        cols: Int,
    ): Long

    @JvmStatic external fun destroy(handle: Long)
    @JvmStatic external fun write(handle: Long, bytes: ByteArray)
    @JvmStatic external fun sendKey(handle: Long, code: Int, modifiers: Int)
    @JvmStatic external fun sendChar(handle: Long, codePoint: Int, modifiers: Int)
    @JvmStatic external fun paste(handle: Long, text: String)
    @JvmStatic external fun resize(handle: Long, rows: Int, cols: Int)
    @JvmStatic external fun generation(handle: Long): Long
    @JvmStatic external fun isClosed(handle: Long): Boolean
    @JvmStatic external fun exitCode(handle: Long): Int
    @JvmStatic external fun info(handle: Long): IntArray
    @JvmStatic external fun snapshot(handle: Long, scrollOffset: Int): IntArray
    @JvmStatic external fun title(handle: Long): String
}
