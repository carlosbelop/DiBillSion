import Foundation

/// Money as minor units (`Int64`) on the Swift side too: never `Float`/`Double`.
enum Money {
    /// Parses user input such as "12.5" or "12,50" into minor units for a currency with
    /// `exponent` decimals. Returns nil for blanks, negatives, junk or too many decimals.
    static func parse(_ text: String, exponent: Int) -> Int64? {
        let cleaned = text.trimmingCharacters(in: .whitespaces).replacingOccurrences(of: ",", with: ".")
        let parts = cleaned.split(separator: ".", omittingEmptySubsequences: false)
        guard (1...2).contains(parts.count) else { return nil }
        let whole = parts[0]
        let fraction = parts.count == 2 ? parts[1] : ""
        guard !(whole.isEmpty && fraction.isEmpty), whole.allSatisfy(\.isASCIIDigit),
              fraction.allSatisfy(\.isASCIIDigit) else { return nil }
        // Extra decimals are fine only when they are zeros ("12.500" EUR).
        let significant = fraction.reversed().drop { $0 == "0" }.count
        guard significant <= exponent else { return nil }
        let padded = String(fraction.prefix(exponent)).padding(toLength: exponent, withPad: "0", startingAt: 0)
        guard let value = Int64(String(whole.isEmpty ? "0" : whole) + padded) else { return nil }
        return value
    }

    /// Formats minor units with the locale's separators, e.g. 1234 EUR -> "12.34 EUR".
    static func format(_ minor: Int64, exponent: Int, code: String, locale: Locale = .current) -> String {
        let formatter = NumberFormatter()
        formatter.locale = locale
        formatter.numberStyle = .decimal
        formatter.minimumFractionDigits = exponent
        formatter.maximumFractionDigits = exponent
        let value = Decimal(sign: minor < 0 ? .minus : .plus, exponent: -exponent,
                            significand: Decimal(minor.magnitude))
        let number = formatter.string(from: value as NSDecimalNumber) ?? "\(minor)"
        return "\(number) \(code)"
    }

    /// Validates an exchange rate typed by the user and normalises it for the core
    /// ("1,0850" -> "1.085"). Nil unless it is a plain number greater than zero.
    static func parseRate(_ text: String) -> String? {
        let cleaned = text.trimmingCharacters(in: .whitespaces).replacingOccurrences(of: ",", with: ".")
        let parts = cleaned.split(separator: ".", omittingEmptySubsequences: false)
        guard (1...2).contains(parts.count), parts.allSatisfy({ $0.allSatisfy(\.isASCIIDigit) }) else { return nil }
        var whole = String(parts[0].drop { $0 == "0" })
        let fraction = parts.count == 2 ? String(parts[1].reversed().drop { $0 == "0" }.reversed()) : ""
        guard !(whole.isEmpty && fraction.isEmpty) else { return nil }
        if whole.isEmpty { whole = "0" }
        return fraction.isEmpty ? whole : "\(whole).\(fraction)"
    }
}

/// Dates cross the boundary as days since 1970-01-01 in the user's calendar day.
enum DateDays {
    /// The calendar day of `date` (in `calendar`'s time zone) as days since the epoch.
    static func from(_ date: Date, calendar: Calendar = .current) -> Int32 {
        let c = calendar.dateComponents([.year, .month, .day], from: date)
        return Int32(daysFromCivil(year: c.year ?? 1970, month: c.month ?? 1, day: c.day ?? 1))
    }

    /// Noon of that day in `calendar`'s time zone, for date pickers and labels.
    static func date(_ days: Int32, calendar: Calendar = .current) -> Date {
        let (y, m, d) = civil(fromDays: Int(days))
        return calendar.date(from: DateComponents(year: y, month: m, day: d, hour: 12)) ?? Date()
    }

    static var today: Int32 { from(Date()) }

    // Proleptic Gregorian conversions in integers (H. Hinnant's algorithms).
    static func daysFromCivil(year: Int, month: Int, day: Int) -> Int {
        let y = month <= 2 ? year - 1 : year
        let era = (y >= 0 ? y : y - 399) / 400
        let yoe = y - era * 400
        let mp = (month + 9) % 12
        let doy = (153 * mp + 2) / 5 + day - 1
        let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy
        return era * 146_097 + doe - 719_468
    }

    static func civil(fromDays days: Int) -> (Int, Int, Int) {
        let z = days + 719_468
        let era = (z >= 0 ? z : z - 146_096) / 146_097
        let doe = z - era * 146_097
        let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100)
        let mp = (5 * doy + 2) / 153
        let d = doy - (153 * mp + 2) / 5 + 1
        let m = mp < 10 ? mp + 3 : mp - 9
        return (yoe + era * 400 + (m <= 2 ? 1 : 0), m, d)
    }
}

private extension Character {
    var isASCIIDigit: Bool { ("0"..."9").contains(self) }
}
