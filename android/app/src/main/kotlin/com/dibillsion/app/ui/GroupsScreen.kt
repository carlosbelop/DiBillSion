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
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.TextButton
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.CenterAlignedTopAppBar
import androidx.compose.material3.pulltorefresh.PullToRefreshBox
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
import androidx.compose.ui.graphics.lerp
import androidx.compose.ui.input.nestedscroll.nestedScroll
import androidx.compose.ui.res.pluralStringResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import com.dibillsion.app.AppViewModel
import com.dibillsion.app.Money
import com.dibillsion.app.R
import com.dibillsion.app.UiState
import com.dibillsion.core.GroupDto

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun GroupsScreen(state: UiState, vm: AppViewModel) {
    var creating by rememberSaveable { mutableStateOf(false) }
    var joining by rememberSaveable { mutableStateOf(false) }
    if (joining) JoinDialog(onJoin = vm::join, onDismiss = { joining = false })
    if (creating) {
        NewGroupScreen(vm, onDone = { creating = false })
        return
    }
    val scroll = TopAppBarDefaults.pinnedScrollBehavior()
    Scaffold(
        modifier = Modifier.nestedScroll(scroll.nestedScrollConnection),
        topBar = {
            CenterAlignedTopAppBar(
                title = { Text(stringResource(R.string.groups_title), style = MaterialTheme.typography.titleLarge) },
                actions = {
                    TextButton(onClick = { joining = true }) { Text(stringResource(R.string.join)) }
                },
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
        PullToRefreshBox(
            isRefreshing = state.refreshing,
            onRefresh = vm::refresh,
            modifier = Modifier.fillMaxSize().padding(padding),
        ) {
            LazyColumn(
                Modifier.fillMaxSize().bottomBounce(),
                contentPadding = PaddingValues(start = 16.dp, end = 16.dp, top = 8.dp, bottom = 96.dp),
                verticalArrangement = Arrangement.spacedBy(12.dp),
            ) {
                items(active, key = { it.id }) { g ->
                    GroupCard(g, state.memberCounts[g.id] ?: 0, g.id in state.sharedIds, vm, Modifier.animateItem()) {
                        vm.openGroup(g.id)
                    }
                }
                if (archived.isNotEmpty()) {
                    item(key = "archived") {
                        SectionHeader(stringResource(R.string.archived_groups), Modifier.padding(top = 8.dp).animateItem())
                    }
                    items(archived, key = { it.id }) { g ->
                        GroupCard(g, state.memberCounts[g.id] ?: 0, g.id in state.sharedIds, vm, Modifier.animateItem().alpha(0.6f)) {
                            vm.openGroup(g.id)
                        }
                    }
                }
            }
        }
    }
    ErrorDialog(state.error, vm::dismissError)
}

@Composable
private fun GroupCard(g: GroupDto, people: Int, shared: Boolean, vm: AppViewModel, modifier: Modifier = Modifier, onClick: () -> Unit) {
    // Each group gets a faint wash of its own colour so cards are told apart at a glance.
    val tint = lerp(MaterialTheme.colorScheme.surfaceContainerLow, avatarColor(g.id), 0.22f)
    ElevatedCard(
        onClick = onClick,
        modifier = modifier.fillMaxWidth(),
        colors = CardDefaults.elevatedCardColors(containerColor = tint),
    ) {
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
                    ).plus(if (shared) listOf(stringResource(R.string.shared)) else emptyList()).joinToString(" · "),
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
                g.myBalanceMinor?.let { mine ->
                    val amount = Money.format(kotlin.math.abs(mine), vm.exponentOf(g.baseCurrency) ?: 2, g.baseCurrency)
                    Text(
                        when {
                            mine < 0 -> stringResource(R.string.you_owe, amount)
                            mine > 0 -> stringResource(R.string.you_are_owed, amount)
                            else -> stringResource(R.string.you_are_square)
                        },
                        style = MaterialTheme.typography.labelLarge,
                        color = balanceColor(mine),
                        modifier = Modifier.padding(top = 2.dp),
                    )
                }
            }
            Icon(
                Icons.AutoMirrored.Filled.KeyboardArrowRight,
                contentDescription = null,
                tint = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
    }
}

@Composable
private fun JoinDialog(onJoin: (String) -> Unit, onDismiss: () -> Unit) {
    var text by rememberSaveable { mutableStateOf("") }
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(stringResource(R.string.join_title)) },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
                Text(stringResource(R.string.join_body), style = MaterialTheme.typography.bodyMedium)
                OutlinedTextField(
                    text, { text = it },
                    label = { Text(stringResource(R.string.invite_link_or_code)) },
                    singleLine = true,
                    modifier = Modifier.fillMaxWidth(),
                )
            }
        },
        confirmButton = {
            TextButton(enabled = text.isNotBlank(), onClick = {
                onJoin(text)
                onDismiss()
            }) { Text(stringResource(R.string.join)) }
        },
        dismissButton = { TextButton(onClick = onDismiss) { Text(stringResource(R.string.cancel)) } },
    )
}
