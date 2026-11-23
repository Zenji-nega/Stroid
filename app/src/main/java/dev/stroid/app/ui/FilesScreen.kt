package dev.stroid.app.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Text
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import dev.stroid.app.input.*
import java.io.File

/** A single-line text field driven by the in-app keyboard. */
class FieldState(var onSubmit: () -> Unit = {}) : InputTarget {
    var text by mutableStateOf("")
    override val kind = TargetKind.FIELD

    override fun onKey(action: KeyAction) {
        when (action) {
            is KeyAction.Text -> text += action.text
            KeyAction.Backspace -> if (text.isNotEmpty()) {
                text = text.substring(0, text.offsetByCodePoints(text.length, -1))
            }
            KeyAction.Enter -> onSubmit()
            else -> {}
        }
    }
}

@Composable
fun FilesScreen(
    root: File,
    leftHanded: Boolean,
    field: FieldState,
    fieldFocused: Boolean,
    onFocusField: () -> Unit,
    onOpen: (File) -> Unit,
    onToggleHand: () -> Unit,
) {
    var dir by remember { mutableStateOf(root) }
    var tick by remember { mutableIntStateOf(0) }
    val entries = remember(dir, tick) {
        (dir.listFiles()?.toList() ?: emptyList())
            .sortedWith(compareBy({ !it.isDirectory }, { it.name.lowercase() }))
    }

    fun create() {
        val name = field.text.trim()
        if (name.isEmpty() || name.contains('/')) return
        val f = File(dir, name)
        runCatching {
            if (name.endsWith("/")) f.mkdirs() else f.createNewFile()
        }
        field.text = ""
        tick++
        if (f.isFile) onOpen(f)
    }
    field.onSubmit = { create() }

    Column(Modifier.fillMaxSize().background(C.bg)) {
        Row(
            Modifier.fillMaxWidth().height(28.dp).padding(horizontal = 16.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Text(
                "~" + dir.absolutePath.removePrefix(root.absolutePath),
                color = C.muted, fontSize = 12.sp, maxLines = 1,
            )
        }
        // Bottom-anchored list: the first entries sit nearest the thumb.
        LazyColumn(
            Modifier.weight(1f).fillMaxWidth(),
            reverseLayout = true,
            contentPadding = PaddingValues(horizontal = 8.dp),
        ) {
            items(entries, key = { it.absolutePath }) { f ->
                Row(
                    Modifier
                        .fillMaxWidth()
                        .height(48.dp)
                        .clip(RoundedCornerShape(10.dp))
                        .clickable { if (f.isDirectory) dir = f else onOpen(f) }
                        .padding(horizontal = 12.dp),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    SvgIcon(if (f.isDirectory) Ic.FILES else Ic.CODE, if (f.isDirectory) C.mint else C.muted, 20.dp)
                    Spacer(Modifier.width(12.dp))
                    Text(f.name + if (f.isDirectory) "/" else "", color = C.text, fontSize = 15.sp, maxLines = 1)
                }
            }
            if (dir != root) {
                item {
                    Row(
                        Modifier
                            .fillMaxWidth()
                            .height(48.dp)
                            .clip(RoundedCornerShape(10.dp))
                            .clickable { dir = dir.parentFile ?: root }
                            .padding(horizontal = 12.dp),
                        verticalAlignment = Alignment.CenterVertically,
                    ) { Text("..", color = C.muted, fontSize = 15.sp) }
                }
            }
        }
        // New file / folder (end the name with / for a folder)
        Row(
            Modifier.fillMaxWidth().height(52.dp).padding(horizontal = 8.dp, vertical = 4.dp),
            horizontalArrangement = Arrangement.spacedBy(6.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            val plus = @Composable {
                Box(
                    Modifier
                        .width(52.dp).fillMaxHeight()
                        .clip(RoundedCornerShape(10.dp))
                        .background(C.mint)
                        .clickable { create() },
                    contentAlignment = Alignment.Center,
                ) { Text("+", color = C.onMint, fontSize = 22.sp) }
            }
            val box = @Composable {
                Box(
                    Modifier
                        .weight(1f).fillMaxHeight()
                        .clip(RoundedCornerShape(10.dp))
                        .background(C.key)
                        .clickable { onFocusField() }
                        .padding(horizontal = 12.dp),
                    contentAlignment = Alignment.CenterStart,
                ) {
                    val shown = if (field.text.isEmpty() && !fieldFocused) "new file name" else field.text + if (fieldFocused) "▏" else ""
                    Text(shown, color = if (field.text.isEmpty()) C.muted else C.text, fontSize = 15.sp, maxLines = 1)
                }
            }
            if (leftHanded) { plus(); box() } else { box(); plus() }
        }
        Row(
            Modifier
                .fillMaxWidth()
                .height(52.dp)
                .clickable { onToggleHand() }
                .padding(horizontal = 16.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Text("Left-hand mode", color = C.text, fontSize = 15.sp, modifier = Modifier.weight(1f))
            Text(if (leftHanded) "ON" else "OFF", color = if (leftHanded) C.mint else C.muted, fontSize = 14.sp)
        }
    }
}
