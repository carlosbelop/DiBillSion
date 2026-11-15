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
        val d = draft().update(2) { it.withMembers(setOf("c", "a")) }
        val items = d.items(2, listOf("a", "b", "c"))
        assertEquals(listOf(27_000L, 117L), items.map { it.totalMinor })
        assertEquals(listOf("a", "c"), items[1].shares.map { it.memberId })
        assertEquals("PAN", items[1].label)
    }

    @Test
    fun problems() {
        assertEquals(ReceiptDraft.Problem.AMOUNT, draft().update(0) { it.copy(amount = "x") }.problem(2))
        assertNull(draft().update(0) { it.copy(amount = "x") }.sum(2))
        assertEquals(ReceiptDraft.Problem.NOBODY, draft().update(0) { it.withMembers(emptySet()) }.problem(2))
        // A free line may have nobody.
        assertNull(draft().update(1) { it.withMembers(emptySet()) }.problem(2))
        val empty = draft().remove(0).remove(2)
        assertEquals(ReceiptDraft.Problem.EMPTY, empty.problem(2))
        val added = empty.add(everyone)
        assertEquals(listOf(1L, 2L), added.lines.map { it.key })
        assertEquals(ReceiptDraft.Problem.AMOUNT, added.problem(2))
    }

    @Test
    fun lineSplitByParts() {
        val d = draft().update(0) { it.withParts(mapOf("a" to 2, "b" to 1, "c" to 0)) }
        assertEquals(setOf("a", "b"), d.lines[0].members)
        assertEquals(false, d.lines[0].even)
        val item = d.items(2, listOf("c", "b", "a"))[0]
        assertEquals(listOf("b" to 1u, "a" to 2u), item.shares.map { it.memberId to it.parts })
        // On a custom line the others keep their parts; a newcomer takes one.
        assertEquals(mapOf("a" to 2L, "b" to 1L, "c" to 1L), d.lines[0].toggle("c").parts)
        assertEquals(mapOf("a" to 2L), d.lines[0].toggle("b").parts)
        // An even line stays even.
        assertEquals(mapOf("a" to 1L, "b" to 1L), draft().lines[0].toggle("c").parts)
    }

    @Test
    fun wholeReceiptSplitAppliesToEveryLineAndEachCanChange() {
        val whole = mapOf("a" to 50L, "b" to 30L, "c" to 0L)
        val d = draft().splitAll(whole)
        assertEquals(List(3) { mapOf("a" to 50L, "b" to 30L) }, d.lines.map { it.parts })
        assertEquals(mapOf("a" to 50L, "b" to 30L), d.currentWholeSplit(everyone))
        // New lines take the whole-receipt split too.
        assertEquals(mapOf("a" to 50L, "b" to 30L), d.add(everyone).lines.last().parts)
        val changed = d.update(1) { it.withMembers(setOf("c")) }
        assertEquals(mapOf("c" to 1L), changed.lines[1].parts)
        assertEquals(mapOf("a" to 50L, "b" to 30L), changed.lines[0].parts)
        // Without a whole split chosen, the dialog starts from what all lines share, else equal.
        assertEquals(mapOf("a" to 1L, "b" to 1L, "c" to 1L), draft().currentWholeSplit(everyone))
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
