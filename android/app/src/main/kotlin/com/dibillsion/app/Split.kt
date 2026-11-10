package com.dibillsion.app

import com.dibillsion.core.ShareDto

/** How an amount is divided between people (SPEC §4). */
enum class SplitMode { EQUAL, PARTS, PERCENT, AMOUNT }

/** Why a split can't be saved yet; [missing] is what's left to assign (negative: too much). */
sealed interface SplitProblem {
    data object Nobody : SplitProblem
    data object BadValue : SplitProblem
    /** In hundredths of a percent. */
    data class PercentOff(val missing: Long) : SplitProblem
    /** In minor units of the expense's currency. */
    data class AmountOff(val missing: Long) : SplitProblem
}

data class SplitResult(val shares: List<ShareDto>, val problem: SplitProblem?)

/**
 * The shares for one amount, in [members] order. Percentages become parts in hundredths of a
 * percent (12.5 % → 1250), so the core still splits them to the exact cent; exact amounts must
 * add up to [totalMinor]. Blank fields count as 0 (not taking part).
 */
object Split {
    const val WHOLE_PERCENT = 10_000L

    fun shares(
        mode: SplitMode,
        members: List<String>,
        among: Set<String>,
        values: Map<String, String>,
        totalMinor: Long?,
        exponent: Int,
    ): SplitResult {
        if (mode == SplitMode.EQUAL) {
            val shares = members.filter { it in among }.map { ShareDto(it, 1u, null) }
            return SplitResult(shares, if (shares.isEmpty()) SplitProblem.Nobody else null)
        }
        val parsed = members.map { id ->
            val text = values[id].orEmpty().trim()
            id to when {
                text.isEmpty() -> 0L
                mode == SplitMode.PARTS -> text.toLongOrNull()?.takeIf { it in 0..1_000_000 }
                mode == SplitMode.PERCENT -> Money.parse(text, 2)?.takeIf { it <= WHOLE_PERCENT }
                else -> Money.parse(text, exponent)
            }
        }
        if (parsed.any { it.second == null }) return SplitResult(emptyList(), SplitProblem.BadValue)
        val taking = parsed.mapNotNull { (id, v) -> v?.takeIf { it > 0 }?.let { id to it } }
        if (taking.isEmpty()) return SplitResult(emptyList(), SplitProblem.Nobody)
        val sum = taking.sumOf { it.second }
        // Parts in their smallest form (50/30/20 % → 5/3/2): the same split, easier to read later.
        val gcd = taking.fold(0L) { g, (_, v) -> gcd(g, v) }.coerceAtLeast(1)
        return when (mode) {
            SplitMode.AMOUNT -> SplitResult(
                taking.map { (id, a) -> ShareDto(id, null, a) },
                totalMinor?.let { t -> (t - sum).takeIf { it != 0L }?.let(SplitProblem::AmountOff) },
            )
            SplitMode.PERCENT -> SplitResult(
                taking.map { (id, p) -> ShareDto(id, (p / gcd).toUInt(), null) },
                (WHOLE_PERCENT - sum).takeIf { it != 0L }?.let(SplitProblem::PercentOff),
            )
            else -> SplitResult(taking.map { (id, p) -> ShareDto(id, (p / gcd).toUInt(), null) }, null)
        }
    }

    private tailrec fun gcd(a: Long, b: Long): Long = if (b == 0L) a else gcd(b, a % b)
}
