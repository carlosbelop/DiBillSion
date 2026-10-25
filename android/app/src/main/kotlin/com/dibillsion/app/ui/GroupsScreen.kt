package com.dibillsion.app.ui

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.ExtendedFloatingActionButton
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.ListItem
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import com.dibillsion.app.AppViewModel
import com.dibillsion.app.R
import com.dibillsion.app.UiState
import java.util.Currency
import java.util.Locale

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun GroupsScreen(state: UiState, vm: AppViewModel) {
    var creating by rememberSaveable { mutableStateOf(false) }
    Scaffold(
        topBar = { TopAppBar(title = { Text(stringResource(R.string.groups_title)) }) },
        floatingActionButton = {
            ExtendedFloatingActionButton(onClick = { creating = true }) { Text(stringResource(R.string.new_group)) }
        },
    ) { padding ->
        if (state.groups.isEmpty()) {
            Text(
                stringResource(R.string.groups_empty),
                modifier = Modifier.padding(padding).padding(24.dp),
            )
        }
        LazyColumn(Modifier.fillMaxSize().padding(padding)) {
            items(state.groups, key = { it.id }) { g ->
                ListItem(
                    headlineContent = { Text(listOfNotNull(g.emoji, g.name).joinToString(" ")) },
                    supportingContent = { Text(g.baseCurrency) },
                    modifier = Modifier.clickable { vm.openGroup(g.id) },
                )
                HorizontalDivider()
            }
        }
    }
    if (creating) NewGroupDialog(vm, onDone = { creating = false })
    ErrorDialog(state.error, vm::dismissError)
}

@Composable
private fun NewGroupDialog(vm: AppViewModel, onDone: () -> Unit) {
    var name by remember { mutableStateOf("") }
    var currency by remember { mutableStateOf(defaultCurrency()) }
    var people by remember { mutableStateOf("") }
    val valid = name.isNotBlank() && vm.exponentOf(currency) != null
    AlertDialog(
        onDismissRequest = onDone,
        title = { Text(stringResource(R.string.new_group)) },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                OutlinedTextField(name, { name = it }, label = { Text(stringResource(R.string.group_name)) }, singleLine = true)
                OutlinedTextField(currency, { currency = it.uppercase().take(3) }, label = { Text(stringResource(R.string.base_currency)) }, singleLine = true)
                OutlinedTextField(people, { people = it }, label = { Text(stringResource(R.string.member_names)) }, modifier = Modifier.fillMaxWidth())
            }
        },
        confirmButton = {
            TextButton(enabled = valid, onClick = {
                vm.createGroup(name, currency, people.split(','))
                onDone()
            }) { Text(stringResource(R.string.create)) }
        },
        dismissButton = { TextButton(onClick = onDone) { Text(stringResource(R.string.cancel)) } },
    )
}

@Composable
fun ErrorDialog(error: String?, onDismiss: () -> Unit) {
    if (error == null) return
    AlertDialog(
        onDismissRequest = onDismiss,
        text = { Text(error) },
        confirmButton = { TextButton(onClick = onDismiss) { Text("OK") } },
    )
}

@Composable
fun ConfirmDialog(text: String, confirm: String, onConfirm: () -> Unit, onDismiss: () -> Unit) {
    AlertDialog(
        onDismissRequest = onDismiss,
        text = { Text(text) },
        confirmButton = {
            TextButton(onClick = {
                onConfirm()
                onDismiss()
            }) { Text(confirm) }
        },
        dismissButton = { TextButton(onClick = onDismiss) { Text(stringResource(R.string.cancel)) } },
    )
}

private fun defaultCurrency(): String =
    runCatching { Currency.getInstance(Locale.getDefault()).currencyCode }.getOrDefault("EUR")
