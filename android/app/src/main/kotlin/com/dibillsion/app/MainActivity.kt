package com.dibillsion.app

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.BackHandler
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.slideInHorizontally
import androidx.compose.animation.slideOutHorizontally
import androidx.compose.animation.togetherWith
import androidx.compose.material3.Surface
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewmodel.compose.viewModel
import com.dibillsion.app.ui.DiBillTheme
import com.dibillsion.app.ui.GroupScreen
import com.dibillsion.app.ui.GroupsScreen

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        setContent { DiBillTheme { App() } }
    }
}

@Composable
private fun App(vm: AppViewModel = viewModel()) {
    val state by vm.state.collectAsStateWithLifecycle()
    Surface {
        // Opening a group slides in from the side; going back slides it out.
        AnimatedContent(
            targetState = state.detail?.group?.id,
            transitionSpec = {
                val forward = targetState != null
                (slideInHorizontally { w -> if (forward) w / 3 else -w / 3 } + fadeIn())
                    .togetherWith(slideOutHorizontally { w -> if (forward) -w / 3 else w / 3 } + fadeOut())
            },
            label = "screen",
        ) { groupId ->
            val detail = state.detail?.takeIf { it.group.id == groupId }
            if (groupId == null || detail == null) {
                GroupsScreen(state, vm)
            } else {
                BackHandler { vm.closeGroup() }
                GroupScreen(detail, state, vm)
            }
        }
    }
}
