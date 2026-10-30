package com.dibillsion.app.ui

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.Close
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FilterChip
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.unit.dp
import com.dibillsion.app.AppViewModel
import com.dibillsion.app.R
import com.dibillsion.core.GroupKindDto
import java.util.Currency
import java.util.Locale

private val Emojis = listOf("✈️", "🏖️", "🏠", "🍽️", "🎉", "🚗", "⛺", "⛷️", "💼", "❤️", "🎓", "⚽")

@OptIn(ExperimentalMaterial3Api::class, ExperimentalLayoutApi::class)
@Composable
fun NewGroupScreen(vm: AppViewModel, onDone: () -> Unit) {
    var name by rememberSaveable { mutableStateOf("") }
    var emoji by rememberSaveable { mutableStateOf<String?>(null) }
    var kind by rememberSaveable { mutableStateOf(GroupKindDto.TRIP) }
    var currency by rememberSaveable { mutableStateOf(defaultCurrency()) }
    var me by rememberSaveable { mutableStateOf("") }
    var others by rememberSaveable { mutableStateOf(listOf("")) }
    var discarding by rememberSaveable { mutableStateOf(false) }

    val currencyOk = vm.exponentOf(currency) != null
    val valid = name.isNotBlank() && currencyOk
    val dirty = name.isNotBlank() || me.isNotBlank() || others.any { it.isNotBlank() } || emoji != null
    val close = { if (dirty) discarding = true else onDone() }
    BackHandler(onBack = close)
    if (discarding) {
        ConfirmDialog(
            text = stringResource(R.string.discard_changes),
            confirm = stringResource(R.string.discard),
            onConfirm = onDone,
            onDismiss = { discarding = false },
        )
    }

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text(stringResource(R.string.new_group)) },
                navigationIcon = {
                    IconButton(onClick = close) { Icon(Icons.Filled.Close, stringResource(R.string.cancel)) }
                },
                actions = {
                    TextButton(enabled = valid, onClick = {
                        vm.createGroup(name, emoji, kind, currency, me, others)
                        onDone()
                    }) { Text(stringResource(R.string.create)) }
                },
            )
        },
    ) { padding ->
        Column(
            Modifier.fillMaxSize().padding(padding).imePadding().verticalScroll(rememberScrollState()).padding(16.dp),
            verticalArrangement = Arrangement.spacedBy(16.dp),
        ) {
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(16.dp)) {
                Avatar(
                    text = emoji ?: initials(name.ifBlank { "?" }),
                    colorKey = name,
                    size = 64.dp,
                    shape = MaterialTheme.shapes.large,
                )
                OutlinedTextField(
                    name, { name = it },
                    label = { Text(stringResource(R.string.group_name)) },
                    singleLine = true,
                    keyboardOptions = KeyboardOptions(capitalization = KeyboardCapitalization.Sentences),
                    modifier = Modifier.weight(1f),
                )
            }
            FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                Emojis.forEach { e ->
                    val selected = e == emoji
                    Box(
                        Modifier.size(44.dp).clip(MaterialTheme.shapes.small)
                            .background(if (selected) MaterialTheme.colorScheme.primaryContainer else MaterialTheme.colorScheme.surfaceContainerHigh)
                            .then(
                                if (selected) Modifier.border(BorderStroke(2.dp, MaterialTheme.colorScheme.primary), MaterialTheme.shapes.small)
                                else Modifier,
                            )
                            .clickable { emoji = if (selected) null else e },
                        contentAlignment = Alignment.Center,
                    ) { Text(e, style = MaterialTheme.typography.titleLarge) }
                }
            }

            SectionLabel(stringResource(R.string.group_kind))
            FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                GroupKindDto.entries.forEach { k ->
                    FilterChip(selected = kind == k, onClick = { kind = k }, label = { Text(stringResource(kindLabel(k))) })
                }
            }

            OutlinedTextField(
                currency, { currency = it.uppercase().filter(Char::isLetter).take(3) },
                label = { Text(stringResource(R.string.base_currency)) },
                supportingText = {
                    Text(stringResource(if (currencyOk) R.string.base_currency_help else R.string.base_currency_invalid))
                },
                isError = !currencyOk,
                singleLine = true,
                keyboardOptions = KeyboardOptions(capitalization = KeyboardCapitalization.Characters),
                modifier = Modifier.fillMaxWidth(),
            )

            SectionLabel(stringResource(R.string.people))
            OutlinedTextField(
                me, { me = it },
                label = { Text(stringResource(R.string.your_name)) },
                singleLine = true,
                keyboardOptions = KeyboardOptions(capitalization = KeyboardCapitalization.Words),
                modifier = Modifier.fillMaxWidth(),
            )
            others.forEachIndexed { i, other ->
                OutlinedTextField(
                    other, { v -> others = others.toMutableList().also { it[i] = v } },
                    label = { Text(stringResource(R.string.person_n, i + 2)) },
                    singleLine = true,
                    keyboardOptions = KeyboardOptions(capitalization = KeyboardCapitalization.Words),
                    trailingIcon = {
                        if (others.size > 1) {
                            IconButton(onClick = { others = others.filterIndexed { j, _ -> j != i } }) {
                                Icon(Icons.Filled.Close, stringResource(R.string.remove))
                            }
                        }
                    },
                    modifier = Modifier.fillMaxWidth(),
                )
            }
            TextButton(onClick = { others = others + "" }) {
                Icon(Icons.Filled.Add, contentDescription = null)
                Text(stringResource(R.string.add_another_person), Modifier.padding(start = 8.dp))
            }
        }
    }
}

@Composable
fun SectionLabel(text: String) {
    Text(text, style = MaterialTheme.typography.titleSmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
}

fun kindLabel(kind: GroupKindDto): Int = when (kind) {
    GroupKindDto.TRIP -> R.string.kind_trip
    GroupKindDto.HOME -> R.string.kind_home
    GroupKindDto.COUPLE -> R.string.kind_couple
    GroupKindDto.EVENT -> R.string.kind_event
    GroupKindDto.PROJECT -> R.string.kind_project
    GroupKindDto.OTHER -> R.string.kind_other
}

private fun defaultCurrency(): String =
    runCatching { Currency.getInstance(Locale.getDefault()).currencyCode }.getOrDefault("EUR")
