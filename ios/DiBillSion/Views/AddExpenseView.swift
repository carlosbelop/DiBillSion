import SwiftUI

/// A new expense split equally, by parts, by percentage or by exact amounts, with each
/// person's exact share computed live by the core.
struct AddExpenseView: View {
    let d: GroupDetail
    @Environment(AppModel.self) private var model
    @Environment(\.dismiss) private var dismiss
    @State private var title = ""
    @State private var amount = ""
    @State private var currency: String
    @State private var rate = ""
    @State private var payerId: String
    @State private var date = Date()
    @State private var among: Set<String>
    @State private var mode: SplitMode = .equal
    @State private var values: [String: String] = [:]
    @State private var confirmDiscard = false
    @State private var saving = false

    init(d: GroupDetail) {
        self.d = d
        let active = d.members.filter(\.active).map(\.id)
        _currency = State(initialValue: d.group.baseCurrency)
        _payerId = State(initialValue: d.meId.flatMap { me in active.contains(me) ? me : nil } ?? active.first ?? "")
        _among = State(initialValue: Set(active))
    }

    private var active: [MemberDto] { d.members.filter(\.active) }
    private var code: String { currency.trimmingCharacters(in: .whitespaces).uppercased() }
    private var exponent: Int? { model.exponent(of: code) }
    private var foreign: Bool { code != d.group.baseCurrency }
    private var amountMinor: Int64? { exponent.flatMap { Money.parse(amount, exponent: $0) }.flatMap { $0 > 0 ? $0 : nil } }
    private var rateValue: String? { foreign ? Money.parseRate(rate) : nil }
    private var split: SplitResult {
        Split.shares(mode: mode, members: active.map(\.id), among: among, values: values,
                     totalMinor: amountMinor, exponent: exponent ?? 2)
    }
    /// Each person's share in the expense currency, exactly as the core will split it.
    private var preview: [String: Int64] {
        let split = self.split
        guard let total = amountMinor, split.problem == nil, !split.shares.isEmpty else { return [:] }
        return Dictionary(zip(split.shares.map(\.memberId), model.previewShares(total, shares: split.shares)),
                          uniquingKeysWith: { a, _ in a })
    }
    private var dirty: Bool { !title.isEmpty || !amount.isEmpty }
    private var canSave: Bool {
        let split = self.split
        return amountMinor != nil && split.problem == nil && !split.shares.isEmpty && !payerId.isEmpty
            && (!foreign || rateValue != nil) && !saving
    }

    var body: some View {
        NavigationStack {
            Form {
                Section {
                    TextField("Description", text: $title)
                    HStack {
                        TextField("Amount", text: $amount)
                            .keyboardType(.decimalPad)
                            .monospacedDigit()
                        TextField("Currency", text: $currency)
                            .textInputAutocapitalization(.characters)
                            .autocorrectionDisabled()
                            .multilineTextAlignment(.trailing)
                            .frame(width: 70)
                    }
                    if !amount.isEmpty && amountMinor == nil {
                        Text("Enter a valid positive amount.").font(.caption).foregroundStyle(Color.coral)
                    }
                    if exponent == nil {
                        Text("Use a three-letter currency code, such as EUR or USD.")
                            .font(.caption).foregroundStyle(Color.coral)
                    } else if foreign {
                        TextField("Rate: 1 \(code) = ? \(d.group.baseCurrency)", text: $rate)
                            .keyboardType(.decimalPad)
                    }
                }
                Section {
                    Picker("Paid by", selection: $payerId) {
                        ForEach(active, id: \.id) { m in Text(m.displayName).tag(m.id) }
                    }
                    DatePicker("Date", selection: $date, displayedComponents: .date)
                }
                Section {
                    SplitModePicker(mode: $mode)
                    ForEach(active, id: \.id) { m in
                        SplitPersonRow(
                            name: m.id == d.meId ? String(localized: "\(m.displayName) (you)") : m.displayName,
                            id: m.id, mode: mode, included: among.contains(m.id),
                            toggle: { if among.contains(m.id) { among.remove(m.id) } else { among.insert(m.id) } },
                            value: Binding(get: { values[m.id] ?? "" }, set: { values[m.id] = $0 }),
                            share: exponent.flatMap { exp in preview[m.id].map { Money.format($0, exponent: exp, code: code) } },
                            unit: code)
                    }
                } header: {
                    HStack {
                        Text("Split")
                        Spacer()
                        if mode == .equal {
                            Button(among.count == active.count ? LocalizedStringKey("No one") : LocalizedStringKey("Everyone")) {
                                among = among.count == active.count ? [] : Set(active.map(\.id))
                            }
                            .font(.caption)
                        }
                    }
                } footer: {
                    if let text = splitProblemText(split.problem, exponent: exponent ?? 2, code: code) {
                        Text(text).foregroundStyle(Color.coral)
                    }
                }
            }
            .navigationTitle("New expense")
            .navigationBarTitleDisplayMode(.inline)
            .interactiveDismissDisabled(dirty)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Cancel") { if dirty { confirmDiscard = true } else { dismiss() } }
                }
                ToolbarItem(placement: .confirmationAction) {
                    Button("Save") { save() }.disabled(!canSave)
                }
            }
            .alert("DiBillSion", isPresented: Binding(get: { model.message != nil },
                                                      set: { if !$0 { model.message = nil } })) {
                Button("OK", role: .cancel) {}
            } message: {
                Text(model.message ?? "")
            }
            .confirmationDialog("Discard what you have entered?", isPresented: $confirmDiscard, titleVisibility: .visible) {
                Button("Discard", role: .destructive) { dismiss() }
            }
        }
    }

    private func save() {
        guard let total = amountMinor else { return }
        saving = true
        Task {
            let ok = await model.addExpense(
                title: title, amountMinor: total, currency: code, fxRate: rateValue,
                payerId: payerId, shares: split.shares, dateDays: DateDays.from(date))
            saving = false
            if ok { dismiss() }
        }
    }
}
