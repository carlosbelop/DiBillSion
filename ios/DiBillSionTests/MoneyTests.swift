import XCTest
@testable import DiBillSion

final class MoneyTests: XCTestCase {
    func testParsesDotsAndCommas() {
        XCTAssertEqual(Money.parse("12.5", exponent: 2), 1250)
        XCTAssertEqual(Money.parse(" 12,50 ", exponent: 2), 1250)
        XCTAssertEqual(Money.parse("0.01", exponent: 2), 1)
        XCTAssertEqual(Money.parse(".5", exponent: 2), 50)
        XCTAssertEqual(Money.parse("12.", exponent: 2), 1200)
        XCTAssertEqual(Money.parse("12.500", exponent: 2), 1250)
        XCTAssertEqual(Money.parse("1500", exponent: 0), 1500)
        XCTAssertEqual(Money.parse("7.250", exponent: 3), 7250)
    }

    func testRejectsJunk() {
        for bad in ["", " ", ".", "-1", "1.234", "1e3", "1,2,3", "abc", "1 000", "99999999999999999999"] {
            XCTAssertNil(Money.parse(bad, exponent: 2), bad)
        }
        XCTAssertNil(Money.parse("1.5", exponent: 0))
    }

    func testFormatsMinorUnits() {
        let en = Locale(identifier: "en_US")
        XCTAssertEqual(Money.format(1234, exponent: 2, code: "EUR", locale: en), "12.34 EUR")
        XCTAssertEqual(Money.format(-5, exponent: 2, code: "EUR", locale: en), "-0.05 EUR")
        XCTAssertEqual(Money.format(1500, exponent: 0, code: "JPY", locale: en), "1,500 JPY")
        XCTAssertEqual(Money.format(1_234_567, exponent: 2, code: "EUR", locale: Locale(identifier: "es_ES")), "12.345,67 EUR")
    }

    func testNormalisesRates() {
        XCTAssertEqual(Money.parseRate("1,0850"), "1.085")
        XCTAssertEqual(Money.parseRate("0.5"), "0.5")
        XCTAssertEqual(Money.parseRate("007"), "7")
        XCTAssertEqual(Money.parseRate(".25"), "0.25")
        for bad in ["", "0", "0.000", "-1", "abc", "1.2.3"] {
            XCTAssertNil(Money.parseRate(bad), bad)
        }
    }

    func testDateDaysRoundTrip() {
        XCTAssertEqual(DateDays.daysFromCivil(year: 1970, month: 1, day: 1), 0)
        XCTAssertEqual(DateDays.daysFromCivil(year: 2026, month: 10, day: 31), 20_757)
        XCTAssertEqual(DateDays.daysFromCivil(year: 1969, month: 12, day: 31), -1)
        for days in [-1, 0, 59, 60, 11_016, 20_757, 47_482] {
            let (y, m, d) = DateDays.civil(fromDays: days)
            XCTAssertEqual(DateDays.daysFromCivil(year: y, month: m, day: d), days)
        }
        var cal = Calendar(identifier: .gregorian)
        cal.timeZone = TimeZone(identifier: "Europe/Madrid")!
        XCTAssertEqual(DateDays.from(DateDays.date(20_757, calendar: cal), calendar: cal), 20_757)
    }

    func testInviteCodeFromLinks() {
        XCTAssertEqual(AppModel.inviteCode(from: " abc123 "), "abc123")
        XCTAssertEqual(AppModel.inviteCode(from: "dibillsion://join/abc123"), "abc123")
        XCTAssertEqual(AppModel.inviteCode(from: "https://example.org/join/abc123?x=1"), "abc123")
    }
}
