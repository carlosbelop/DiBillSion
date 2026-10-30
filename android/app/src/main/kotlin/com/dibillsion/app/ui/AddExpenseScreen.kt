package com.dibillsion.app.ui

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.filled.DateRange
import androidx.compose.material3.Checkbox
import androidx.compose.material3.DatePicker
import androidx.compose.material3.DatePickerDialog
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.ExposedDropdownMenuBox
import androidx.compose.material3.ExposedDropdownMenuDefaults
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.MenuAnchorType
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.rememberDatePickerState
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
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import com.dibillsion.app.AppViewModel
import com.dibillsion.app.GroupDetail
import com.dibillsion.app.Money
import com.dibillsion.app.R
import java.math.BigDecimal
import java.math.RoundingMode
import java.time.LocalDate
import java.time.format.DateTimeFormatter
import java.time.format.FormatStyle

private const val DAY_MS = 86_400_000L

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun AddExpenseScreen(d: GroupDetail, vm: AppViewModel, onDone: () -> Unit) {
    val active = d.members.filter { it.active }
    var title by rememberSaveable { mutableStateOf("") }
    var amount by rememberSaveable { mutableStateOf("") }
    var currency by rememberSaveable { mutableStateOf(d.group.baseCurrency) }
    var rate by rememberSaveable { mutableStateOf("") }
    var payer by rememberSaveable { mutableStateOf(active.firstOrNull()?.id) }
    var among by rememberSaveable { mutableStateOf(active.map { it.id }) }
    var day by rememberSaveable { mutableIntStateOf(LocalDate.now().toEpochDay().toInt()) }
    var pickingDate by remember { mutableStateOf(false) }
    var discarding by remember { mutableStateOf(false) }

    val foreign = currency != d.group.baseCurrency
    val exponent = vm.exponentOf(currency)
    val minor = exponent?.let { Money.parse(amount, it) }?.takeIf { it > 0 }
    val fxRate = if (foreign) Money.parseRate(rate) else null
    val problem = when {
        amount.isNotBlank() && minor == null -> R.string.error_amount
        foreign && rate.isNotBlank() && fxRate == null -> R.string.error_rate
        among.isEmpty() -> R.string.error_nobody
        else -> null
    }
    val canSave = title.isNotBlank() && minor != null && payer != null && among.isNotEmpty() && (!foreign || fxRate != null)
    val dirty = title.isNotBlank() || amount.isNotBlank()
    val close = { if (dirty) discarding = true else onDone() }
    BackHandler(onBack = close)

    // Shares in the expense currency, in member order, exactly as the core will split them.
    val included = active.map { it.id }.filter { it in among }
    val preview = minor?.let { vm.previewEqualSplit(it, included) }.orEmpty()
    val shareOf = included.zip(preview).toMap()

    if (discarding) {
        ConfirmDialog(
            text = stringResource(R.string.discard_changes),
            confirm = stringResource(R.string.discard),
            onConfirm = onDone,
            onDismiss = { discarding = false },
        )
    }
    if (pickingDate) {
        val picker = rememberDatePickerState(initialSelectedDateMillis = day * DAY_MS)
        DatePickerDialog(
            onDismissRequest = { pickingDate = false },
            confirmButton = {
                TextButton(onClick = {
                    picker.selectedDateMillis?.let { day = (it / DAY_MS).toInt() }
                    pickingDate = false
                }) { Text(stringResource(R.string.ok)) }
            },
            dismissButton = { TextButton(onClick = { pickingDate = false }) { Text(stringResource(R.string.cancel)) } },
        ) { DatePicker(picker) }
    }

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text(stringResource(R.string.new_expense)) },
                navigationIcon = { IconButton(onClick = close) { Icon(Icons.Filled.Close, stringResource(R.string.cancel)) } },
                actions = {
                    TextButton(enabled = canSave, onClick = {
                        vm.addExpense(title, minor ?: return@TextButton, currency, fxRate, payer ?: return@TextButton, among, day)
                        onDone()
                    }) { Text(stringResource(R.string.save)) }
                },
            )
        },
    ) { padding ->
        Column(
            Modifier.fillMaxSize().padding(padding).imePadding().verticalScroll(rememberScrollState()).padding(16.dp),
            verticalArrangement = Arrangement.spacedBy(16.dp),
        ) {
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                OutlinedTextField(
                    amount, { amount = it },
                    placeholder = { Text("0", style = MaterialTheme.typography.headlineMedium) },
                    label = { Text(stringResource(R.string.amount)) },
                    textStyle = MaterialTheme.typography.headlineMedium.copy(fontWeight = FontWeight.SemiBold),
                    keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Decimal),
                    singleLine = true,
                    isError = amount.isNotBlank() && minor == null,
                    modifier = Modifier.weight(1f),
                )
                OutlinedTextField(
                    currency, { currency = it.uppercase().filter(Char::isLetter).take(3) },
                    label = { Text(stringResource(R.string.currency)) },
                    singleLine = true,
                    isError = exponent == null,
                    keyboardOptions = KeyboardOptions(capitalization = KeyboardCapitalization.Characters),
                    modifier = Modifier.width(104.dp),
                )
            }
            if (foreign) {
                OutlinedTextField(
                    rate, { rate = it },
                    label = { Text(stringResource(R.string.rate, currency, d.group.baseCurrency)) },
                    supportingText = {
                        val converted = minor?.let { m -> fxRate?.let { approx(m, exponent ?: 2, it, d.exponent) } }
                        converted?.let { Text(stringResource(R.string.converted, Money.format(it, d.exponent, d.group.baseCurrency))) }
                    },
                    keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Decimal),
                    singleLine = true,
                    modifier = Modifier.fillMaxWidth(),
                )
            }
            OutlinedTextField(
                title, { title = it },
                label = { Text(stringResource(R.string.title)) },
                singleLine = true,
                keyboardOptions = KeyboardOptions(capitalization = KeyboardCapitalization.Sentences),
                modifier = Modifier.fillMaxWidth(),
            )
            Row(horizontalArrangement = Arrangement.spacedBy(12.dp), verticalAlignment = Alignment.CenterVertically) {
                PayerPicker(d, active.map { it.id to it.displayName }, payer, { payer = it }, Modifier.weight(1f))
                OutlinedButton(onClick = { pickingDate = true }) {
                    Icon(Icons.Filled.DateRange, null)
                    Text(
                        LocalDate.ofEpochDay(day.toLong()).format(DateTimeFormatter.ofLocalizedDate(FormatStyle.SHORT)),
                        Modifier.padding(start = 8.dp),
                    )
                }
            }

            HorizontalDivider()
            Row(verticalAlignment = Alignment.CenterVertically) {
                Text(stringResource(R.string.split_between), style = MaterialTheme.typography.titleMedium, modifier = Modifier.weight(1f))
                val all = among.size == active.size
                TextButton(onClick = { among = if (all) emptyList() else active.map { it.id } }) {
                    Text(stringResource(if (all) R.string.select_none else R.string.select_all))
                }
            }
            active.forEach { m ->
                val on = m.id in among
                Row(
                    Modifier.fillMaxWidth().clickable { among = if (on) among - m.id else among + m.id }.padding(vertical = 4.dp),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    Checkbox(checked = on, onCheckedChange = { among = if (it) among + m.id else among - m.id })
                    Avatar(initials(m.displayName), m.id, size = 32.dp)
                    Text(m.displayName, Modifier.padding(start = 12.dp).weight(1f), style = MaterialTheme.typography.bodyLarge)
                    Text(
                        shareOf[m.id]?.let { Money.format(it, exponent ?: 2, currency) } ?: "—",
                        style = MaterialTheme.typography.bodyLarge,
                        color = if (on) MaterialTheme.colorScheme.onSurface else MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
            }
            problem?.let { Text(stringResource(it), color = MaterialTheme.colorScheme.error) }
        }
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun PayerPicker(
    d: GroupDetail,
    options: List<Pair<String, String>>,
    selected: String?,
    onSelect: (String) -> Unit,
    modifier: Modifier = Modifier,
) {
    var open by remember { mutableStateOf(false) }
    ExposedDropdownMenuBox(expanded = open, onExpandedChange = { open = it }, modifier = modifier) {
        OutlinedTextField(
            value = selected?.let(d::name).orEmpty(),
            onValueChange = {},
            readOnly = true,
            label = { Text(stringResource(R.string.payer)) },
            trailingIcon = { ExposedDropdownMenuDefaults.TrailingIcon(open) },
            singleLine = true,
            modifier = Modifier.menuAnchor(MenuAnchorType.PrimaryNotEditable).fillMaxWidth(),
        )
        ExposedDropdownMenu(expanded = open, onDismissRequest = { open = false }) {
            options.forEach { (id, name) ->
                DropdownMenuItem(
                    leadingIcon = { Avatar(initials(name), id, size = 28.dp) },
                    text = { Text(name) },
                    onClick = {
                        onSelect(id)
                        open = false
                    },
                )
            }
        }
    }
}

/** Display-only conversion; the core does the stored one (ADR 0002). */
private fun approx(minor: Long, exponent: Int, rate: String, baseExponent: Int): Long =
    BigDecimal.valueOf(minor, exponent).multiply(BigDecimal(rate))
        .setScale(baseExponent, RoundingMode.HALF_EVEN).unscaledValue().toLong()
