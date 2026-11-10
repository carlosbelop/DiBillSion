package com.dibillsion.app.ui

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.Checkbox
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.SegmentedButton
import androidx.compose.material3.SegmentedButtonDefaults
import androidx.compose.material3.SingleChoiceSegmentedButtonRow
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import com.dibillsion.app.Money
import com.dibillsion.app.R
import com.dibillsion.app.SplitMode
import com.dibillsion.app.SplitProblem
import java.math.BigDecimal

/** Equal / Parts / % (/ Amounts). */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun SplitModeSelector(mode: SplitMode, withAmounts: Boolean, onMode: (SplitMode) -> Unit) {
    val modes = listOfNotNull(
        SplitMode.EQUAL to R.string.split_equal,
        SplitMode.PARTS to R.string.split_parts,
        SplitMode.PERCENT to R.string.split_percent,
        if (withAmounts) SplitMode.AMOUNT to R.string.split_amounts else null,
    )
    SingleChoiceSegmentedButtonRow(Modifier.fillMaxWidth()) {
        modes.forEachIndexed { i, (m, label) ->
            SegmentedButton(
                selected = mode == m,
                onClick = { onMode(m) },
                shape = SegmentedButtonDefaults.itemShape(i, modes.size),
                label = { Text(stringResource(label), maxLines = 1) },
            )
        }
    }
}

/** One person in a split: a checkbox for equal splits, otherwise a number to type. */
@Composable
fun SplitRow(
    name: String,
    id: String,
    mode: SplitMode,
    included: Boolean,
    onIncluded: (Boolean) -> Unit,
    value: String,
    onValue: (String) -> Unit,
    share: String?,
    unit: String?,
    compact: Boolean = false,
) {
    Row(
        Modifier.fillMaxWidth()
            .then(if (mode == SplitMode.EQUAL) Modifier.clickable { onIncluded(!included) } else Modifier)
            .padding(vertical = 4.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        if (mode == SplitMode.EQUAL) Checkbox(checked = included, onCheckedChange = onIncluded)
        if (!compact) Avatar(initials(name), id, size = 32.dp)
        Text(
            name,
            Modifier.padding(start = if (compact) 4.dp else 12.dp).weight(1f),
            style = if (compact) MaterialTheme.typography.bodyMedium else MaterialTheme.typography.bodyLarge,
            maxLines = 1,
        )
        if (mode != SplitMode.EQUAL) {
            OutlinedTextField(
                value, onValue,
                placeholder = { Text("0") },
                suffix = {
                    Text(
                        when (mode) {
                            SplitMode.PERCENT -> "%"
                            SplitMode.PARTS -> "×"
                            else -> unit.orEmpty()
                        },
                    )
                },
                singleLine = true,
                keyboardOptions = KeyboardOptions(
                    keyboardType = if (mode == SplitMode.PARTS) KeyboardType.Number else KeyboardType.Decimal,
                ),
                textStyle = MaterialTheme.typography.bodyLarge.copy(textAlign = TextAlign.End),
                modifier = Modifier.width(
                    when {
                        compact -> 84.dp
                        mode == SplitMode.AMOUNT -> 132.dp
                        else -> 104.dp
                    },
                ).padding(horizontal = if (compact) 4.dp else 8.dp),
            )
        }
        Text(
            share ?: "—",
            style = if (compact) MaterialTheme.typography.bodySmall else MaterialTheme.typography.bodyMedium,
            color = if (share != null) MaterialTheme.colorScheme.onSurface else MaterialTheme.colorScheme.onSurfaceVariant,
            textAlign = TextAlign.End,
            modifier = Modifier.width(if (compact) 72.dp else 88.dp),
        )
    }
}

/** What's wrong with a split, in words, or null. */
@Composable
fun splitProblemText(problem: SplitProblem?, exponent: Int, currency: String): String? = when (problem) {
    null -> null
    SplitProblem.Nobody -> stringResource(R.string.error_nobody)
    SplitProblem.BadValue -> stringResource(R.string.error_split_value)
    is SplitProblem.PercentOff -> {
        val pct = BigDecimal.valueOf(kotlin.math.abs(problem.missing), 2).stripTrailingZeros().toPlainString()
        stringResource(if (problem.missing > 0) R.string.error_percent_left else R.string.error_percent_over, pct)
    }
    is SplitProblem.AmountOff -> {
        val amount = Money.format(kotlin.math.abs(problem.missing), exponent, currency)
        stringResource(if (problem.missing > 0) R.string.error_amount_left else R.string.error_amount_over, amount)
    }
}
