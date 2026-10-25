package com.dibillsion.app

import java.math.BigDecimal
import java.math.RoundingMode
import java.text.NumberFormat
import java.util.Locale

/** Money as minor units (Long) on the Kotlin side too: never Float/Double. */
object Money {
    /**
     * Parses user input such as "12.5" or "12,50" into minor units for a currency with
     * [exponent] decimals. Returns null for blanks, negatives or too many decimals.
     */
    fun parse(text: String, exponent: Int): Long? {
        val cleaned = text.trim().replace(',', '.')
        if (cleaned.isEmpty()) return null
        return try {
            val value = BigDecimal(cleaned)
            if (value.signum() < 0) return null
            value.setScale(exponent, RoundingMode.UNNECESSARY).unscaledValue().longValueExact()
        } catch (_: ArithmeticException) {
            null
        } catch (_: NumberFormatException) {
            null
        }
    }

    /** Formats minor units with the locale's separators, e.g. 1234 EUR -> "12.34 EUR". */
    fun format(minor: Long, exponent: Int, code: String, locale: Locale = Locale.getDefault()): String {
        val nf = NumberFormat.getNumberInstance(locale).apply {
            minimumFractionDigits = exponent
            maximumFractionDigits = exponent
        }
        return nf.format(BigDecimal.valueOf(minor, exponent)) + " " + code
    }

    /** Validates an exchange rate typed by the user and normalises it for the core. */
    fun parseRate(text: String): String? {
        val cleaned = text.trim().replace(',', '.')
        return try {
            BigDecimal(cleaned).takeIf { it.signum() > 0 }?.stripTrailingZeros()?.toPlainString()
        } catch (_: NumberFormatException) {
            null
        }
    }
}
