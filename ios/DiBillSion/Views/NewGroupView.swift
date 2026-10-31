import SwiftUI

/// Name, kind, currency and the people; I am the first person.
struct NewGroupView: View {
    @Environment(AppModel.self) private var model
    @Environment(\.dismiss) private var dismiss
    @State private var name = ""
    @State private var emoji = ""
    @State private var kind = GroupKindDto.home
    @State private var currency = Locale.current.currency?.identifier ?? "EUR"
    @State private var me = ""
    @State private var others = [""]

    private var currencyValid: Bool { model.exponent(of: currency) != nil }
    private var canCreate: Bool {
        !name.trimmingCharacters(in: .whitespaces).isEmpty && currencyValid
    }

    var body: some View {
        NavigationStack {
            Form {
                Section {
                    TextField("Group name", text: $name)
                    TextField("Emoji (optional)", text: $emoji)
                        .onChange(of: emoji) { _, new in
                            if new.count > 1 { emoji = String(new.suffix(1)) }
                        }
                    Picker("What is it for?", selection: $kind) {
                        ForEach(GroupKindDto.allCases) { k in
                            Label(k.label, systemImage: k.symbol).tag(k)
                        }
                    }
                }
                Section {
                    TextField("Currency", text: $currency)
                        .textInputAutocapitalization(.characters)
                        .autocorrectionDisabled()
                } footer: {
                    if currencyValid {
                        Text("Three-letter code. Balances are shown in this currency.")
                    } else {
                        Text("Use a three-letter currency code, such as EUR or USD.").foregroundStyle(Color.coral)
                    }
                }
                Section("People") {
                    TextField("Your name", text: $me)
                    ForEach(others.indices, id: \.self) { i in
                        TextField("Person \(i + 2)", text: $others[i])
                    }
                    Button { others.append("") } label: {
                        Label("Add someone else", systemImage: "person.badge.plus")
                    }
                }
            }
            .navigationTitle("New group")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) { Button("Cancel") { dismiss() } }
                ToolbarItem(placement: .confirmationAction) {
                    Button("Create") {
                        let (n, e, k, c, m, o) = (name, emoji, kind, currency, me, others)
                        dismiss()
                        Task {
                            await model.createGroup(name: n, emoji: e.isEmpty ? nil : e, kind: k,
                                                    currency: c, me: m, others: o)
                        }
                    }
                    .disabled(!canCreate)
                }
            }
        }
    }
}
