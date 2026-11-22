package dev.stroid.app

import android.content.Context
import java.io.File

/**
 * Layout of Stroid's private environment, modelled on Termux:
 *
 *   <filesDir>/usr   PREFIX  (bin, lib, etc, tmp, share)
 *   <filesDir>/home  HOME
 *
 * Phase 0 only creates the directory skeleton and reports its state.
 * Installing the base packages (shell, coreutils, package manager) is the
 * next step; see docs/bootstrap.md.
 */
class Bootstrap(context: Context) {
    val prefix: File = File(context.filesDir, "usr")
    val home: File = File(context.filesDir, "home")

    private val prefixDirs = listOf("bin", "lib", "etc", "tmp", "share")

    /** True when the base packages have been installed (marker file present). */
    val isInstalled: Boolean
        get() = File(prefix, ".stroid-bootstrap-complete").exists()

    /** Creates the PREFIX and HOME directory skeleton. Safe to call repeatedly. */
    fun ensureLayout(): Boolean {
        val dirs = prefixDirs.map { File(prefix, it) } + home
        return dirs.all { it.isDirectory || it.mkdirs() }
    }
}
