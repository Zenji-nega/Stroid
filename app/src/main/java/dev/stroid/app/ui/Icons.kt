package dev.stroid.app.ui

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.layout.size
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.StrokeJoin
import androidx.compose.ui.graphics.drawscope.Fill
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.drawscope.scale
import androidx.compose.ui.graphics.vector.PathParser
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember

object Ic {
    const val CODE = "M8 8l-5 4 5 4M16 8l5 4-5 4"
    const val TERM = "M4 6l6 6-6 6M12 18h8"
    const val GIT = "M4 5a2 2 0 1 0 4 0a2 2 0 1 0-4 0M4 19a2 2 0 1 0 4 0a2 2 0 1 0-4 0M16 8a2 2 0 1 0 4 0a2 2 0 1 0-4 0M6 7v10M18 10c0 5-8 3-12 8"
    const val FILES = "M14 3H7a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2V8zM14 3v5h5"
    const val AGENT = "M12 3l1.8 5.2L19 10l-5.2 1.8L12 17l-1.8-5.2L5 10l5.2-1.8z"
    const val PLAY = "M8 5.5v13l11-6.5z"
    const val PAD = "M12 3v18M3 12h18M9 6l3-3 3 3M9 18l3 3 3-3M6 9l-3 3 3 3M18 9l3 3-3 3"
    const val SHIFT = "M12 5l-7 8h4v6h6v-6h4z"
    const val BACKSPACE = "M20 6H9l-6 6 6 6h11a1 1 0 0 0 1-1V7a1 1 0 0 0-1-1zM12 10l4 4M16 10l-4 4"
    const val ENTER = "M19 7v5a2 2 0 0 1-2 2H6M10 10l-4 4 4 4"
}

/** Draws one of the design's 24x24 SVG path icons. */
@Composable
fun SvgIcon(path: String, tint: Color, size: Dp = 22.dp, filled: Boolean = false) {
    val p = remember(path) { PathParser().parsePathString(path).toPath() }
    Canvas(Modifier.size(size)) {
        val s = this.size.width / 24f
        scale(s, s, pivot = Offset.Zero) {
            if (filled) {
                drawPath(p, tint, style = Fill)
            } else {
                drawPath(
                    p, tint,
                    style = Stroke(width = 1.8f, cap = StrokeCap.Round, join = StrokeJoin.Round),
                )
            }
        }
    }
}
