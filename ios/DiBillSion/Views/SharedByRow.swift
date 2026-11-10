import SwiftUI

/// Who shares an item, in one line; tapping opens a list with a check per person. Easier than a
/// row of names to scroll once a group has more than a few people.
struct SharedByRow: View {
    /// The people who can share it, in member order.
    let people: [MemberDto]
    let selected: Set<String>
    /// "Everyone" can also mean "nobody" (the receipt review lets an item start empty).
    var allowNone = true
    var enabled = true
    let toggle: (String) -> Void
    let setAll: (Bool) -> Void
    @State private var picking = false

    var body: some View {
        Button { picking = true } label: {
            HStack(spacing: 8) {
                Image(systemName: "person.2").foregroundStyle(Color.teal)
                Text(summary).lineLimit(1).foregroundStyle(.primary)
                Spacer()
                Image(systemName: "chevron.right").font(.caption.weight(.semibold)).foregroundStyle(.tertiary)
            }
        }
        .buttonStyle(.borderless)
        .disabled(!enabled)
        .accessibilityHint(Text("Choose who shares it"))
        .sheet(isPresented: $picking) {
            PeopleList(people: people, selected: selected, allowNone: allowNone, toggle: toggle, setAll: setAll)
                .presentationDetents([.medium, .large])
        }
    }

    private var summary: String {
        let names = people.filter { selected.contains($0.id) }.map(\.displayName)
        if names.isEmpty { return String(localized: "Nobody") }
        if names.count == people.count { return String(localized: "Everyone") }
        if names.count <= 2 { return names.joined(separator: ", ") }
        return names.prefix(2).joined(separator: ", ") + " +\(names.count - 2)"
    }
}

private struct PeopleList: View {
    let people: [MemberDto]
    let selected: Set<String>
    let allowNone: Bool
    let toggle: (String) -> Void
    let setAll: (Bool) -> Void
    @Environment(\.dismiss) private var dismiss

    var body: some View {
        NavigationStack {
            List {
                let all = people.allSatisfy { selected.contains($0.id) }
                Section {
                    row(Text("Everyone"), checked: all, disabled: all && !allowNone) { setAll(!all) }
                }
                Section {
                    ForEach(people, id: \.id) { m in
                        let on = selected.contains(m.id)
                        // The last person can't leave unless the item may have nobody.
                        let last = on && selected.count == 1 && !allowNone
                        row(Text(m.displayName), checked: on, disabled: last, avatar: m) { toggle(m.id) }
                    }
                }
            }
            .navigationTitle("Who shares it")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar { ToolbarItem(placement: .confirmationAction) { Button("Done") { dismiss() } } }
        }
    }

    private func row(_ label: Text, checked: Bool, disabled: Bool, avatar: MemberDto? = nil,
                     action: @escaping () -> Void) -> some View {
        Button(action: action) {
            HStack(spacing: 12) {
                if let avatar { Avatar(id: avatar.id, name: avatar.displayName, size: 30) }
                label.foregroundStyle(.primary)
                Spacer()
                Image(systemName: checked ? "checkmark.circle.fill" : "circle")
                    .font(.title3)
                    .foregroundStyle(checked ? Color.teal : .secondary)
            }
        }
        .disabled(disabled)
        .accessibilityAddTraits(checked ? .isSelected : [])
    }
}
