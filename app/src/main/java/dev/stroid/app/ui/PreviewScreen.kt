package dev.stroid.app.ui

import android.annotation.SuppressLint
import android.webkit.WebView
import android.webkit.WebViewClient
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Text
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.platform.LocalLayoutDirection
import androidx.compose.ui.unit.LayoutDirection
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.viewinterop.AndroidView
import java.io.File

/** Full-screen preview of a local HTML file. */
@SuppressLint("SetJavaScriptEnabled")
@Composable
fun PreviewScreen(file: File, leftHanded: Boolean, onBack: () -> Unit) {
    var web by remember { mutableStateOf<WebView?>(null) }
    val dir = if (leftHanded) LayoutDirection.Rtl else LayoutDirection.Ltr
    Column(Modifier.fillMaxSize().background(C.bg)) {
        AndroidView(
            modifier = Modifier.weight(1f).fillMaxWidth(),
            factory = { ctx ->
                WebView(ctx).apply {
                    settings.javaScriptEnabled = true
                    settings.allowFileAccess = true
                    settings.domStorageEnabled = true
                    webViewClient = WebViewClient()
                    loadUrl("file://" + file.absolutePath)
                    web = this
                }
            },
            onRelease = { it.destroy() },
        )
        CompositionLocalProvider(LocalLayoutDirection provides dir) {
            Row(
                Modifier.fillMaxWidth().height(68.dp).background(C.panel).padding(12.dp, 8.dp),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                Box(
                    Modifier
                        .width(120.dp).fillMaxHeight()
                        .clip(RoundedCornerShape(16.dp))
                        .background(C.mint)
                        .clickable { onBack() },
                    contentAlignment = Alignment.Center,
                ) {
                    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                        SvgIcon(Ic.CODE, C.onMint)
                        Text("Code", color = C.onMint, fontSize = 16.sp)
                    }
                }
                Box(
                    Modifier
                        .width(88.dp).fillMaxHeight()
                        .clip(RoundedCornerShape(16.dp))
                        .background(C.key)
                        .clickable { web?.reload() },
                    contentAlignment = Alignment.Center,
                ) { Text("Reload", color = C.text, fontSize = 14.sp) }
                Text(file.name, color = C.muted, fontSize = 12.sp, maxLines = 1, modifier = Modifier.weight(1f))
            }
        }
    }
}
