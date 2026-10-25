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
                    listOf(ItemDto(null, 1001, listOf(ShareDto(a, 1u, null), ShareDto(b, 1u, null)))),
                ),
            )
            val s = core.balances(g).suggestions.single()
            assertEquals(Triple(b, a, 500L), Triple(s.fromId, s.toId, s.amountMinor))

            core.saveTransaction(
                TransactionDto(
                    null, g, TransactionKindDto.TRANSFER, "", b, "EUR", null, 20_001,
                    listOf(ItemDto(null, 500, listOf(ShareDto(a, null, 500)))),
                ),
            )
            assertTrue(core.balances(g).balances.all { it.amountMinor == 0L })
        }
    }

    @Test(expected = CoreException.InvalidInput::class)
    fun validationErrorsSurfaceAsExceptions() {
        Core.inMemory().use { it.createGroup("X", null, GroupKindDto.OTHER, "EURO") }
    }
}
