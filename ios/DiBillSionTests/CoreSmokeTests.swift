import XCTest
@testable import DiBillSion

/// Goes through the real Rust core (the XCFramework), in memory.
final class CoreSmokeTests: XCTestCase {
    func testFlatScenario() throws {
        let core = try Core.inMemory()
        let flat = try core.createGroup(name: "Flat", emoji: "🏠", kind: .home, baseCurrency: "EUR")
        let ana = try core.addMember(groupId: flat, displayName: "Ana")
        let bea = try core.addMember(groupId: flat, displayName: "Bea")
        let cai = try core.addMember(groupId: flat, displayName: "Cai")
        try core.setMe(groupId: flat, memberId: bea)

        // 10.00 EUR paid by Ana, split three ways: the cents still add up.
        let everyone = [ana, bea, cai].map { ShareDto(memberId: $0, parts: 1, amountMinor: nil) }
        let shares = try previewSplit(totalMinor: 1000, shares: everyone)
        XCTAssertEqual(shares.reduce(0, +), 1000)
        _ = try core.saveTransaction(t: TransactionDto(
            id: nil, groupId: flat, kind: .expense, title: "Groceries", payerId: ana, currency: "EUR",
            fxRate: nil, dateDays: DateDays.today,
            items: [ItemDto(label: nil, totalMinor: 1000, shares: everyone)], baseTotalMinor: nil))

        let balances = try core.balances(groupId: flat)
        XCTAssertEqual(balances.balances.map(\.amountMinor).reduce(0, +), 0)
        XCTAssertEqual(balances.suggestions.map(\.amountMinor).reduce(0, +), 1000 - shares[0])
        XCTAssertTrue(balances.suggestions.allSatisfy { $0.toId == ana })

        let group = try XCTUnwrap(core.groups().first)
        XCTAssertEqual(group.meId, bea)
        XCTAssertEqual(group.myBalanceMinor, -shares[1])
        XCTAssertFalse(try core.isShared(groupId: flat))
    }

    func testErrorsAreTyped() throws {
        let core = try Core.inMemory()
        XCTAssertThrowsError(try core.createGroup(name: "X", emoji: nil, kind: .other, baseCurrency: "ZZZ")) {
            guard case CoreError.UnknownCurrency = $0 else { return XCTFail("\($0)") }
        }
        XCTAssertEqual(try currencyExponent(code: "JPY"), 0)
    }
}
