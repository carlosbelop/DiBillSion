package com.dibillsion.app.ui

import androidx.activity.compose.BackHandler
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
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.CheckCircle
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.filled.DateRange
import androidx.compose.material.icons.filled.Delete
import androidx.compose.material.icons.filled.Warning
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.DatePicker
import androidx.compose.material3.DatePickerDialog
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FilterChip
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.rememberDatePickerState
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
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import com.dibillsion.app.AppViewModel
import com.dibillsion.app.GroupDetail
import com.dibillsion.app.Money
import com.dibillsion.app.R
import com.dibillsion.app.ReceiptDraft
import java.time.LocalDate
import java.time.format.DateTimeFormatter
import java.time.format.FormatStyle

private const val DAY_MS = 86_400_000L

/** Shown while the server's model reads the photo. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun ReadingReceiptScreen(onCancel: () -> Unit) {
    BackHandler(onBack = onCancel)
    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text(stringResource(R.string.scan_receipt)) },
                navigationIcon = { IconButton(onClick = onCancel) { Icon(Icons.Filled.Close, stringResource(R.string.cancel)) } },
            )
        },
    ) { padding ->
        Box(Modifier.fillMaxSize().padding(padding), contentAlignment = Alignment.Center) {
            Column(horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.spacedBy(16.dp)) {
                CircularProgressIndicator()
                Text(stringResource(R.string.reading_receipt), style = MaterialTheme.typography.titleMedium)
                Text(
                    stringResource(R.string.reading_receipt_hint),
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    textAlign = TextAlign.Center,
                    modifier = Modifier.padding(horizontal = 32.dp),
                )
            }
        }
    }
}

/**
 * Review of a scanned receipt (SPEC flow 8): fix what the model misread, choose who shares
 * each item (everyone by default), and save it as one expense with an item per line.
 */
@OptIn(ExperimentalMaterial3Api::class, ExperimentalLayoutApi::class)
@Composable
fun ReceiptReviewScreen(d: GroupDetail, draft: ReceiptDraft, vm: AppViewModel) {
    val active = d.members.filter { it.active }
    val everyone = active.map { it.id }.toSet()
    var discarding by remember { mutableStateOf(false) }
    var pickingDate by remember { mutableStateOf(false) }

    val exponent = vm.exponentOf(draft.currency)
    val foreign = draft.currency != d.group.baseCurrency
    val fxRate = if (foreign) Money.parseRate(draft.rate) else null
    val problem = exponent?.let(draft::problem)
    val canSave = exponent != null && problem == null && draft.title.isNotBlank() && draft.payerId != null &&
        (!foreign || fxRate != null)
    val close = { discarding = true }
    BackHandler(onBack = close)

    if (discarding) {
        ConfirmDialog(
            text = stringResource(R.string.discard_receipt),
            confirm = stringResource(R.string.discard),
            onConfirm = vm::closeScan,
            onDismiss = { discarding = false },
        )
    }
    if (pickingDate) {
        val picker = rememberDatePickerState(initialSelectedDateMillis = draft.dateDays * DAY_MS)
        DatePickerDialog(
            onDismissRequest = { pickingDate = false },
            confirmButton = {
                TextButton(onClick = {
                    picker.selectedDateMillis?.let { ms -> vm.editReceipt { it.copy(dateDays = (ms / DAY_MS).toInt()) } }
                    pickingDate = false
                }) { Text(stringResource(R.string.ok)) }
            },
            dismissButton = { TextButton(onClick = { pickingDate = false }) { Text(stringResource(R.string.cancel)) } },
        ) { DatePicker(picker) }
    }

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text(stringResource(R.string.check_receipt)) },
                navigationIcon = { IconButton(onClick = close) { Icon(Icons.Filled.Close, stringResource(R.string.cancel)) } },
                actions = { TextButton(enabled = canSave, onClick = vm::saveReceipt) { Text(stringResource(R.string.save)) } },
            )
        },
    ) { padding ->
        Column(
            Modifier.fillMaxSize().padding(padding).imePadding().verticalScroll(rememberScrollState()).padding(16.dp),
            verticalArrangement = Arrangement.spacedBy(16.dp),
        ) {
            OutlinedTextField(
                draft.title, { t -> vm.editReceipt { it.copy(title = t) } },
                label = { Text(stringResource(R.string.title)) },
                singleLine = true,
                keyboardOptions = KeyboardOptions(capitalization = KeyboardCapitalization.Sentences),
                modifier = Modifier.fillMaxWidth(),
            )
            Row(horizontalArrangement = Arrangement.spacedBy(12.dp), verticalAlignment = Alignment.CenterVertically) {
                PayerPicker(
                    d, active.map { it.id to it.displayName }, draft.payerId,
                    { id -> vm.editReceipt { it.copy(payerId = id) } }, Modifier.weight(1f),
                )
                OutlinedButton(onClick = { pickingDate = true }) {
                    Icon(Icons.Filled.DateRange, null)
                    Text(
                        LocalDate.ofEpochDay(draft.dateDays.toLong()).format(DateTimeFormatter.ofLocalizedDate(FormatStyle.SHORT)),
                        Modifier.padding(start = 8.dp),
                    )
                }
            }
            Row(horizontalArrangement = Arrangement.spacedBy(12.dp), verticalAlignment = Alignment.CenterVertically) {
                OutlinedTextField(
                    draft.currency, { c -> vm.editReceipt { it.copy(currency = c.uppercase().filter(Char::isLetter).take(3)) } },
                    label = { Text(stringResource(R.string.currency)) },
                    singleLine = true,
                    isError = exponent == null,
                    keyboardOptions = KeyboardOptions(capitalization = KeyboardCapitalization.Characters),
                    modifier = Modifier.width(104.dp),
                )
                if (foreign) {
                    OutlinedTextField(
                        draft.rate, { r -> vm.editReceipt { it.copy(rate = r) } },
                        label = { Text(stringResource(R.string.rate, draft.currency, d.group.baseCurrency)) },
                        keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Decimal),
                        singleLine = true,
                        isError = draft.rate.isNotBlank() && fxRate == null,
                        modifier = Modifier.weight(1f),
                    )
                }
            }
            if (exponent != null) TotalCheck(draft, exponent)

            HorizontalDivider()
            Text(stringResource(R.string.receipt_items_hint), style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
            draft.lines.forEach { line ->
                Card(colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surfaceContainerLow)) {
                    Column(Modifier.padding(12.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                            OutlinedTextField(
                                line.label, { t -> vm.editReceipt { it.update(line.key) { l -> l.copy(label = t) } } },
                                placeholder = { Text(stringResource(R.string.item)) },
                                singleLine = true,
                                modifier = Modifier.weight(1f),
                            )
                            OutlinedTextField(
                                line.amount, { a -> vm.editReceipt { it.update(line.key) { l -> l.copy(amount = a) } } },
                                placeholder = { Text("0") },
                                singleLine = true,
                                isError = exponent != null && Money.parse(line.amount, exponent) == null,
                                keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Decimal),
                                modifier = Modifier.width(112.dp),
                            )
                            IconButton(onClick = { vm.editReceipt { it.remove(line.key) } }) {
                                Icon(Icons.Filled.Delete, stringResource(R.string.delete_item))
                            }
                        }
                        if (line.discountMinor != 0L && exponent != null) {
                            Text(
                                stringResource(R.string.includes_discount, Money.format(-line.discountMinor, exponent, draft.currency)),
                                style = MaterialTheme.typography.bodySmall,
                                color = MaterialTheme.colorScheme.onSurfaceVariant,
                            )
                        }
                        FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                            val all = line.members.containsAll(everyone)
                            FilterChip(
                                selected = all,
                                onClick = { vm.editReceipt { it.update(line.key) { l -> l.copy(members = if (all) emptySet() else everyone) } } },
                                label = { Text(stringResource(R.string.select_all)) },
                            )
                            active.forEach { m ->
                                val on = m.id in line.members
                                FilterChip(
                                    selected = on && !all,
                                    onClick = {
                                        // From "everyone", a tap picks just that person: the usual case.
                                        val next = when {
                                            all -> setOf(m.id)
                                            on -> line.members - m.id
                                            else -> line.members + m.id
                                        }
                                        vm.editReceipt { it.update(line.key) { l -> l.copy(members = next) } }
                                    },
                                    leadingIcon = { Avatar(initials(m.displayName), m.id, size = 20.dp) },
                                    label = { Text(m.displayName) },
                                )
                            }
                        }
                    }
                }
            }
            OutlinedButton(onClick = { vm.editReceipt { it.add(everyone) } }) {
                Icon(Icons.Filled.Add, null)
                Text(stringResource(R.string.add_item), Modifier.padding(start = 8.dp))
            }

            if (exponent != null) {
                val shares = vm.receiptShares(draft, d.members.map { it.id })
                if (shares.isNotEmpty()) {
                    HorizontalDivider()
                    Text(stringResource(R.string.per_person), style = MaterialTheme.typography.titleMedium)
                    active.filter { it.id in shares }.forEach { m ->
                        Row(verticalAlignment = Alignment.CenterVertically) {
                            Avatar(initials(m.displayName), m.id, size = 32.dp)
                            Text(m.displayName, Modifier.padding(start = 12.dp).weight(1f), style = MaterialTheme.typography.bodyLarge)
                            Text(
                                Money.format(shares[m.id] ?: 0, exponent, draft.currency),
                                style = MaterialTheme.typography.bodyLarge,
                                fontWeight = FontWeight.SemiBold,
                            )
                        }
                    }
                }
            }
            problem?.let {
                val res = when (it) {
                    ReceiptDraft.Problem.AMOUNT -> R.string.error_item_amount
                    ReceiptDraft.Problem.NOBODY -> R.string.error_item_nobody
                    ReceiptDraft.Problem.EMPTY -> R.string.error_amount
                }
                Text(stringResource(res), color = MaterialTheme.colorScheme.error)
            }
            Text(
                pluralStringResource(R.plurals.scans_left, draft.scansLeft, draft.scansLeft),
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
    }
}

/** Whether the items add up to the printed total: the one check the user can't skip reading. */
@Composable
private fun TotalCheck(draft: ReceiptDraft, exponent: Int) {
    val sum = draft.sum(exponent) ?: return
    val printed = draft.printedTotalMinor
    val ok = printed == sum
    val fmt = { m: Long -> Money.format(m, exponent, draft.currency) }
    val text = when {
        printed == null -> stringResource(R.string.receipt_no_total, fmt(sum))
        ok -> stringResource(R.string.receipt_adds_up, fmt(sum))
        else -> stringResource(R.string.receipt_mismatch, fmt(sum), fmt(printed))
    }
    val color = if (ok) MaterialTheme.colorScheme.secondaryContainer else MaterialTheme.colorScheme.errorContainer
    Card(colors = CardDefaults.cardColors(containerColor = color)) {
        Row(Modifier.padding(12.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
            Icon(if (ok) Icons.Filled.CheckCircle else Icons.Filled.Warning, null, Modifier.size(24.dp))
            Text(text, style = MaterialTheme.typography.bodyMedium)
        }
    }
}
