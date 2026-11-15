import SwiftUI

/// Equal / Parts / % (/ Amounts).
struct SplitModePicker: View {
    @Binding var mode: SplitMode
    var withAmounts = true

    var body: some View {
        Picker("Split", selection: $mode) {
            Text("Equally").tag(SplitMode.equal)
            Text("Parts").tag(SplitMode.parts)
            Text("%").tag(SplitMode.percent)
            if withAmounts { Text("Amounts").tag(SplitMode.amount) }
        }
        .pickerStyle(.segmented)
    }
}

/// One person in a split: a check for equal splits, otherwise a number to type.
struct SplitPersonRow: View {
    let name: String
    let id: String
    let mode: SplitMode
    let included: Bool
    let toggle: () -> Void
    @Binding var value: String
    /// What they pay, formatted; nil while it can't be worked out.
    let share: String?
    /// Currency code, shown next to exact amounts.
    var unit = ""

    var body: some View {
        HStack(spacing: 10) {
            if mode == .equal {
                Button(action: toggle) {
                    HStack(spacing: 10) {
                        Image(systemName: included ? "checkmark.circle.fill" : "circle")
                            .foregroundStyle(included ? Color.teal : .secondary)
                        Avatar(id: id, name: name, size: 28)
                        Text(name).foregroundStyle(.primary).lineLimit(1)
                    }
                }
                .buttonStyle(.borderless)
                .accessibilityAddTraits(included ? .isSelected : [])
            } else {
                Avatar(id: id, name: name, size: 28)
                Text(name).lineLimit(1)
            }
            Spacer(minLength: 4)
            if mode != .equal {
                HStack(spacing: 2) {
                    TextField("0", text: $value)
                        .keyboardType(mode == .parts ? .numberPad : .decimalPad)
                        .multilineTextAlignment(.trailing)
                        .monospacedDigit()
                        .frame(width: mode == .amount ? 80 : 56)
                    Text(mode == .percent ? "%" : mode == .parts ? "×" : unit)
                        .font(.caption).foregroundStyle(.secondary)
                }
                .padding(.horizontal, 6)
                .padding(.vertical, 4)
                .background(RoundedRectangle(cornerRadius: 6).fill(Color.secondary.opacity(0.12)))
            }
            Text(share ?? "—")
                .monospacedDigit()
                .font(.subheadline)
                .foregroundStyle(share == nil ? .tertiary : .secondary)
                .frame(minWidth: 72, alignment: .trailing)
        }
    }
}

/// What's wrong with a split, in words, or nil.
func splitProblemText(_ problem: SplitProblem?, exponent: Int, code: String) -> String? {
    switch problem {
    case nil: return nil
    case .nobody: return String(localized: "Pick at least one person.")
    case .badValue: return String(localized: "Check the numbers: whole parts, percentages up to 100.")
    case .percentOff(let missing):
        let digits = Money.plain(abs(missing), exponent: 2)
        let trimmed = digits.hasSuffix(".00") ? String(digits.dropLast(3))
            : digits.hasSuffix("0") ? String(digits.dropLast()) : digits
        let pct = trimmed + " %"
        return missing > 0 ? String(localized: "\(pct) left to assign.") : String(localized: "\(pct) too much.")
    case .amountOff(let missing):
        let amount = Money.format(abs(missing), exponent: exponent, code: code)
        return missing > 0 ? String(localized: "\(amount) left to assign.") : String(localized: "\(amount) too much.")
    }
}

/// Equal, parts or percentage for an item, or for every item at once. `initial` is who takes
/// part and with how many parts; `apply` gets everyone in `people` (0 = not taking part).
/// `totalMinor` only feeds the live preview of each share.
struct PartsSplitSheet: View {
    let title: String
    var hint: String?
    let totalMinor: Int64?
    let people: [MemberDto]
    let exponent: Int
    let code: String
    let apply: ([String: Int64]) -> Void
    @Environment(AppModel.self) private var model
    @Environment(\.dismiss) private var dismiss
    @State private var mode: SplitMode
    @State private var among: Set<String>
    @State private var values: [String: String]

    init(title: String, hint: String? = nil, totalMinor: Int64?, initial: [String: Int64], people: [MemberDto],
         exponent: Int, code: String, apply: @escaping ([String: Int64]) -> Void) {
        self.title = title
        self.hint = hint
        self.totalMinor = totalMinor
        self.people = people
        self.exponent = exponent
        self.code = code
        self.apply = apply
        let even = Set(initial.values).count <= 1
        _mode = State(initialValue: even ? .equal : .parts)
        _among = State(initialValue: Set(initial.keys))
        _values = State(initialValue: even ? [:] : Split.displayParts(initial))
    }

    private var split: SplitResult {
        Split.shares(mode: mode, members: people.map(\.id), among: among, values: values,
                     totalMinor: totalMinor, exponent: exponent)
    }

    var body: some View {
        let split = self.split
        let preview: [Int64] = {
            guard let total = totalMinor, total > 0, split.problem == nil else { return [] }
            return model.previewShares(total, shares: split.shares)
        }()
        let shareOf = Dictionary(zip(split.shares.map(\.memberId), preview), uniquingKeysWith: { a, _ in a })
        NavigationStack {
            Form {
                Section {
                    SplitModePicker(mode: $mode, withAmounts: false)
                } footer: {
                    if let hint { Text(hint) }
                }
                Section {
                    ForEach(people, id: \.id) { m in
                        SplitPersonRow(
                            name: m.displayName, id: m.id, mode: mode, included: among.contains(m.id),
                            toggle: { if among.contains(m.id) { among.remove(m.id) } else { among.insert(m.id) } },
                            value: Binding(get: { values[m.id] ?? "" }, set: { values[m.id] = $0 }),
                            share: shareOf[m.id].map { Money.format($0, exponent: exponent, code: code) })
                    }
                } footer: {
                    if let text = splitProblemText(split.problem, exponent: exponent, code: code) {
                        Text(text).foregroundStyle(Color.coral)
                    }
                }
            }
            .navigationTitle(title)
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) { Button("Cancel") { dismiss() } }
                ToolbarItem(placement: .confirmationAction) {
                    Button("Apply") {
                        var chosen: [String: Int64] = [:]
                        for s in split.shares { chosen[s.memberId] = Int64(s.parts ?? 0) }
                        apply(Dictionary(uniqueKeysWithValues: people.map { ($0.id, chosen[$0.id] ?? 0) }))
                        dismiss()
                    }
                    .disabled(split.problem != nil)
                }
            }
        }
    }
}
