import Foundation

/// How an amount is divided between people (SPEC §4).
enum SplitMode: String, CaseIterable, Identifiable, Sendable {
    case equal, parts, percent, amount
    var id: String { rawValue }
}

/// Why a split can't be saved yet; `missing` is what's left to assign (negative: too much).
enum SplitProblem: Equatable, Sendable {
    case nobody
    case badValue
    /// In hundredths of a percent.
    case percentOff(missing: Int64)
    /// In minor units of the expense's currency.
    case amountOff(missing: Int64)
}

struct SplitResult: Equatable {
    let shares: [ShareDto]
    let problem: SplitProblem?
}

/// The shares for one amount, in `members` order. Percentages become parts in hundredths of a
/// percent (12.5 % → 1250), so the core still splits them to the exact cent; exact amounts must
/// add up to `totalMinor`. Blank fields count as 0 (not taking part).
enum Split {
    static let wholePercent: Int64 = 10_000

    static func shares(mode: SplitMode, members: [String], among: Set<String>, values: [String: String],
                       totalMinor: Int64?, exponent: Int) -> SplitResult {
        if mode == .equal {
            let shares = members.filter(among.contains).map { ShareDto(memberId: $0, parts: 1, amountMinor: nil) }
            return SplitResult(shares: shares, problem: shares.isEmpty ? .nobody : nil)
        }
        var parsed: [(String, Int64)] = []
        for id in members {
            let text = (values[id] ?? "").trimmingCharacters(in: .whitespaces)
            let value: Int64?
            if text.isEmpty {
                value = 0
            } else {
                switch mode {
                case .parts: value = Int64(text).flatMap { (0...1_000_000).contains($0) ? $0 : nil }
                case .percent: value = Money.parse(text, exponent: 2).flatMap { $0 <= wholePercent ? $0 : nil }
                default: value = Money.parse(text, exponent: exponent)
                }
            }
            guard let value else { return SplitResult(shares: [], problem: .badValue) }
            parsed.append((id, value))
        }
        let taking = parsed.filter { $0.1 > 0 }
        if taking.isEmpty { return SplitResult(shares: [], problem: .nobody) }
        let sum = taking.reduce(0) { $0 + $1.1 }
        switch mode {
        case .amount:
            let off = totalMinor.map { $0 - sum }.flatMap { $0 != 0 ? $0 : nil }
            return SplitResult(shares: taking.map { ShareDto(memberId: $0.0, parts: nil, amountMinor: $0.1) },
                               problem: off.map { .amountOff(missing: $0) })
        case .percent:
            let off = wholePercent - sum
            return SplitResult(shares: reduced(taking), problem: off != 0 ? .percentOff(missing: off) : nil)
        default:
            return SplitResult(shares: reduced(taking), problem: nil)
        }
    }

    /// Parts in their smallest form (50/30/20 % → 5/3/2): the same split, easier to read later.
    private static func reduced(_ taking: [(String, Int64)]) -> [ShareDto] {
        let g = max(taking.reduce(0) { gcd($0, $1.1) }, 1)
        return taking.map { ShareDto(memberId: $0.0, parts: UInt32($0.1 / g), amountMinor: nil) }
    }

    static func gcd(_ a: Int64, _ b: Int64) -> Int64 { b == 0 ? a : gcd(b, a % b) }

    /// Parts as the editor shows them, smallest form: 5000/3000/2000 reads as 5/3/2.
    static func displayParts(_ parts: [String: Int64]) -> [String: String] {
        let g = max(parts.values.reduce(0, gcd), 1)
        return parts.mapValues { String($0 / g) }
    }
}
