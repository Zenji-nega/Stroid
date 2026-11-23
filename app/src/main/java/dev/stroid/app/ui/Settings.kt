package dev.stroid.app.ui

import android.content.Context

/** Small persisted preferences. */
class Settings(context: Context) {
    private val prefs = context.getSharedPreferences("stroid", Context.MODE_PRIVATE)

    var leftHanded: Boolean
        get() = prefs.getBoolean("leftHanded", false)
        set(v) = prefs.edit().putBoolean("leftHanded", v).apply()

    var lastFile: String?
        get() = prefs.getString("lastFile", null)
        set(v) = prefs.edit().putString("lastFile", v).apply()
}
