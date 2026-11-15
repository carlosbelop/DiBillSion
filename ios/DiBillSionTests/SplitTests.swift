import XCTest
@testable import DiBillSion

final class SplitTests: XCTestCase {
    private let people = ["a", "b", "c"]

    private func shares(_ mode: SplitMode, _ values: [String: String], total: Int64? = 1000) -> SplitResult {
        Split.shares(mode: mode, members: people, among: [], values: values, totalMinor: total, exponent: 2)
    }

    func testEqual() {
        let r = Split.shares(mode: .equal, members: people, among: ["c", "a"], values: [:], totalMinor: 1000, exponent: 2)
        XCTAssertEqual(r.shares.map(\.memberId), ["a", "c"])
        XCTAssertNil(r.problem)
        XCTAssertEqual(Split.shares(mode: .equal, members: people, among: [], values: [:], totalMinor: 1000,
                                    exponent: 2).problem, .nobody)
    }

    func testParts() {
        let r = shares(.parts, ["a": "2", "b": "", "c": "1"])
        XCTAssertEqual(r.shares.map(\.memberId), ["a", "c"])
        XCTAssertEqual(r.shares.map(\.parts), [2, 1])
        XCTAssertNil(r.problem)
        XCTAssertEqual(shares(.parts, ["a": "1.5"]).problem, .badValue)
        XCTAssertEqual(shares(.parts, ["a": "0"]).problem, .nobody)
    }

    func testPercentagesBecomeHundredths() {
        XCTAssertEqual(shares(.percent, ["a": "50", "b": "33,33", "c": "16.67"]).shares.map(\.parts), [5000, 3333, 1667])
        XCTAssertEqual(shares(.percent, ["a": "50", "b": "30", "c": "20"]).shares.map(\.parts), [5, 3, 2])
        XCTAssertEqual(shares(.parts, ["a": "2", "b": "4"]).shares.map(\.parts), [1, 2])
        XCTAssertEqual(shares(.percent, ["a": "75"]).problem, .percentOff(missing: 2500))
        XCTAssertEqual(shares(.percent, ["a": "60", "b": "50"]).problem, .percentOff(missing: -1000))
        XCTAssertEqual(shares(.percent, ["a": "101"]).problem, .badValue)
        XCTAssertEqual(shares(.percent, ["a": "10.005"]).problem, .badValue)
    }

    func testExactAmountsMustAddUp() {
        let r = shares(.amount, ["a": "6", "b": "4"])
        XCTAssertEqual(r.shares.map(\.amountMinor), [600, 400])
        XCTAssertNil(r.problem)
        XCTAssertEqual(shares(.amount, ["a": "9"]).problem, .amountOff(missing: 100))
        XCTAssertEqual(shares(.amount, ["a": "10.50"]).problem, .amountOff(missing: -50))
        XCTAssertNil(shares(.amount, ["a": "3"], total: nil).problem)
    }

    func testDisplayParts() {
        XCTAssertEqual(Split.displayParts(["a": 5000, "b": 3000, "c": 2000]), ["a": "5", "b": "3", "c": "2"])
    }
}
