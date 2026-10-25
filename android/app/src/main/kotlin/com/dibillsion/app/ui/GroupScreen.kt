package com.dibillsion.app.ui

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.Button
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.ExtendedFloatingActionButton
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.ListItem
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.PrimaryTabRow
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Tab
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import com.dibillsion.app.AppViewModel
import com.dibillsion.app.GroupDetail
import com.dibillsion.app.Money
import com.dibillsion.app.R
import com.dibillsion.core.MemberDto
import com.dibillsion.core.SuggestionDto
import com.dibillsion.core.TransactionDto
import com.dibillsion.core.TransactionKindDto

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun GroupScreen(d: GroupDetail, error: String?, vm: AppViewModel) {
    var tab by rememberSaveable { mutableIntStateOf(0) }
    var adding by rememberSaveable { mutableStateOf(false) }
    if (adding) {
        AddExpenseScreen(d, vm, onDone = { adding = false })
        ErrorDialog(error, vm::dismissError)
        return
    }
    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text(listOfNotNull(d.group.emoji, d.group.name).joinToString(" ")) },
                navigationIcon = { TextButton(onClick = vm::closeGroup) { Text(stringResource(R.string.back)) } },
            )
        },
        floatingActionButton = {
            if (tab == 0) {
                ExtendedFloatingActionButton(onClick = { adding = true }) { Text(stringResource(R.string.add_expense)) }
            }
        },
    ) { padding ->
        Column(Modifier.fillMaxSize().padding(padding)) {
            PrimaryTabRow(selectedTabIndex = tab) {
                listOf(R.string.tab_expenses, R.string.tab_balances, R.string.tab_people).forEachIndexed { i, label ->
                    Tab(selected = tab == i, onClick = { tab = i }, text = { Text(stringResource(label)) })
                }
            }
            when (tab) {
                0 -> Transactions(d, vm)
                1 -> Balances(d, vm)
                else -> People(d, vm)
            }
        }
    }
    ErrorDialog(error, vm::dismissError)
}

@Composable
private fun Transactions(d: GroupDetail, vm: AppViewModel) {
    var deleting by remember { mutableStateOf<TransactionDto?>(null) }
    deleting?.let { t ->
        ConfirmDialog(
            text = stringResource(R.string.confirm_delete_expense, headline(d, t)),
            confirm = stringResource(R.string.delete),
            onConfirm = { t.id?.let(vm::deleteTransaction) },
            onDismiss = { deleting = null },
        )
    }
    if (d.transactions.isEmpty()) {
        Text(stringResource(R.string.expenses_empty), Modifier.padding(24.dp))
        return
    }
    LazyColumn {
        items(d.transactions, key = { it.id ?: "" }) { t ->
            ListItem(
                headlineContent = { Text(headline(d, t)) },
                supportingContent = { Text(subtitle(d, t)) },
                trailingContent = {
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        Column(horizontalAlignment = Alignment.End) {
                            val exp = vm.exponentOf(t.currency) ?: 2
                            Text(Money.format(t.items.sumOf { it.totalMinor }, exp, t.currency))
                            t.baseTotalMinor?.let {
                                Text(
                                    Money.format(it, d.exponent, d.group.baseCurrency),
                                    style = MaterialTheme.typography.bodySmall,
                                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                                )
                            }
                        }
                        TextButton(onClick = { deleting = t }) { Text(stringResource(R.string.delete)) }
                    }
                },
            )
            HorizontalDivider()
        }
    }
}

@Composable
private fun headline(d: GroupDetail, t: TransactionDto): String = when (t.kind) {
    TransactionKindDto.TRANSFER ->
        stringResource(R.string.transfer_line, d.name(t.payerId), d.name(t.items.first().shares.first().memberId))
    else -> t.title
}

@Composable
private fun subtitle(d: GroupDetail, t: TransactionDto): String = when (t.kind) {
    TransactionKindDto.EXPENSE -> stringResource(R.string.paid_by, d.name(t.payerId))
    TransactionKindDto.INCOME -> stringResource(R.string.income_line, d.name(t.payerId))
    TransactionKindDto.TRANSFER -> ""
}

@Composable
private fun Balances(d: GroupDetail, vm: AppViewModel) {
    val b = d.balances
    var paying by remember { mutableStateOf<SuggestionDto?>(null) }
    paying?.let { s ->
        ConfirmDialog(
            text = stringResource(
                R.string.confirm_paid, s.fromName, Money.format(s.amountMinor, d.exponent, b.currency), s.toName,
            ),
            confirm = stringResource(R.string.mark_paid),
            onConfirm = { vm.markPaid(s.fromId, s.toId, s.amountMinor) },
            onDismiss = { paying = null },
        )
    }
    LazyColumn {
        items(b.balances, key = { it.memberId }) { m ->
            ListItem(
                headlineContent = { Text(m.displayName) },
                trailingContent = {
                    Text(
                        Money.format(m.amountMinor, d.exponent, b.currency),
                        color = if (m.amountMinor < 0) MaterialTheme.colorScheme.error else MaterialTheme.colorScheme.primary,
                    )
                },
            )
        }
        item {
            HorizontalDivider()
            Text(
                stringResource(if (b.suggestions.isEmpty()) R.string.balances_settled else R.string.suggested),
                style = MaterialTheme.typography.titleMedium,
                modifier = Modifier.padding(16.dp),
            )
        }
        items(b.suggestions, key = { it.fromId + it.toId }) { s ->
            ListItem(
                headlineContent = { Text(stringResource(R.string.pays, s.fromName, s.toName)) },
                supportingContent = { Text(Money.format(s.amountMinor, d.exponent, b.currency)) },
                trailingContent = {
                    Button(onClick = { paying = s }) { Text(stringResource(R.string.mark_paid)) }
                },
            )
        }
    }
}

@Composable
private fun People(d: GroupDetail, vm: AppViewModel) {
    var name by rememberSaveable { mutableStateOf("") }
    var deleting by remember { mutableStateOf<MemberDto?>(null) }
    deleting?.let { m ->
        ConfirmDialog(
            text = stringResource(R.string.confirm_delete_person, m.displayName),
            confirm = stringResource(R.string.delete),
            onConfirm = { vm.deleteMember(m.id) },
            onDismiss = { deleting = null },
        )
    }
    Column(Modifier.fillMaxWidth()) {
        Row(
            Modifier.fillMaxWidth().padding(16.dp),
            horizontalArrangement = Arrangement.spacedBy(8.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            OutlinedTextField(name, { name = it }, label = { Text(stringResource(R.string.person_name)) }, singleLine = true, modifier = Modifier.weight(1f))
            Button(enabled = name.isNotBlank(), onClick = {
                vm.addMember(name)
                name = ""
            }) { Text(stringResource(R.string.add_person)) }
        }
        LazyColumn {
            items(d.members, key = { it.id }) { m ->
                ListItem(
                    headlineContent = { Text(m.displayName) },
                    supportingContent = { if (!m.active) Text(stringResource(R.string.inactive)) },
                    trailingContent = {
                        Row {
                            TextButton(onClick = { vm.setMemberActive(m.id, !m.active) }) {
                                Text(stringResource(if (m.active) R.string.deactivate else R.string.activate))
                            }
                            TextButton(onClick = { deleting = m }) { Text(stringResource(R.string.delete)) }
                        }
                    },
                )
            }
        }
    }
}
