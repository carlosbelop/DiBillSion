package com.dibillsion.app.ui

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.gestures.detectTransformGestures
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.height
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
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.filled.Delete
import androidx.compose.material3.Card
import androidx.compose.material3.CircularProgressIndicator
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
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
import androidx.compose.ui.res.pluralStringResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import com.dibillsion.app.AppViewModel
import com.dibillsion.app.GroupDetail
import com.dibillsion.app.Money
import com.dibillsion.app.R
import com.dibillsion.core.TransactionDto
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.TextButton
import androidx.compose.runtime.mutableStateMapOf
import com.dibillsion.app.Split
import com.dibillsion.app.SplitMode
import com.dibillsion.core.ItemDto
import com.dibillsion.core.MemberDto
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
    var splitting by remember { mutableStateOf<Int?>(null) }
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
    splitting?.let { index ->
        t.items.getOrNull(index)?.let { item ->
            ItemSplitDialog(
                title = item.label ?: t.title,
                item = item,
                people = d.members.filter { it.active || item.shares.any { s -> s.memberId == it.id } },
                exponent = exponent,
                currency = t.currency,
                vm = vm,
                onApply = { parts ->
                    val current = item.shares.associate { it.memberId to (it.parts ?: 0u) }
                    vm.setItemParts(id, index, parts, current)
                    splitting = null
                },
                onDismiss = { splitting = null },
            )
        }
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
            t.receiptImageId?.let { ReceiptPhoto(it, vm) }
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
                        val even = item.shares.map { it.parts }.distinct().size <= 1
                        if (byParts) {
                            Row(verticalAlignment = Alignment.CenterVertically) {
                                Text(
                                    when {
                                        !even -> stringResource(R.string.custom_split)
                                        inItem.size > 1 && parts.isNotEmpty() -> stringResource(
                                            R.string.each_pays,
                                            if (parts.min() == parts.max()) fmt(parts.max()) else fmt(parts.min()) + "–" + fmt(parts.max()),
                                        )
                                        else -> ""
                                    },
                                    style = MaterialTheme.typography.bodySmall,
                                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                                    modifier = Modifier.weight(1f),
                                )
                                TextButton(onClick = { splitting = index }) { Text(stringResource(R.string.split_item)) }
                            }
                        }
                        if (!byParts) {
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

/** The receipt photo: a thumbnail that opens full screen, where it can be pinched and moved. */
@Composable
private fun ReceiptPhoto(imageId: String, vm: AppViewModel) {
    var image by remember(imageId) { mutableStateOf<ImageBitmap?>(null) }
    var failed by remember(imageId) { mutableStateOf(false) }
    var open by remember { mutableStateOf(false) }
    LaunchedEffect(imageId) {
        image = vm.receiptImage(imageId)
        failed = image == null
    }
    Card(
        onClick = { if (image != null) open = true },
        modifier = Modifier.fillMaxWidth().height(160.dp),
        colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surfaceContainerLow),
    ) {
        Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
            val loaded = image
            when {
                loaded != null -> Image(
                    loaded,
                    contentDescription = stringResource(R.string.receipt_photo),
                    contentScale = ContentScale.Crop,
                    modifier = Modifier.fillMaxSize(),
                )
                failed -> Text(
                    stringResource(R.string.receipt_photo_unavailable),
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    textAlign = TextAlign.Center,
                    modifier = Modifier.padding(16.dp),
                )
                else -> CircularProgressIndicator()
            }
        }
    }
    val loaded = image
    if (open && loaded != null) {
        Dialog(onDismissRequest = { open = false }, properties = DialogProperties(usePlatformDefaultWidth = false)) {
            var scale by remember { mutableFloatStateOf(1f) }
            var offset by remember { mutableStateOf(Offset.Zero) }
            Box(
                Modifier.fillMaxSize().background(Color.Black)
                    .pointerInput(Unit) {
                        detectTransformGestures { _, pan, zoom, _ ->
                            scale = (scale * zoom).coerceIn(1f, 6f)
                            offset = if (scale == 1f) Offset.Zero else offset + pan
                        }
                    },
            ) {
                Image(
                    loaded,
                    contentDescription = stringResource(R.string.receipt_photo),
                    contentScale = ContentScale.Fit,
                    modifier = Modifier.fillMaxSize().graphicsLayer {
                        scaleX = scale
                        scaleY = scale
                        translationX = offset.x
                        translationY = offset.y
                    },
                )
                IconButton(onClick = { open = false }, modifier = Modifier.align(Alignment.TopEnd).padding(16.dp)) {
                    Icon(Icons.Filled.Close, stringResource(R.string.close), tint = Color.White)
                }
            }
        }
    }
}

/** Equal, parts or percentage for one item; applied per person (ADR 0009). */
@Composable
private fun ItemSplitDialog(
    title: String,
    item: ItemDto,
    people: List<MemberDto>,
    exponent: Int,
    currency: String,
    vm: AppViewModel,
    onApply: (Map<String, UInt>) -> Unit,
    onDismiss: () -> Unit,
) {
    val even = item.shares.map { it.parts }.distinct().size <= 1
    var mode by remember { mutableStateOf(if (even) SplitMode.EQUAL else SplitMode.PARTS) }
    var among by remember { mutableStateOf(item.shares.map { it.memberId }.toSet()) }
    val values = remember {
        mutableStateMapOf<String, String>().apply {
            if (!even) {
                // Shown in their smallest form: 5000/3000/2000 reads as 5/3/2.
                val all = item.shares.mapNotNull { it.parts?.toLong() }
                val g = all.fold(0L) { acc, v -> java.math.BigInteger.valueOf(acc).gcd(java.math.BigInteger.valueOf(v)).toLong() }
                    .coerceAtLeast(1)
                item.shares.forEach { s -> s.parts?.let { put(s.memberId, (it.toLong() / g).toString()) } }
            }
        }
    }
    val split = Split.shares(mode, people.map { it.id }, among, values, item.totalMinor, exponent)
    val preview = if (split.problem == null) vm.previewShares(item.totalMinor, split.shares) else emptyList()
    val shareOf = split.shares.map { it.memberId }.zip(preview).toMap()
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(stringResource(R.string.split_item_title, title), maxLines = 2) },
        text = {
            Column(Modifier.verticalScroll(rememberScrollState()), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                SplitModeSelector(mode, withAmounts = false) { mode = it }
                people.forEach { m ->
                    SplitRow(
                        name = m.displayName,
                        id = m.id,
                        mode = mode,
                        included = m.id in among,
                        onIncluded = { on -> among = if (on) among + m.id else among - m.id },
                        value = values[m.id].orEmpty(),
                        onValue = { values[m.id] = it },
                        share = shareOf[m.id]?.let { Money.format(it, exponent, currency) },
                        unit = null,
                        compact = true,
                    )
                }
                splitProblemText(split.problem, exponent, currency)?.let {
                    Text(it, color = MaterialTheme.colorScheme.error)
                }
            }
        },
        confirmButton = {
            TextButton(
                enabled = split.problem == null,
                onClick = {
                    val chosen = split.shares.associate { it.memberId to (it.parts ?: 0u) }
                    onApply(people.associate { it.id to (chosen[it.id] ?: 0u) })
                },
            ) { Text(stringResource(R.string.apply)) }
        },
        dismissButton = { TextButton(onClick = onDismiss) { Text(stringResource(R.string.cancel)) } },
    )
}
