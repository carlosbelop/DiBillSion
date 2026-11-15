import Foundation

/// One editable line of a scanned receipt. `parts` says who shares it and how much each one
/// takes (all 1 = equally); everyone in it has at least one part.
struct ReceiptLine: Identifiable, Equatable, Sendable {
    let id: Int
    var label: String
    var amount: String
    var parts: [String: Int64]
    /// Discount folded into this line by the core (zero or negative), shown next to it.
    var discountMinor: Int64 = 0

    /// Who shares it; setting it shares it equally between exactly those people.
    var members: Set<String> {
        get { Set(parts.keys) }
        set { parts = equalParts(newValue) }
    }

    /// Everyone takes the same part.
    var even: Bool { Set(parts.values).count <= 1 }

    /// Adds or removes one person. An even line stays even; on a custom one the others keep
    /// their parts and a newcomer takes one, as on a saved item (ADR 0009).
    mutating func toggle(_ id: String) {
        if parts[id] != nil {
            parts[id] = nil
        } else if even {
            members.insert(id)
        } else {
            parts[id] = 1
        }
    }

    /// `split` with zeros (not taking part) dropped.
    mutating func setParts(_ split: [String: Int64]) { parts = split.filter { $0.value > 0 } }
}

/// Everyone in `members` with one part.
func equalParts(_ members: Set<String>) -> [String: Int64] {
    Dictionary(uniqueKeysWithValues: members.map { ($0, Int64(1)) })
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
    /// The last split chosen for the whole receipt, offered again when it's reopened.
    var wholeSplit: [String: Int64]?

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

    /// The expense's items: free lines are dropped, the rest split by their parts.
    func items(exponent: Int, memberOrder: [String]) -> [ItemDto] {
        zip(lines, amounts(exponent: exponent)).compactMap { line, amount in
            guard let minor = amount, minor > 0 else { return nil }
            let label = line.label.trimmingCharacters(in: .whitespaces)
            return ItemDto(
                label: label.isEmpty ? nil : label, totalMinor: minor,
                shares: memberOrder.compactMap { id in
                    line.parts[id].map { ShareDto(memberId: id, parts: UInt32($0), amountMinor: nil) }
                })
        }
    }

    /// One split for the whole receipt: every line takes it, and each can still be changed
    /// afterwards. Zeros (not taking part) are dropped.
    mutating func splitAll(_ split: [String: Int64]) {
        let parts = split.filter { $0.value > 0 }
        for i in lines.indices { lines[i].parts = parts }
        wholeSplit = parts
    }

    /// What the whole-receipt sheet starts from: the last one chosen, else what every line shares.
    func currentWholeSplit(everyone: Set<String>) -> [String: Int64] {
        if let wholeSplit { return wholeSplit }
        let distinct = lines.map(\.parts).reduce(into: [[String: Int64]]()) { acc, p in
            if !acc.contains(p) { acc.append(p) }
        }
        return distinct.count == 1 ? distinct[0] : equalParts(everyone)
    }

    mutating func update(_ id: Int, _ change: (inout ReceiptLine) -> Void) {
        if let i = lines.firstIndex(where: { $0.id == id }) { change(&lines[i]) }
    }

    mutating func remove(_ id: Int) { lines.removeAll { $0.id == id } }

    mutating func add(everyone: Set<String>) {
        lines.append(ReceiptLine(id: (lines.map(\.id).max() ?? -1) + 1, label: "", amount: "",
                                 parts: wholeSplit ?? equalParts(everyone)))
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
                            parts: equalParts(everyone), discountMinor: item.discountMinor)
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
