package com.dibillsion.app.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.automirrored.filled.ArrowForward
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.AddCircle
import androidx.compose.material.icons.filled.CheckCircle
import androidx.compose.material.icons.filled.Delete
import androidx.compose.material.icons.filled.MoreVert
import androidx.compose.material.icons.filled.ShoppingCart
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.ExtendedFloatingActionButton
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.ListItem
import androidx.compose.material3.ListItemDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedCard
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.PrimaryTabRow
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Switch
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
import androidx.compose.ui.draw.clip
import androidx.compose.ui.res.pluralStringResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import com.dibillsion.app.AppViewModel
import com.dibillsion.app.GroupDetail
import com.dibillsion.app.Money
import com.dibillsion.app.R
import com.dibillsion.core.MemberDto
import com.dibillsion.core.SuggestionDto
import com.dibillsion.core.TransactionDto
import com.dibillsion.core.TransactionKindDto
import java.time.LocalDate
import java.time.format.DateTimeFormatter
import java.time.format.FormatStyle
import kotlin.math.abs

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun GroupScreen(d: GroupDetail, error: String?, vm: AppViewModel) {
    var tab by rememberSaveable { mutableIntStateOf(0) }
    var adding by rememberSaveable { mutableStateOf(false) }
    var addingPerson by rememberSaveable { mutableStateOf(false) }
    var menu by remember { mutableStateOf(false) }
    var deletingGroup by remember { mutableStateOf(false) }
    if (adding) {
        AddExpenseScreen(d, vm, onDone = { adding = false })
        ErrorDialog(error, vm::dismissError)
        return
    }
    if (deletingGroup) {
        ConfirmDialog(
            title = stringResource(R.string.delete_group_title, d.group.name),
            text = stringResource(R.string.delete_group_body),
            confirm = stringResource(R.string.delete),
            onConfirm = vm::deleteGroup,
            onDismiss = { deletingGroup = false },
        )
    }
    if (addingPerson) AddPersonDialog(onAdd = vm::addMember, onDismiss = { addingPerson = false })

    Scaffold(
        topBar = {
            TopAppBar(
                title = {
                    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                        Avatar(d.group.emoji ?: initials(d.group.name), d.group.id, size = 36.dp, shape = MaterialTheme.shapes.small)
                        Column {
                            Text(d.group.name, style = MaterialTheme.typography.titleMedium)
                            Text(
                                pluralStringResource(R.plurals.people_count, d.members.size, d.members.size),
                                style = MaterialTheme.typography.bodySmall,
                                color = MaterialTheme.colorScheme.onSurfaceVariant,
                            )
                        }
                    }
                },
                navigationIcon = {
                    IconButton(onClick = vm::closeGroup) {
                        Icon(Icons.AutoMirrored.Filled.ArrowBack, stringResource(R.string.back))
                    }
                },
                actions = {
                    IconButton(onClick = { menu = true }) { Icon(Icons.Filled.MoreVert, stringResource(R.string.more)) }
                    DropdownMenu(expanded = menu, onDismissRequest = { menu = false }) {
                        DropdownMenuItem(
                            text = { Text(stringResource(if (d.group.archived) R.string.restore_group else R.string.archive_group)) },
                            onClick = {
                                menu = false
                                vm.setArchived(!d.group.archived)
                            },
                        )
                        DropdownMenuItem(
                            text = { Text(stringResource(R.string.delete_group)) },
                            leadingIcon = { Icon(Icons.Filled.Delete, null) },
                            onClick = {
                                menu = false
                                deletingGroup = true
                            },
                        )
                    }
                },
            )
        },
        floatingActionButton = {
            when (tab) {
                0 -> ExtendedFloatingActionButton(
                    onClick = { adding = true },
                    icon = { Icon(Icons.Filled.Add, null) },
                    text = { Text(stringResource(R.string.add_expense)) },
                )
                2 -> ExtendedFloatingActionButton(
                    onClick = { addingPerson = true },
                    icon = { Icon(Icons.Filled.Add, null) },
                    text = { Text(stringResource(R.string.add_person)) },
                )
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
                0 -> Transactions(d, vm, onAdd = { adding = true })
                1 -> Balances(d, vm)
                else -> People(d, vm)
            }
        }
    }
    ErrorDialog(error, vm::dismissError)
}

@Composable
private fun Transactions(d: GroupDetail, vm: AppViewModel, onAdd: () -> Unit) {
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
        EmptyState(
            emoji = "🪙",
            title = stringResource(R.string.expenses_empty_title),
            body = stringResource(R.string.expenses_empty),
            action = stringResource(R.string.add_expense),
            onAction = onAdd,
        )
        return
    }
    val byDay = d.transactions.groupBy { it.dateDays }
    LazyColumn(contentPadding = PaddingValues(bottom = 96.dp)) {
        item { SpendingSummary(d) }
        byDay.forEach { (day, list) ->
            item(key = "day-$day") { SectionHeader(dayLabel(day)) }
            items(list, key = { it.id ?: "" }) { t -> TransactionRow(d, t, vm, onDelete = { deleting = t }) }
        }
    }
}

@Composable
private fun SpendingSummary(d: GroupDetail) {
    val expenses = d.transactions.filter { it.kind == TransactionKindDto.EXPENSE }
    val total = expenses.sumOf { t -> t.baseTotalMinor ?: t.items.sumOf { it.totalMinor } }
    Card(
        Modifier.fillMaxWidth().padding(16.dp),
        colors = CardDefaults.cardColors(
            containerColor = MaterialTheme.colorScheme.primaryContainer,
            contentColor = MaterialTheme.colorScheme.onPrimaryContainer,
        ),
    ) {
        Column(Modifier.padding(20.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
            Text(stringResource(R.string.total_spent), style = MaterialTheme.typography.labelLarge)
            Text(
                Money.format(total, d.exponent, d.group.baseCurrency),
                style = MaterialTheme.typography.headlineMedium,
                fontWeight = FontWeight.SemiBold,
            )
            Text(
                pluralStringResource(R.plurals.expenses_count, expenses.size, expenses.size),
                style = MaterialTheme.typography.bodyMedium,
            )
        }
    }
}

@Composable
private fun TransactionRow(d: GroupDetail, t: TransactionDto, vm: AppViewModel, onDelete: () -> Unit) {
    var menu by remember { mutableStateOf(false) }
    val icon = when (t.kind) {
        TransactionKindDto.EXPENSE -> Icons.Filled.ShoppingCart
        TransactionKindDto.INCOME -> Icons.Filled.AddCircle
        TransactionKindDto.TRANSFER -> Icons.AutoMirrored.Filled.ArrowForward
    }
    ListItem(
        leadingContent = { IconBadge(icon) },
        headlineContent = { Text(headline(d, t)) },
        supportingContent = subtitle(d, t)?.let { { Text(it) } },
        trailingContent = {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Column(horizontalAlignment = Alignment.End) {
                    val exp = vm.exponentOf(t.currency) ?: 2
                    Text(
                        Money.format(t.items.sumOf { it.totalMinor }, exp, t.currency),
                        style = MaterialTheme.typography.titleSmall,
                        color = if (t.kind == TransactionKindDto.INCOME) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.onSurface,
                    )
                    t.baseTotalMinor?.let {
                        Text(
                            Money.format(it, d.exponent, d.group.baseCurrency),
                            style = MaterialTheme.typography.bodySmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                    }
                }
                Box {
                    IconButton(onClick = { menu = true }) { Icon(Icons.Filled.MoreVert, stringResource(R.string.more)) }
                    DropdownMenu(expanded = menu, onDismissRequest = { menu = false }) {
                        DropdownMenuItem(
                            text = { Text(stringResource(R.string.delete)) },
                            leadingIcon = { Icon(Icons.Filled.Delete, null) },
                            onClick = {
                                menu = false
                                onDelete()
                            },
                        )
                    }
                }
            }
        },
    )
}

@Composable
private fun dayLabel(day: Int): String {
    val date = LocalDate.ofEpochDay(day.toLong())
    val today = LocalDate.now()
    return when (date) {
        today -> stringResource(R.string.today)
        today.minusDays(1) -> stringResource(R.string.yesterday)
        else -> date.format(DateTimeFormatter.ofLocalizedDate(FormatStyle.MEDIUM))
    }
}

@Composable
private fun headline(d: GroupDetail, t: TransactionDto): String = when (t.kind) {
    TransactionKindDto.TRANSFER ->
        stringResource(R.string.transfer_line, d.name(t.payerId), d.name(t.items.first().shares.first().memberId))
    else -> t.title
}

@Composable
private fun subtitle(d: GroupDetail, t: TransactionDto): String? = when (t.kind) {
    TransactionKindDto.EXPENSE -> stringResource(R.string.paid_by, d.name(t.payerId))
    TransactionKindDto.INCOME -> stringResource(R.string.income_line, d.name(t.payerId))
    TransactionKindDto.TRANSFER -> null
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
    val largest = b.balances.maxOfOrNull { abs(it.amountMinor) }?.takeIf { it > 0 } ?: 1L
    LazyColumn(contentPadding = PaddingValues(bottom = 32.dp)) {
        item { SectionHeader(stringResource(R.string.balances_header)) }
        items(b.balances, key = { it.memberId }) { m ->
            Column(Modifier.padding(horizontal = 16.dp, vertical = 8.dp)) {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Avatar(initials(m.displayName), m.memberId, size = 32.dp)
                    Text(m.displayName, Modifier.padding(start = 12.dp).weight(1f), style = MaterialTheme.typography.bodyLarge)
                    Text(
                        Money.format(m.amountMinor, d.exponent, b.currency),
                        style = MaterialTheme.typography.titleSmall,
                        color = balanceColor(m.amountMinor),
                    )
                }
                BalanceBar(m.amountMinor.toFloat() / largest, Modifier.padding(top = 8.dp))
            }
        }
        item {
            SectionHeader(stringResource(R.string.suggested))
            if (b.suggestions.isEmpty()) {
                Row(
                    Modifier.padding(horizontal = 16.dp),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(8.dp),
                ) {
                    Icon(Icons.Filled.CheckCircle, null, tint = MaterialTheme.colorScheme.primary)
                    Text(stringResource(R.string.balances_settled))
                }
            }
        }
        items(b.suggestions, key = { it.fromId + it.toId }) { s ->
            OutlinedCard(Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 6.dp)) {
                Row(Modifier.padding(16.dp), verticalAlignment = Alignment.CenterVertically) {
                    Avatar(initials(s.fromName), s.fromId, size = 36.dp)
                    Icon(
                        Icons.AutoMirrored.Filled.ArrowForward, null,
                        Modifier.padding(horizontal = 4.dp),
                        tint = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                    Avatar(initials(s.toName), s.toId, size = 36.dp)
                    Column(Modifier.weight(1f).padding(start = 12.dp)) {
                        Text(stringResource(R.string.pays, s.fromName, s.toName), style = MaterialTheme.typography.bodyMedium)
                        Text(
                            Money.format(s.amountMinor, d.exponent, b.currency),
                            style = MaterialTheme.typography.titleMedium,
                            fontWeight = FontWeight.SemiBold,
                        )
                    }
                    FilledTonalButton(onClick = { paying = s }) { Text(stringResource(R.string.settle)) }
                }
            }
        }
    }
}

/** A bar centred on zero: owed money grows right, debt grows left. */
@Composable
private fun BalanceBar(fraction: Float, modifier: Modifier = Modifier) {
    val track = MaterialTheme.colorScheme.surfaceContainerHighest
    val shape = RoundedCornerShape(4.dp)
    Row(modifier.fillMaxWidth().height(8.dp)) {
        Box(Modifier.weight(1f).fillMaxHeight().clip(shape).background(track), contentAlignment = Alignment.CenterEnd) {
            if (fraction < 0) {
                Box(Modifier.fillMaxWidth(-fraction).fillMaxHeight().clip(shape).background(MaterialTheme.colorScheme.error))
            }
        }
        Spacer(Modifier.width(2.dp))
        Box(Modifier.weight(1f).fillMaxHeight().clip(shape).background(track), contentAlignment = Alignment.CenterStart) {
            if (fraction > 0) {
                Box(Modifier.fillMaxWidth(fraction).fillMaxHeight().clip(shape).background(MaterialTheme.colorScheme.primary))
            }
        }
    }
}

@Composable
private fun People(d: GroupDetail, vm: AppViewModel) {
    var deleting by remember { mutableStateOf<MemberDto?>(null) }
    deleting?.let { m ->
        ConfirmDialog(
            text = stringResource(R.string.confirm_delete_person, m.displayName),
            confirm = stringResource(R.string.remove),
            onConfirm = { vm.deleteMember(m.id) },
            onDismiss = { deleting = null },
        )
    }
    LazyColumn(contentPadding = PaddingValues(top = 8.dp, bottom = 96.dp)) {
        items(d.members, key = { it.id }) { m ->
            ListItem(
                leadingContent = { Avatar(initials(m.displayName), m.id) },
                headlineContent = { Text(m.displayName) },
                supportingContent = {
                    Text(stringResource(if (m.active) R.string.active else R.string.inactive))
                },
                trailingContent = {
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        Switch(checked = m.active, onCheckedChange = { vm.setMemberActive(m.id, it) })
                        IconButton(onClick = { deleting = m }) { Icon(Icons.Filled.Delete, stringResource(R.string.remove)) }
                    }
                },
                colors = ListItemDefaults.colors(),
            )
        }
    }
}

@Composable
private fun AddPersonDialog(onAdd: (String) -> Unit, onDismiss: () -> Unit) {
    var name by rememberSaveable { mutableStateOf("") }
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(stringResource(R.string.add_person)) },
        text = {
            OutlinedTextField(name, { name = it }, label = { Text(stringResource(R.string.person_name)) }, singleLine = true)
        },
        confirmButton = {
            TextButton(enabled = name.isNotBlank(), onClick = {
                onAdd(name)
                onDismiss()
            }) { Text(stringResource(R.string.add)) }
        },
        dismissButton = { TextButton(onClick = onDismiss) { Text(stringResource(R.string.cancel)) } },
    )
}
