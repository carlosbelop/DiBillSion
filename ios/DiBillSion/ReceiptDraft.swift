import Foundation

/// One editable line of a scanned receipt. `members` share it equally.
struct ReceiptLine: Identifiable, Equatable, Sendable {
    let id: Int
    var label: String
    var amount: String
    var members: Set<String>
    /// Discount folded into this line by the core (zero or negative), shown next to it.
    var discountMinor: Int64 = 0
}

/// The review screen's state for a scanned receipt (SPEC flow 8). Amounts stay text until
/// saving, like the expense editor.
struct ReceiptDraft: Equatable, Sendable {
    enum Problem: Equatable { case amount, nobody, empty }

    var title: String
    var payerId: String
    var currency: String
    var rate: String
    var dateDays: Int32
    var lines: [ReceiptLine]
    /// The total printed on the receipt, in `currency`.
    var printedTotalMinor: Int64?
    var scansLeft: Int

    /// Each line in minor units; nil where the text isn't a valid amount (zero is fine).
    func amounts(exponent: Int) -> [Int64?] { lines.map { Money.parse($0.amount, exponent: exponent) } }

    /// Sum of the lines, or nil while one of them is invalid.
    func sum(exponent: Int) -> Int64? {
        let a = amounts(exponent: exponent)
        return a.contains(where: { $0 == nil }) ? nil : a.reduce(0) { $0 + ($1 ?? 0) }
    }

    func problem(exponent: Int) -> Problem? {
        let a = amounts(exponent: exponent)
        if a.contains(where: { $0 == nil }) { return .amount }
        if zip(lines, a).contains(where: { ($1 ?? 0) > 0 && $0.members.isEmpty }) { return .nobody }
        if !a.contains(where: { ($0 ?? 0) > 0 }) { return .empty }
        return nil
    }

    /// The expense's items: free lines are dropped, everyone else shares equally.
    func items(exponent: Int, memberOrder: [String]) -> [ItemDto] {
        zip(lines, amounts(exponent: exponent)).compactMap { line, amount in
            guard let minor = amount, minor > 0 else { return nil }
            let label = line.label.trimmingCharacters(in: .whitespaces)
            return ItemDto(
                label: label.isEmpty ? nil : label, totalMinor: minor,
                shares: memberOrder.filter(line.members.contains)
                    .map { ShareDto(memberId: $0, parts: 1, amountMinor: nil) })
        }
    }

    mutating func update(_ id: Int, _ change: (inout ReceiptLine) -> Void) {
        if let i = lines.firstIndex(where: { $0.id == id }) { change(&lines[i]) }
    }

    mutating func remove(_ id: Int) { lines.removeAll { $0.id == id } }

    mutating func add(everyone: Set<String>) {
        lines.append(ReceiptLine(id: (lines.map(\.id).max() ?? -1) + 1, label: "", amount: "", members: everyone))
    }

    /// A draft from the core's reading, everyone on every line.
    static func from(_ r: ScannedReceiptDto, exponent: Int, everyone: Set<String>, payerId: String,
                     today: Int32, untitled: String) -> ReceiptDraft {
        ReceiptDraft(
            title: r.merchant.map(titleCase) ?? untitled,
            payerId: payerId,
            currency: r.currency,
            rate: "",
            dateDays: r.dateDays ?? today,
            lines: r.items.enumerated().map { i, item in
                ReceiptLine(id: i, label: item.label, amount: Money.plain(item.totalMinor, exponent: exponent),
                            members: everyone, discountMinor: item.discountMinor)
            },
            printedTotalMinor: r.totalMinor,
            scansLeft: Int(r.scansLeftToday))
    }

    /// "RESTAURANTE HERMANOS ALBA" -> "Restaurante Hermanos Alba"; mixed case is kept.
    static func titleCase(_ text: String) -> String {
        let trimmed = text.trimmingCharacters(in: .whitespaces)
        guard trimmed == trimmed.uppercased() else { return trimmed }
        return trimmed.lowercased().split(separator: " ")
            .map { $0.prefix(1).uppercased() + $0.dropFirst() }
            .joined(separator: " ")
    }
}
