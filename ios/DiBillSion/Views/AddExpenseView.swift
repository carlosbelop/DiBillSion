import SwiftUI

/// A new expense split equally between the people ticked, with each person's exact share
/// computed live by the core.
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
    /// Ticked people in member order, so the preview lines up with the list.
    private var chosen: [String] { active.map(\.id).filter(among.contains) }
    private var preview: [String: Int64] {
        guard let total = amountMinor, !chosen.isEmpty else { return [:] }
        return Dictionary(uniqueKeysWithValues: zip(chosen, model.previewEqualSplit(total, among: chosen)))
    }
    private var dirty: Bool { !title.isEmpty || !amount.isEmpty }
    private var canSave: Bool {
        amountMinor != nil && !chosen.isEmpty && !payerId.isEmpty && (!foreign || rateValue != nil) && !saving
    }

    var body: some View {
        NavigationStack {
            Form {
                Section {
                    TextField("Description", text: $title)
                    HStack {
                        TextField("Amount", text: $amount)
                            .keyboardType(.decimalPad)
                            .font(.title2.monospacedDigit())
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
                    ForEach(active, id: \.id) { m in
                        Button {
                            if among.contains(m.id) { among.remove(m.id) } else { among.insert(m.id) }
                        } label: {
                            HStack {
                                Image(systemName: among.contains(m.id) ? "checkmark.circle.fill" : "circle")
                                    .foregroundStyle(among.contains(m.id) ? Color.teal : .secondary)
                                Text(m.id == d.meId ? String(localized: "\(m.displayName) (you)") : m.displayName)
                                    .foregroundStyle(.primary)
                                Spacer()
                                if let share = preview[m.id], let exp = exponent {
                                    Text(Money.format(share, exponent: exp, code: code))
                                        .monospacedDigit().foregroundStyle(.secondary)
                                }
                            }
                        }
                    }
                } header: {
                    HStack {
                        Text("Split equally")
                        Spacer()
                        Button(among.count == active.count ? LocalizedStringKey("No one") : LocalizedStringKey("Everyone")) {
                            among = among.count == active.count ? [] : Set(active.map(\.id))
                        }
                        .font(.caption)
                    }
                } footer: {
                    if chosen.isEmpty { Text("Pick at least one person.").foregroundStyle(Color.coral) }
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
                payerId: payerId, among: chosen, dateDays: DateDays.from(date))
            saving = false
            if ok { dismiss() }
        }
    }
}
