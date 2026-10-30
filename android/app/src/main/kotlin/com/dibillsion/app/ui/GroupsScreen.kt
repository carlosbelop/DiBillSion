package com.dibillsion.app.ui

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.KeyboardArrowRight
import androidx.compose.material.icons.filled.Add
import androidx.compose.material3.ElevatedCard
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.ExtendedFloatingActionButton
import androidx.compose.material3.Icon
import androidx.compose.material3.LargeTopAppBar
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.input.nestedscroll.nestedScroll
import androidx.compose.ui.res.pluralStringResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import com.dibillsion.app.AppViewModel
import com.dibillsion.app.R
import com.dibillsion.app.UiState
import com.dibillsion.core.GroupDto

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun GroupsScreen(state: UiState, vm: AppViewModel) {
    var creating by rememberSaveable { mutableStateOf(false) }
    if (creating) {
        NewGroupScreen(vm, onDone = { creating = false })
        return
    }
    val scroll = TopAppBarDefaults.exitUntilCollapsedScrollBehavior()
    Scaffold(
        modifier = Modifier.nestedScroll(scroll.nestedScrollConnection),
        topBar = {
            LargeTopAppBar(
                title = { Text(stringResource(R.string.groups_title)) },
                scrollBehavior = scroll,
            )
        },
        floatingActionButton = {
            if (state.groups.isNotEmpty()) {
                ExtendedFloatingActionButton(
                    onClick = { creating = true },
                    icon = { Icon(Icons.Filled.Add, contentDescription = null) },
                    text = { Text(stringResource(R.string.new_group)) },
                )
            }
        },
    ) { padding ->
        if (state.groups.isEmpty()) {
            EmptyState(
                emoji = "🧾",
                title = stringResource(R.string.groups_empty_title),
                body = stringResource(R.string.groups_empty),
                action = stringResource(R.string.new_group),
                onAction = { creating = true },
                modifier = Modifier.padding(padding),
            )
            return@Scaffold
        }
        val (archived, active) = state.groups.partition { it.archived }
        LazyColumn(
            Modifier.fillMaxSize().padding(padding),
            contentPadding = PaddingValues(start = 16.dp, end = 16.dp, bottom = 96.dp),
            verticalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            items(active, key = { it.id }) { g -> GroupCard(g, state.memberCounts[g.id] ?: 0) { vm.openGroup(g.id) } }
            if (archived.isNotEmpty()) {
                item { SectionHeader(stringResource(R.string.archived_groups), Modifier.padding(top = 8.dp)) }
                items(archived, key = { it.id }) { g ->
                    GroupCard(g, state.memberCounts[g.id] ?: 0, Modifier.alpha(0.6f)) { vm.openGroup(g.id) }
                }
            }
        }
    }
    ErrorDialog(state.error, vm::dismissError)
}

@Composable
private fun GroupCard(g: GroupDto, people: Int, modifier: Modifier = Modifier, onClick: () -> Unit) {
    ElevatedCard(onClick = onClick, modifier = modifier.fillMaxWidth()) {
        Row(
            Modifier.padding(16.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(16.dp),
        ) {
            Avatar(
                text = g.emoji ?: initials(g.name),
                colorKey = g.id,
                size = 52.dp,
                shape = MaterialTheme.shapes.medium,
            )
            Column(Modifier.weight(1f)) {
                Text(g.name, style = MaterialTheme.typography.titleMedium)
                Text(
                    listOf(
                        stringResource(kindLabel(g.kind)),
                        pluralStringResource(R.plurals.people_count, people, people),
                        g.baseCurrency,
                    ).joinToString(" · "),
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
            Icon(
                Icons.AutoMirrored.Filled.KeyboardArrowRight,
                contentDescription = null,
                tint = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
    }
}
