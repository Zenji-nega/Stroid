package dev.stroid.app.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.platform.LocalLayoutDirection
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.LayoutDirection
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.runtime.CompositionLocalProvider

enum class Tab(val label: String, val icon: String) {
    CODE("Code", Ic.CODE),
    TERM("Term", Ic.TERM),
    GIT("Git", Ic.GIT),
    FILES("Files", Ic.FILES),
    AGENT("Agent", Ic.AGENT),
}

/** Bottom dock. In left-handed mode the whole row is mirrored. */
@Composable
fun Dock(
    current: Tab,
    leftHanded: Boolean,
    info: String,
    onTab: (Tab) -> Unit,
    onRun: () -> Unit,
) {
    val dir = if (leftHanded) LayoutDirection.Rtl else LayoutDirection.Ltr
    CompositionLocalProvider(LocalLayoutDirection provides dir) {
        Row(
            Modifier
                .fillMaxWidth()
                .height(68.dp)
                .background(C.panel)
                .padding(start = 12.dp, end = 12.dp, top = 4.dp, bottom = 8.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Text(
                info,
                color = C.muted,
                fontSize = 12.sp,
                maxLines = 1,
                modifier = Modifier.weight(1f).padding(end = 4.dp),
            )
            for (t in Tab.entries) {
                val on = t == current
                Column(
                    Modifier
                        .width(52.dp)
                        .height(52.dp)
                        .clip(RoundedCornerShape(14.dp))
                        .background(if (on) C.active else C.panel)
                        .clickable { onTab(t) },
                    horizontalAlignment = Alignment.CenterHorizontally,
                    verticalArrangement = Arrangement.Center,
                ) {
                    SvgIcon(t.icon, if (on) C.mint else C.muted)
                    Text(
                        t.label,
                        color = if (on) C.mint else C.muted,
                        fontSize = 11.sp,
                        fontWeight = FontWeight.Medium,
                    )
                }
            }
            Box(
                Modifier
                    .padding(start = 8.dp)
                    .width(56.dp)
                    .height(52.dp)
                    .clip(RoundedCornerShape(16.dp))
                    .background(C.mint)
                    .clickable { onRun() },
                contentAlignment = Alignment.Center,
            ) { SvgIcon(Ic.PLAY, C.onMint, filled = true) }
        }
    }
}
