package com.dibillsion.app.ui

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.Checkbox
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FilterChip
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
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import com.dibillsion.app.AppViewModel
import com.dibillsion.app.GroupDetail
import com.dibillsion.app.Money
import com.dibillsion.app.R

@OptIn(ExperimentalMaterial3Api::class, ExperimentalLayoutApi::class)
@Composable
fun AddExpenseScreen(d: GroupDetail, vm: AppViewModel, onDone: () -> Unit) {
    val active = d.members.filter { it.active }
    var title by rememberSaveable { mutableStateOf("") }
    var amount by rememberSaveable { mutableStateOf("") }
    var currency by rememberSaveable { mutableStateOf(d.group.baseCurrency) }
    var rate by rememberSaveable { mutableStateOf("") }
    var payer by rememberSaveable { mutableStateOf(active.firstOrNull()?.id) }
    var among by rememberSaveable { mutableStateOf(active.map { it.id }) }

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

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text(stringResource(R.string.add_expense)) },
                navigationIcon = { TextButton(onClick = onDone) { Text(stringResource(R.string.cancel)) } },
            )
        },
    ) { padding ->
        Column(
            Modifier.fillMaxSize().padding(padding).padding(16.dp).verticalScroll(rememberScrollState()),
            verticalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            OutlinedTextField(title, { title = it }, label = { Text(stringResource(R.string.title)) }, singleLine = true, modifier = Modifier.fillMaxWidth())
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                OutlinedTextField(
                    amount, { amount = it },
                    label = { Text(stringResource(R.string.amount)) },
                    keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Decimal),
                    singleLine = true,
                    modifier = Modifier.weight(2f),
                )
                OutlinedTextField(
                    currency, { currency = it.uppercase().take(3) },
                    label = { Text(stringResource(R.string.currency)) },
                    singleLine = true,
                    modifier = Modifier.weight(1f),
                )
            }
            if (foreign) {
                OutlinedTextField(
                    rate, { rate = it },
                    label = { Text(stringResource(R.string.rate, currency, d.group.baseCurrency)) },
                    keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Decimal),
                    singleLine = true,
                    modifier = Modifier.fillMaxWidth(),
                )
            }
            Text(stringResource(R.string.payer), style = MaterialTheme.typography.titleSmall)
            FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                active.forEach { m ->
                    FilterChip(selected = payer == m.id, onClick = { payer = m.id }, label = { Text(m.displayName) })
                }
            }
            Text(stringResource(R.string.split_between), style = MaterialTheme.typography.titleSmall)
            active.forEach { m ->
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Checkbox(
                        checked = m.id in among,
                        onCheckedChange = { on -> among = if (on) among + m.id else among - m.id },
                    )
                    Text(m.displayName)
                }
            }
            problem?.let { Text(stringResource(it), color = MaterialTheme.colorScheme.error) }
            Button(
                enabled = canSave,
                onClick = {
                    vm.addExpense(title, minor ?: return@Button, currency, fxRate, payer ?: return@Button, among)
                    onDone()
                },
                modifier = Modifier.fillMaxWidth(),
            ) { Text(stringResource(R.string.save)) }
        }
    }
}
