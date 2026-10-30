package com.dibillsion.app

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.BackHandler
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
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
        val detail = state.detail
        if (detail == null) {
            GroupsScreen(state, vm)
        } else {
            BackHandler { vm.closeGroup() }
            GroupScreen(detail, state.error, vm)
        }
    }
}
