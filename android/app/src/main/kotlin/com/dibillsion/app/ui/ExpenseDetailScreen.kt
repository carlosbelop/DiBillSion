package com.dibillsion.app.ui

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.filled.Delete
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FilterChip
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.pluralStringResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import com.dibillsion.app.AppViewModel
import com.dibillsion.app.GroupDetail
import com.dibillsion.app.Money
import com.dibillsion.app.R
import com.dibillsion.core.TransactionDto
import com.dibillsion.core.TransactionKindDto
import java.time.LocalDate
import java.time.format.DateTimeFormatter
import java.time.format.FormatStyle

/**
 * One expense or income with its items (ADR 0009): anyone can add or remove people on each
 * item, and the per-person totals follow, computed by the core.
 */
@OptIn(ExperimentalMaterial3Api::class, ExperimentalLayoutApi::class)
@Composable
fun ExpenseDetailScreen(d: GroupDetail, t: TransactionDto, vm: AppViewModel, onClose: () -> Unit) {
    BackHandler(onBack = onClose)
    var deleting by remember { mutableStateOf(false) }
    val id = t.id ?: return
    val exponent = vm.exponentOf(t.currency) ?: d.exponent
    val fmt = { minor: Long -> Money.format(minor, exponent, t.currency) }
    val active = d.members.filter { it.active }
    val owed = mutableMapOf<String, Long>()

    if (deleting) {
        ConfirmDialog(
            text = stringResource(R.string.confirm_delete_expense, t.title),
            confirm = stringResource(R.string.delete),
            onConfirm = {
                vm.deleteTransaction(id)
                onClose()
            },
            onDismiss = { deleting = false },
        )
    }
    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text(t.title, maxLines = 1) },
                navigationIcon = {
                    IconButton(onClick = onClose) { Icon(Icons.AutoMirrored.Filled.ArrowBack, stringResource(R.string.back)) }
                },
                actions = {
                    IconButton(onClick = { deleting = true }) { Icon(Icons.Filled.Delete, stringResource(R.string.delete)) }
                },
            )
        },
    ) { padding ->
        Column(
            Modifier.fillMaxSize().padding(padding).verticalScroll(rememberScrollState()).padding(16.dp),
            verticalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            Card(
                Modifier.fillMaxWidth(),
                colors = CardDefaults.cardColors(
                    containerColor = MaterialTheme.colorScheme.primaryContainer,
                    contentColor = MaterialTheme.colorScheme.onPrimaryContainer,
                ),
            ) {
                Column(Modifier.padding(20.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
                    Text(fmt(t.items.sumOf { it.totalMinor }), style = MaterialTheme.typography.headlineMedium, fontWeight = FontWeight.SemiBold)
                    t.baseTotalMinor?.let { Text(Money.format(it, d.exponent, d.group.baseCurrency)) }
                    Text(
                        stringResource(
                            if (t.kind == TransactionKindDto.INCOME) R.string.income_line else R.string.paid_by,
                            d.name(t.payerId),
                        ),
                    )
                    Text(
                        LocalDate.ofEpochDay(t.dateDays.toLong()).format(DateTimeFormatter.ofLocalizedDate(FormatStyle.MEDIUM)) +
                            " · " + pluralStringResource(R.plurals.items_count, t.items.size, t.items.size),
                        style = MaterialTheme.typography.bodyMedium,
                    )
                }
            }
            Text(
                stringResource(R.string.detail_items_hint),
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )

            t.items.forEachIndexed { index, item ->
                val parts = vm.itemShares(item)
                item.shares.zip(parts).forEach { (s, p) -> owed.merge(s.memberId, p, Long::plus) }
                val inItem = item.shares.map { it.memberId }.toSet()
                val byParts = item.shares.all { it.parts != null }
                Card(colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surfaceContainerLow)) {
                    Column(Modifier.padding(12.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                        Row(verticalAlignment = Alignment.CenterVertically) {
                            Text(
                                item.label ?: t.title,
                                style = MaterialTheme.typography.titleSmall,
                                modifier = Modifier.weight(1f),
                            )
                            Text(fmt(item.totalMinor), style = MaterialTheme.typography.titleSmall)
                        }
                        // Active people, plus inactive ones still in the item (they can only leave).
                        val people = d.members.filter { it.active || it.id in inItem }
                        FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                            people.forEach { m ->
                                val on = m.id in inItem
                                val last = on && inItem.size == 1
                                // The last person stays shown as selected; tapping them does nothing.
                                FilterChip(
                                    selected = on,
                                    enabled = byParts && (on || m.active),
                                    onClick = { if (!last) vm.setItemMember(id, index, m.id, !on) },
                                    leadingIcon = { Avatar(initials(m.displayName), m.id, size = 20.dp) },
                                    label = { Text(m.displayName) },
                                )
                            }
                        }
                        if (byParts && inItem.size > 1 && parts.isNotEmpty()) {
                            Text(
                                // Cents left over go to some people, so a range when they differ.
                                stringResource(
                                    R.string.each_pays,
                                    if (parts.min() == parts.max()) fmt(parts.max()) else fmt(parts.min()) + "–" + fmt(parts.max()),
                                ),
                                style = MaterialTheme.typography.bodySmall,
                                color = MaterialTheme.colorScheme.onSurfaceVariant,
                            )
                        } else if (!byParts) {
                            Text(
                                stringResource(R.string.exact_amounts_item),
                                style = MaterialTheme.typography.bodySmall,
                                color = MaterialTheme.colorScheme.onSurfaceVariant,
                            )
                        }
                    }
                }
            }

            HorizontalDivider()
            Text(stringResource(R.string.per_person), style = MaterialTheme.typography.titleMedium)
            d.members.filter { it.id in owed }.forEach { m ->
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Avatar(initials(m.displayName), m.id, size = 32.dp)
                    Text(
                        if (m.id == d.meId) stringResource(R.string.me_suffix, m.displayName) else m.displayName,
                        Modifier.padding(start = 12.dp).weight(1f),
                        style = MaterialTheme.typography.bodyLarge,
                    )
                    Text(fmt(owed[m.id] ?: 0), style = MaterialTheme.typography.bodyLarge, fontWeight = FontWeight.SemiBold)
                }
            }
            if (active.isEmpty()) Text(stringResource(R.string.error_nobody))
        }
    }
}
