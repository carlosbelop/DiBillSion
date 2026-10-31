import XCTest
@testable import DiBillSion

final class ReceiptDraftTests: XCTestCase {
    private let everyone: Set<String> = ["a", "b", "c"]

    private func scanned(merchant: String? = "RESTAURANTE HERMANOS ALBA", date: Int32? = 20_313) -> ScannedReceiptDto {
        ScannedReceiptDto(
            merchant: merchant, dateDays: date, currency: "EUR",
            items: [
                ScannedItemDto(label: "MENU CENA", quantity: "3", totalMinor: 27_000, discountMinor: 0),
                ScannedItemDto(label: "AGUA", quantity: "4", totalMinor: 0, discountMinor: 0),
                ScannedItemDto(label: "PAN", quantity: nil, totalMinor: 117, discountMinor: -8),
            ],
            totalMinor: 27_117, itemsSumMinor: 27_117, scansLeftToday: 19)
    }

    private func draft() -> ReceiptDraft {
        ReceiptDraft.from(scanned(), exponent: 2, everyone: everyone, payerId: "a", today: 1, untitled: "Receipt")
    }

    func testStartsFromTheReading() {
        let d = draft()
        XCTAssertEqual(d.title, "Restaurante Hermanos Alba")
        XCTAssertEqual(d.dateDays, 20_313)
        XCTAssertEqual(d.lines.map(\.amount), ["270.00", "0.00", "1.17"])
        XCTAssertEqual(d.lines[2].discountMinor, -8)
        XCTAssertEqual(d.sum(exponent: 2), 27_117)
        XCTAssertNil(d.problem(exponent: 2))
        XCTAssertEqual(d.scansLeft, 19)
        let bare = ReceiptDraft.from(scanned(merchant: nil, date: nil), exponent: 2, everyone: everyone,
                                     payerId: "a", today: 1, untitled: "Receipt")
        XCTAssertEqual(bare.title, "Receipt")
        XCTAssertEqual(bare.dateDays, 1)
    }

    func testItemsDropFreeLinesAndKeepMemberOrder() {
        var d = draft()
        d.update(2) { $0.members = ["c", "a"] }
        let items = d.items(exponent: 2, memberOrder: ["a", "b", "c"])
        XCTAssertEqual(items.map(\.totalMinor), [27_000, 117])
        XCTAssertEqual(items[1].shares.map(\.memberId), ["a", "c"])
        XCTAssertEqual(items[1].label, "PAN")
    }

    func testProblems() {
        var d = draft()
        d.update(0) { $0.amount = "x" }
        XCTAssertEqual(d.problem(exponent: 2), .amount)
        XCTAssertNil(d.sum(exponent: 2))
        d = draft()
        d.update(0) { $0.members = [] }
        XCTAssertEqual(d.problem(exponent: 2), .nobody)
        d = draft()
        d.update(1) { $0.members = [] } // a free line may have nobody
        XCTAssertNil(d.problem(exponent: 2))
        d = draft()
        d.remove(0)
        d.remove(2)
        XCTAssertEqual(d.problem(exponent: 2), .empty)
        d.add(everyone: everyone)
        XCTAssertEqual(d.lines.map(\.id), [1, 2])
        XCTAssertEqual(d.problem(exponent: 2), .amount)
    }

    func testTitleCaseAndPlainAmounts() {
        XCTAssertEqual(ReceiptDraft.titleCase("AHORRAMAS"), "Ahorramas")
        XCTAssertEqual(ReceiptDraft.titleCase(" RELAIS ODEON "), "Relais Odeon")
        XCTAssertEqual(ReceiptDraft.titleCase("McDonald's"), "McDonald's")
        XCTAssertEqual(Money.plain(250, exponent: 2), "2.50")
        XCTAssertEqual(Money.plain(5, exponent: 2), "0.05")
        XCTAssertEqual(Money.plain(-8, exponent: 2), "-0.08")
        XCTAssertEqual(Money.plain(1234, exponent: 0), "1234")
        XCTAssertEqual(Money.plain(0, exponent: 3), "0.000")
    }

    func testImageTargetSize() {
        XCTAssertTrue(ReceiptImage.targetSize(width: 4000, height: 3000) == (1600, 1200))
        XCTAssertTrue(ReceiptImage.targetSize(width: 1795, height: 3891) == (738, 1600))
        XCTAssertTrue(ReceiptImage.targetSize(width: 800, height: 600) == (800, 600))
    }
}
