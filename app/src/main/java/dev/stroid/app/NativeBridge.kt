package dev.stroid.app

/**
 * Kotlin side of the JNI bridge to the Rust core (core/crates/stroid-jni).
 * Function names must match the exported symbols
 * `Java_dev_stroid_app_NativeBridge_*`.
 */
object NativeBridge {
    init {
        System.loadLibrary("stroid_jni")
    }

    @JvmStatic
    private external fun nativeVersion(): String?

    @JvmStatic
    private external fun nativeGreet(name: String): String?

    /** Version of the native core. */
    fun version(): String = nativeVersion() ?: "unknown"

    /** Round-trips a string through Rust to verify the bridge works. */
    fun greet(name: String): String = nativeGreet(name) ?: ""
}
