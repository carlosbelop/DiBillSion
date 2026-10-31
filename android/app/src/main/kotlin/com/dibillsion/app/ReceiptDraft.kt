package com.dibillsion.app

import com.dibillsion.core.ItemDto
import com.dibillsion.core.ScannedReceiptDto
import com.dibillsion.core.ShareDto

/** One editable line of a scanned receipt. [members] share it equally. */
data class ReceiptLine(
    val key: Long,
    val label: String,
    val amount: String,
    val members: Set<String>,
    /** Discount folded into this line by the core (zero or negative), shown next to it. */
    val discountMinor: Long = 0,
)

/**
 * The review screen's state for a scanned receipt (SPEC flow 8). Lives in the view model so
 * edits survive rotation; amounts stay text until saving, like the expense editor.
 */
data class ReceiptDraft(
    val title: String,
    val payerId: String?,
    val currency: String,
    val rate: String,
    val dateDays: Int,
    val lines: List<ReceiptLine>,
    /** The total printed on the receipt, in [currency]. */
    val printedTotalMinor: Long?,
    val scansLeft: Int,
) {
    enum class Problem { AMOUNT, NOBODY, EMPTY }

    /** Each line in minor units; null where the text isn't a valid amount (zero is fine). */
    fun amounts(exponent: Int): List<Long?> = lines.map { Money.parse(it.amount, exponent) }

    /** Sum of the lines, or null while one of them is invalid. */
    fun sum(exponent: Int): Long? = amounts(exponent).let { a -> if (a.any { it == null }) null else a.sumOf { it ?: 0 } }

    fun problem(exponent: Int): Problem? {
        val amounts = amounts(exponent)
        return when {
            amounts.any { it == null } -> Problem.AMOUNT
            lines.zip(amounts).any { (line, a) -> (a ?: 0) > 0 && line.members.isEmpty() } -> Problem.NOBODY
            amounts.none { (it ?: 0) > 0 } -> Problem.EMPTY
            else -> null
        }
    }

    /** The expense's items: free lines are dropped, everyone else shares equally. */
    fun items(exponent: Int, memberOrder: List<String>): List<ItemDto> =
        lines.zip(amounts(exponent)).mapNotNull { (line, amount) ->
            val minor = amount?.takeIf { it > 0 } ?: return@mapNotNull null
            val shares = memberOrder.filter { it in line.members }.map { ShareDto(it, 1u, null) }
            ItemDto(line.label.trim().ifEmpty { null }, minor, shares)
        }

    fun update(key: Long, change: (ReceiptLine) -> ReceiptLine) = copy(lines = lines.map { if (it.key == key) change(it) else it })

    fun remove(key: Long) = copy(lines = lines.filterNot { it.key == key })

    fun add(everyone: Set<String>) = copy(lines = lines + ReceiptLine((lines.maxOfOrNull { it.key } ?: -1) + 1, "", "", everyone))

    companion object {
        /** A draft from the core's reading, everyone on every line, paid by me if I'm known. */
        fun from(
            r: ScannedReceiptDto,
            exponent: Int,
            everyone: Set<String>,
            payerId: String?,
            today: Int,
            untitled: String,
        ) = ReceiptDraft(
            title = r.merchant?.let(::titleCase) ?: untitled,
            payerId = payerId,
            currency = r.currency,
            rate = "",
            dateDays = r.dateDays ?: today,
            lines = r.items.mapIndexed { i, item ->
                ReceiptLine(i.toLong(), item.label, Money.plain(item.totalMinor, exponent), everyone, item.discountMinor)
            },
            printedTotalMinor = r.totalMinor,
            scansLeft = r.scansLeftToday.toInt(),
        )

        /** "RESTAURANTE HERMANOS ALBA" -> "Restaurante Hermanos Alba"; mixed case is kept. */
        fun titleCase(text: String): String =
            if (text != text.uppercase()) text.trim()
            else text.trim().lowercase().split(' ').joinToString(" ") { w -> w.replaceFirstChar { it.titlecase() } }
    }
}
