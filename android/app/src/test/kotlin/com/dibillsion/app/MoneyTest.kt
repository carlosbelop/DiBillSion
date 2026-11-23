package com.dibillsion.app

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test
import java.util.Locale

class MoneyTest {
    @Test
    fun parsesIntoMinorUnits() {
        assertEquals(1250L, Money.parse("12.5", 2))
        assertEquals(1250L, Money.parse(" 12,50 ", 2))
        assertEquals(1234L, Money.parse("1234", 0))
        assertEquals(1000L, Money.parse("1", 3))
    }

    @Test
    fun rejectsBadInput() {
        assertNull(Money.parse("", 2))
        assertNull(Money.parse("-1", 2))
        assertNull(Money.parse("1.234", 2))
        assertNull(Money.parse("abc", 2))
        assertNull(Money.parse("99999999999999999999", 2))
    }

    @Test
    fun percentHasOneDecimalRoundedHalfUp() {
        assertEquals("16.7", Money.percent(1667, Locale.US))
        assertEquals("16,7", Money.percent(1667, Locale.GERMANY))
        assertEquals("12.5", Money.percent(1245, Locale.US))
        assertEquals("100.0", Money.percent(10_000, Locale.US))
        assertEquals("0.0", Money.percent(0, Locale.US))
    }

    @Test
    fun formatsWithLocaleSeparators() {
        assertEquals("1,234.50 EUR", Money.format(123450, 2, "EUR", Locale.US))
        assertEquals("1.234,50 EUR", Money.format(123450, 2, "EUR", Locale.GERMANY))
        assertEquals("1,234 JPY", Money.format(1234, 0, "JPY", Locale.US))
        assertEquals("-3.34 EUR", Money.format(-334, 2, "EUR", Locale.US))
    }

    @Test
    fun ratesArePositiveDecimals() {
        assertEquals("0.9215", Money.parseRate("0,92150"))
        assertNull(Money.parseRate("0"))
        assertNull(Money.parseRate("x"))
    }
}
