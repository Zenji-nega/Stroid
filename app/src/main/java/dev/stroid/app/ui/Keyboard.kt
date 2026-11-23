package dev.stroid.app.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.gestures.detectDragGestures
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Text
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.LocalLayoutDirection
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.LayoutDirection
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import dev.stroid.app.input.*
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch

/** Shared state of the in-app keyboard. */
class KeyboardState {
    var lang by mutableStateOf(Lang.EN)
    var layer by mutableStateOf(Layer.LETTERS)
    var shift by mutableStateOf(false)
}

@Composable
private fun KeyCap(
    modifier: Modifier,
    bg: Color,
    repeat: Boolean = false,
    onKey: () -> Unit,
    content: @Composable () -> Unit,
) {
    val current by rememberUpdatedState(onKey)
    val scope = rememberCoroutineScope()
    Box(
        modifier
            .fillMaxHeight()
            .clip(RoundedCornerShape(10.dp))
            .background(bg)
            .pointerInput(repeat) {
                detectTapGestures(
                    onPress = {
                        current()
                        val job = if (repeat) {
                            scope.launch {
                                delay(380)
                                while (true) {
                                    current()
                                    delay(45)
                                }
                            }
                        } else {
                            null
                        }
                        tryAwaitRelease()
                        job?.cancel()
                    },
                )
            },
        contentAlignment = Alignment.Center,
    ) { content() }
}

@Composable
fun StroidKeyboard(state: KeyboardState, target: InputTarget?, leftHanded: Boolean) {
    // Letters keep their physical order in both modes.
    CompositionLocalProvider(LocalLayoutDirection provides LayoutDirection.Ltr) {
        val rows = Layouts.rows(state.lang, state.layer, state.shift, leftHanded)
        Column(
            Modifier
                .fillMaxWidth()
                .height(206.dp)
                .background(C.panel)
                .padding(horizontal = 4.dp, vertical = 3.dp),
            verticalArrangement = Arrangement.spacedBy(6.dp),
        ) {
            for (row in rows) {
                Row(
                    Modifier.weight(1f).fillMaxWidth(),
                    horizontalArrangement = Arrangement.spacedBy(4.dp),
                ) {
                    for (k in row) KeyView(k, state, target)
                }
            }
        }
    }
}

@Composable
private fun RowScope.KeyView(k: Key, state: KeyboardState, target: InputTarget?) {
    val m = Modifier.weight(k.weight)
    fun send(a: KeyAction) {
        target?.onKey(a)
    }
    when (k) {
        is Key.Glyph -> KeyCap(m, C.key, onKey = {
            send(KeyAction.Text(k.text))
            if (state.shift) state.shift = false
        }) { Text(k.text, color = C.text, fontSize = 18.sp) }
        is Key.Shift -> KeyCap(m, if (state.shift) C.mint else C.keyAlt, onKey = { state.shift = !state.shift }) {
            SvgIcon(Ic.SHIFT, if (state.shift) C.onMint else C.muted, 20.dp)
        }
        is Key.Backspace -> KeyCap(m, C.keyAlt, repeat = true, onKey = { send(KeyAction.Backspace) }) {
            SvgIcon(Ic.BACKSPACE, C.text, 22.dp)
        }
        is Key.Enter -> KeyCap(m, C.mint, onKey = { send(KeyAction.Enter) }) {
            SvgIcon(Ic.ENTER, C.onMint, 22.dp)
        }
        is Key.Space -> KeyCap(m, C.key, onKey = { send(KeyAction.Text(" ")) }) {
            Text(state.lang.code, color = C.muted, fontSize = 12.sp)
        }
        is Key.LayerSwitch -> KeyCap(m, C.keyAlt, onKey = { state.layer = k.target }) {
            Text(k.label, color = C.muted, fontSize = 13.sp, fontWeight = FontWeight.Medium)
        }
        is Key.LangSwitch -> KeyCap(m, C.keyAlt, onKey = {
            state.lang = state.lang.next()
            state.layer = Layer.LETTERS
        }) { Text(state.lang.next().code, color = C.muted, fontSize = 13.sp, fontWeight = FontWeight.Medium) }
    }
}

private val codeSymbols = listOf(
    "Tab" to KeyAction.Tab,
    ";" to KeyAction.Text(";"),
    ":" to KeyAction.Text(":"),
    "[ ]" to KeyAction.Wrap("[", "]"),
    "\" \"" to KeyAction.Wrap("\"", "\""),
    "{ }" to KeyAction.Wrap("{", "}"),
    "( )" to KeyAction.Wrap("(", ")"),
)

private val termSymbols = listOf(
    "Esc" to KeyAction.Escape,
    "Tab" to KeyAction.Tab,
    "/" to KeyAction.Text("/"),
    "|" to KeyAction.Text("|"),
    "↑" to KeyAction.Arrow(Dir.UP),
    "↓" to KeyAction.Arrow(Dir.DOWN),
)

/** The 48dp row above the keyboard: completions or symbols, plus the cursor pad. */
@Composable
fun ShortcutRow(target: InputTarget?, leftHanded: Boolean, version: Int) {
    val dir = if (leftHanded) LayoutDirection.Rtl else LayoutDirection.Ltr
    val suggestions = remember(version, target) { target?.suggestions().orEmpty() }
    val density = LocalDensity.current
    CompositionLocalProvider(LocalLayoutDirection provides dir) {
        Row(
            Modifier
                .fillMaxWidth()
                .height(48.dp)
                .background(C.bg)
                .padding(horizontal = 6.dp, vertical = 5.dp),
            horizontalArrangement = Arrangement.spacedBy(5.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            // Cursor pad sits on the thumb side (first in the row).
            Box(
                Modifier
                    .width(52.dp)
                    .fillMaxHeight()
                    .clip(RoundedCornerShape(10.dp))
                    .background(C.mint)
                    .pointerInput(target) {
                        var ax = 0f
                        var ay = 0f
                        val stepX = with(density) { 12.dp.toPx() }
                        val stepY = with(density) { 22.dp.toPx() }
                        detectDragGestures { change, drag ->
                            change.consume()
                            ax += drag.x
                            ay += drag.y
                            val dx = (ax / stepX).toInt()
                            val dy = (ay / stepY).toInt()
                            if (dx != 0 || dy != 0) {
                                ax -= dx * stepX
                                ay -= dy * stepY
                                target?.onKey(KeyAction.Cursor(dx, dy))
                            }
                        }
                    },
                contentAlignment = Alignment.Center,
            ) { SvgIcon(Ic.PAD, C.onMint, 22.dp) }

            CompositionLocalProvider(LocalLayoutDirection provides LayoutDirection.Ltr) {
                val term = target?.kind == TargetKind.TERMINAL
                if (term) {
                    Chip("Ctrl", target?.ctrlActive == true, Modifier.weight(1f)) { target?.toggleCtrl() }
                }
                if (!term && suggestions.isNotEmpty()) {
                    // Best suggestion nearest the thumb: first item is closest to the pad.
                    for (s in suggestions) {
                        Chip(s, false, Modifier.weight(1f)) { target?.applySuggestion(s) }
                    }
                } else {
                    val items = if (term) termSymbols else codeSymbols
                    for ((label, action) in items) {
                        Chip(label, false, Modifier.weight(1f)) { target?.onKey(action) }
                    }
                }
            }
        }
    }
}

@Composable
private fun Chip(label: String, on: Boolean, modifier: Modifier, onClick: () -> Unit) {
    val current by rememberUpdatedState(onClick)
    Box(
        modifier
            .fillMaxHeight()
            .clip(RoundedCornerShape(10.dp))
            .background(if (on) C.mint else C.key)
            .pointerInput(Unit) { detectTapGestures(onPress = { current(); tryAwaitRelease() }) },
        contentAlignment = Alignment.Center,
    ) {
        Text(
            label,
            color = if (on) C.onMint else C.text,
            fontSize = 14.sp,
            maxLines = 1,
        )
    }
}
