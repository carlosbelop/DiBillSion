package com.dibillsion.app

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class SplitTest {
    private val people = listOf("a", "b", "c")

    private fun shares(mode: SplitMode, values: Map<String, String>, total: Long? = 1000) =
        Split.shares(mode, people, emptySet(), values, total, 2)

    @Test
    fun equal() {
        val r = Split.shares(SplitMode.EQUAL, people, setOf("c", "a"), emptyMap(), 1000, 2)
        assertEquals(listOf("a", "c"), r.shares.map { it.memberId })
        assertNull(r.problem)
        assertEquals(SplitProblem.Nobody, Split.shares(SplitMode.EQUAL, people, emptySet(), emptyMap(), 1000, 2).problem)
    }

    @Test
    fun parts() {
        val r = shares(SplitMode.PARTS, mapOf("a" to "2", "b" to "", "c" to "1"))
        assertEquals(listOf("a" to 2u, "c" to 1u), r.shares.map { it.memberId to it.parts })
        assertNull(r.problem)
        assertEquals(SplitProblem.BadValue, shares(SplitMode.PARTS, mapOf("a" to "1.5")).problem)
        assertEquals(SplitProblem.Nobody, shares(SplitMode.PARTS, mapOf("a" to "0")).problem)
    }

    @Test
    fun percentagesBecomeHundredths() {
        val r = shares(SplitMode.PERCENT, mapOf("a" to "50", "b" to "33,33", "c" to "16.67"))
        assertEquals(listOf(5000u, 3333u, 1667u), r.shares.map { it.parts })
        assertNull(r.problem)
        // Reduced to the smallest equivalent parts.
        assertEquals(listOf(5u, 3u, 2u), shares(SplitMode.PERCENT, mapOf("a" to "50", "b" to "30", "c" to "20")).shares.map { it.parts })
        assertEquals(listOf(1u, 2u), shares(SplitMode.PARTS, mapOf("a" to "2", "b" to "4")).shares.map { it.parts })
        assertEquals(SplitProblem.PercentOff(2500), shares(SplitMode.PERCENT, mapOf("a" to "75")).problem)
        assertEquals(SplitProblem.PercentOff(-1000), shares(SplitMode.PERCENT, mapOf("a" to "60", "b" to "50")).problem)
        assertEquals(SplitProblem.BadValue, shares(SplitMode.PERCENT, mapOf("a" to "101")).problem)
        assertEquals(SplitProblem.BadValue, shares(SplitMode.PERCENT, mapOf("a" to "10.005")).problem)
    }

    @Test
    fun exactAmountsMustAddUp() {
        val r = shares(SplitMode.AMOUNT, mapOf("a" to "6", "b" to "4"))
        assertEquals(listOf(600L, 400L), r.shares.map { it.amountMinor })
        assertNull(r.problem)
        assertEquals(SplitProblem.AmountOff(100), shares(SplitMode.AMOUNT, mapOf("a" to "9")).problem)
        assertEquals(SplitProblem.AmountOff(-50), shares(SplitMode.AMOUNT, mapOf("a" to "10.50")).problem)
        // No total yet: nothing to compare with.
        assertNull(shares(SplitMode.AMOUNT, mapOf("a" to "3"), total = null).problem)
    }
}
