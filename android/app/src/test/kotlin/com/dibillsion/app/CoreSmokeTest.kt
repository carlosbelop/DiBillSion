package com.dibillsion.app

import com.dibillsion.core.Core
import com.dibillsion.core.CoreException
import com.dibillsion.core.GroupKindDto
import com.dibillsion.core.ItemDto
import com.dibillsion.core.ShareDto
import com.dibillsion.core.TransactionDto
import com.dibillsion.core.TransactionKindDto
import com.dibillsion.core.coreVersion
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/** Calls the real Rust core (host build) through the generated bindings. */
class CoreSmokeTest {
    @Test
    fun expenseToSettledThroughTheBindings() {
        assertTrue(coreVersion().isNotEmpty())
        Core.inMemory().use { core ->
            val g = core.createGroup("Trip", null, GroupKindDto.TRIP, "EUR")
            val a = core.addMember(g, "Ana")
            val b = core.addMember(g, "Bea")
            core.saveTransaction(
                TransactionDto(
                    null, g, TransactionKindDto.EXPENSE, "Dinner", a, "EUR", null, 20_000,
                    listOf(ItemDto(null, 1001, listOf(ShareDto(a, 1u, null), ShareDto(b, 1u, null)))), null,
                ),
            )
            val s = core.balances(g).suggestions.single()
            assertEquals(Triple(b, a, 500L), Triple(s.fromId, s.toId, s.amountMinor))

            core.saveTransaction(
                TransactionDto(
                    null, g, TransactionKindDto.TRANSFER, "", b, "EUR", null, 20_001,
                    listOf(ItemDto(null, 500, listOf(ShareDto(a, null, 500)))), null,
                ),
            )
            assertTrue(core.balances(g).balances.all { it.amountMinor == 0L })

            // My summary: Bea's half of the dinner, which Ana paid; the transfer isn't spending.
            val mine = core.mySummary(g, b)
            assertEquals(500L to 1001L, mine.spentMinor to mine.groupSpentMinor)
            assertEquals(4995u, mine.shareBasisPoints)
            assertTrue(mine.paid.isEmpty())
            assertEquals(1u to 1u, mine.involved.single().itemsIn to mine.involved.single().items)
            assertEquals(1, core.mySummary(g, a).paid.size)
        }
    }

    @Test(expected = CoreException.UnknownCurrency::class)
    fun validationErrorsSurfaceAsExceptions() {
        Core.inMemory().use { it.createGroup("X", null, GroupKindDto.OTHER, "EURO") }
    }
}
