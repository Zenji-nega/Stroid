package dev.stroid.app.ui

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.*
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import dev.stroid.app.Bootstrap
import dev.stroid.app.input.InputTarget
import java.io.File

private const val SAMPLE_HTML = """<!doctype html>
<html>
<head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<title>Hello Stroid</title>
<style>body{font-family:sans-serif;background:#16181d;color:#d4d8de;text-align:center;padding-top:30vh}h1{color:#7dd3c0}</style>
</head>
<body><h1>Hello, Stroid</h1><p>Edit me and press Run.</p></body>
</html>
"""

@Composable
fun StroidApp() {
    val context = LocalContext.current
    val settings = remember { Settings(context) }
    val boot = remember { Bootstrap(context).also { it.ensureLayout() } }
    val terminal = remember { TerminalState(boot) }
    val kb = remember { KeyboardState() }
    val field = remember { FieldState() }

    var leftHanded by remember { mutableStateOf(settings.leftHanded) }
    var tab by remember { mutableStateOf(Tab.CODE) }
    var kbVisible by remember { mutableStateOf(false) }
    var fieldFocused by remember { mutableStateOf(false) }
    var preview by remember { mutableStateOf<File?>(null) }
    var info by remember { mutableStateOf("Stroid") }

    var editor by remember {
        mutableStateOf<EditorState?>(
            runCatching {
                var path = settings.lastFile?.takeIf { File(it).isFile }
                if (path == null) {
                    val f = File(boot.home, "hello.html")
                    if (!f.exists()) f.writeText(SAMPLE_HTML)
                    path = f.absolutePath
                }
                EditorState.open(path)
            }.getOrNull(),
        )
    }

    fun openFile(f: File) {
        runCatching {
            editor?.close()
            editor = EditorState.open(f.absolutePath)
            settings.lastFile = f.absolutePath
            tab = Tab.CODE
            fieldFocused = false
            kbVisible = false
        }.onFailure { info = "Cannot open ${f.name}" }
    }

    DisposableEffect(Unit) {
        onDispose {
            editor?.close()
            terminal.close()
        }
    }

    val target: InputTarget? = when (tab) {
        Tab.CODE -> editor
        Tab.TERM -> terminal
        Tab.FILES -> if (fieldFocused) field else null
        else -> null
    }
    val showKeyboard = kbVisible && target != null

    BackHandler(enabled = preview != null || kbVisible) {
        if (preview != null) preview = null else kbVisible = false
    }

    val p = preview
    if (p != null) {
        PreviewScreen(p, leftHanded) { preview = null }
        return
    }

    Column(Modifier.fillMaxSize().background(C.bg)) {
        Box(Modifier.weight(1f).fillMaxWidth()) {
            when (tab) {
                Tab.CODE -> {
                    val e = editor
                    if (e != null) {
                        EditorScreen(e, leftHanded) { kbVisible = true }
                    } else {
                        SoonScreen("No file open", "Open one from Files")
                    }
                }
                Tab.TERM -> TerminalScreen(terminal) { kbVisible = true }
                Tab.FILES -> FilesScreen(
                    root = boot.home,
                    leftHanded = leftHanded,
                    field = field,
                    fieldFocused = fieldFocused,
                    onFocusField = { fieldFocused = true; kbVisible = true },
                    onOpen = { openFile(it) },
                    onToggleHand = {
                        leftHanded = !leftHanded
                        settings.leftHanded = leftHanded
                    },
                )
                Tab.GIT -> SoonScreen("Git", "Coming in Phase 3")
                Tab.AGENT -> SoonScreen("Agent", "Coming in Phase 5")
            }
        }
        if (showKeyboard) {
            val version = (editor?.version ?: 0) + terminal.tick + field.text.length
            ShortcutRow(target, leftHanded, version)
            StroidKeyboard(kb, target, leftHanded)
        }
        Dock(
            current = tab,
            leftHanded = leftHanded,
            info = info,
            onTab = { t ->
                if (t == tab) {
                    kbVisible = !kbVisible
                } else {
                    tab = t
                    fieldFocused = false
                    kbVisible = t == Tab.CODE || t == Tab.TERM
                }
            },
            onRun = {
                val e = editor
                val path = e?.path
                if (e == null || path == null) {
                    info = "Nothing to run"
                } else {
                    e.save()
                    val ext = path.substringAfterLast('.', "").lowercase()
                    when (ext) {
                        "html", "htm" -> preview = File(path)
                        "sh" -> {
                            terminal.run("sh '$path'")
                            tab = Tab.TERM
                            kbVisible = false
                        }
                        else -> info = "Run supports .html and .sh"
                    }
                }
            },
        )
    }
}
