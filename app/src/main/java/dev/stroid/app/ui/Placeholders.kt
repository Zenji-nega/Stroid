package dev.stroid.app.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.*
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp

@Composable
fun SoonScreen(title: String, note: String) {
    Box(Modifier.fillMaxSize().background(C.bg).padding(32.dp), contentAlignment = Alignment.Center) {
        Column(horizontalAlignment = Alignment.CenterHorizontally) {
            Text(title, color = C.text, fontSize = 20.sp)
            Spacer(Modifier.height(8.dp))
            Text(note, color = C.muted, fontSize = 14.sp)
        }
    }
}
