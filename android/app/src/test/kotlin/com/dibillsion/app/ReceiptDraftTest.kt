package com.dibillsion.app

import com.dibillsion.core.ScannedItemDto
import com.dibillsion.core.ScannedReceiptDto
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class ReceiptDraftTest {
    private val everyone = setOf("a", "b", "c")

    private fun scanned() = ScannedReceiptDto(
        merchant = "RESTAURANTE HERMANOS ALBA",
        dateDays = 20_313,
        currency = "EUR",
        items = listOf(
            ScannedItemDto("MENU CENA", "3", 27_000, 0),
            ScannedItemDto("AGUA", "4", 0, 0),
            ScannedItemDto("PAN", null, 117, -8),
        ),
        totalMinor = 27_117,
        itemsSumMinor = 27_117,
        scansLeftToday = 19u,
    )

    private fun draft() = ReceiptDraft.from(scanned(), 2, everyone, "a", today = 1, untitled = "Receipt")

    @Test
    fun startsFromTheReading() {
        val d = draft()
        assertEquals("Restaurante Hermanos Alba", d.title)
        assertEquals(20_313, d.dateDays)
        assertEquals(listOf("270.00", "0.00", "1.17"), d.lines.map { it.amount })
        assertEquals(-8L, d.lines[2].discountMinor)
        assertEquals(27_117L, d.sum(2))
        assertNull(d.problem(2))
        assertEquals(19, d.scansLeft)
        assertEquals(1, ReceiptDraft.from(scanned().copy(dateDays = null, merchant = null), 2, everyone, "a", 1, "Receipt").dateDays)
    }

    @Test
    fun itemsDropFreeLinesAndKeepMemberOrder() {
        val d = draft().update(2) { it.copy(members = setOf("c", "a")) }
        val items = d.items(2, listOf("a", "b", "c"))
        assertEquals(listOf(27_000L, 117L), items.map { it.totalMinor })
        assertEquals(listOf("a", "c"), items[1].shares.map { it.memberId })
        assertEquals("PAN", items[1].label)
    }

    @Test
    fun problems() {
        assertEquals(ReceiptDraft.Problem.AMOUNT, draft().update(0) { it.copy(amount = "x") }.problem(2))
        assertNull(draft().update(0) { it.copy(amount = "x") }.sum(2))
        assertEquals(ReceiptDraft.Problem.NOBODY, draft().update(0) { it.copy(members = emptySet()) }.problem(2))
        // A free line may have nobody.
        assertNull(draft().update(1) { it.copy(members = emptySet()) }.problem(2))
        val empty = draft().remove(0).remove(2)
        assertEquals(ReceiptDraft.Problem.EMPTY, empty.problem(2))
        val added = empty.add(everyone)
        assertEquals(listOf(1L, 2L), added.lines.map { it.key })
        assertEquals(ReceiptDraft.Problem.AMOUNT, added.problem(2))
    }

    @Test
    fun titleCase() {
        assertEquals("Ahorramas", ReceiptDraft.titleCase("AHORRAMAS"))
        assertEquals("Relais Odeon", ReceiptDraft.titleCase(" RELAIS ODEON "))
        assertEquals("McDonald's", ReceiptDraft.titleCase("McDonald's"))
    }

    @Test
    fun imageTargetSize() {
        assertEquals(1600 to 1200, ReceiptImage.targetSize(4000, 3000))
        assertEquals(738 to 1600, ReceiptImage.targetSize(1795, 3891))
        assertEquals(800 to 600, ReceiptImage.targetSize(800, 600))
        assertEquals(1 to 1600, ReceiptImage.targetSize(1, 5000))
    }
}
