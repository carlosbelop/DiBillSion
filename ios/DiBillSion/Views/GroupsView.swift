import SwiftUI

/// The home screen: active groups, then archived ones.
struct GroupsView: View {
    @Environment(AppModel.self) private var model
    @State private var creating = false
    @State private var joining = false

    var body: some View {
        @Bindable var model = model
        NavigationStack(path: $model.path) {
            List {
                let active = model.groups.filter { !$0.archived }
                let archived = model.groups.filter(\.archived)
                if model.groups.isEmpty {
                    emptyState
                }
                Section {
                    ForEach(active, id: \.id) { row($0) }
                }
                if !archived.isEmpty {
                    Section("Archived") {
                        ForEach(archived, id: \.id) { row($0) }
                    }
                }
            }
            .listStyle(.insetGrouped)
            .navigationTitle("Your groups")
            .navigationDestination(for: String.self) { GroupView(groupId: $0) }
            .refreshable { await model.refresh() }
            .toolbar {
                ToolbarItem(placement: .topBarLeading) {
                    Button("Join") { joining = true }
                }
                ToolbarItem(placement: .topBarTrailing) {
                    Button { creating = true } label: { Label("New group", systemImage: "plus") }
                }
            }
            .sheet(isPresented: $creating) { NewGroupView() }
            .sheet(isPresented: $joining) { JoinView() }
            .task { await model.refreshGroups() }
        }
        .alert("DiBillSion", isPresented: Binding(get: { model.message != nil },
                                                  set: { if !$0 { model.message = nil } })) {
            Button("OK", role: .cancel) {}
        } message: {
            Text(model.message ?? "")
        }
    }

    private var emptyState: some View {
        VStack(spacing: 12) {
            Image(systemName: "person.3.fill").font(.largeTitle).foregroundStyle(Color.teal)
            Text("Split your first bill").font(.headline)
            Text("Create a group for a trip, a shared flat or a dinner, add the people, and DiBillSion keeps the accounts exact to the cent.")
                .font(.subheadline).foregroundStyle(.secondary).multilineTextAlignment(.center)
            Button("New group") { creating = true }.buttonStyle(.borderedProminent)
        }
        .frame(maxWidth: .infinity)
        .padding(.vertical, 24)
        .listRowBackground(Color.clear)
    }

    private func row(_ g: GroupDto) -> some View {
        NavigationLink(value: g.id) {
            HStack(spacing: 12) {
                Avatar(id: g.id, name: g.name, emoji: g.emoji, size: 44)
                VStack(alignment: .leading, spacing: 2) {
                    Text(g.name).font(.headline)
                    let people = model.memberCounts[g.id] ?? 0
                    Group {
                        if model.sharedIds.contains(g.id) {
                            Text("\(people) people · Shared")
                        } else {
                            Text("\(people) people")
                        }
                    }
                    .font(.caption).foregroundStyle(.secondary)
                    if let mine = g.myBalanceMinor {
                        let exp = model.exponent(of: g.baseCurrency) ?? 2
                        MyBalanceLabel(minor: mine) { Money.format($0, exponent: exp, code: g.baseCurrency) }
                            .font(.caption.weight(.medium))
                    }
                }
            }
            .padding(.vertical, 4)
        }
    }
}

/// Paste an invite link or code to join a shared group.
struct JoinView: View {
    @Environment(AppModel.self) private var model
    @Environment(\.dismiss) private var dismiss
    @State private var text = ""

    var body: some View {
        NavigationStack {
            Form {
                Section {
                    TextField("Invite link or code", text: $text)
                        .textInputAutocapitalization(.never)
                        .autocorrectionDisabled()
                    PasteButton(payloadType: String.self) { strings in
                        if let first = strings.first { text = first }
                    }
                } footer: {
                    Text("Paste the invite link or code someone sent you.")
                }
            }
            .navigationTitle("Join a group")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) { Button("Cancel") { dismiss() } }
                ToolbarItem(placement: .confirmationAction) {
                    Button("Join") {
                        let code = text
                        dismiss()
                        Task { await model.join(code) }
                    }
                    .disabled(text.trimmingCharacters(in: .whitespaces).isEmpty)
                }
            }
        }
        .presentationDetents([.medium])
    }
}
