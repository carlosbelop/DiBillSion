import SwiftUI

/// One expense or income with its items (ADR 0009): anyone can add or remove people on each
/// item, and the per-person totals follow, computed by the core.
struct ExpenseDetailView: View {
    let transactionId: String
    @Environment(AppModel.self) private var model
    @Environment(\.dismiss) private var dismiss
    @State private var confirmDelete = false

    var body: some View {
        NavigationStack {
            if let d = model.detail, let t = d.transactions.first(where: { $0.id == transactionId }) {
                content(d, t)
            } else {
                // Deleted here or on another phone.
                ContentUnavailableView("That item no longer exists.", systemImage: "trash")
                    .toolbar { ToolbarItem(placement: .confirmationAction) { Button("OK") { dismiss() } } }
            }
        }
    }

    private func content(_ d: GroupDetail, _ t: TransactionDto) -> some View {
        let exponent = model.exponent(of: t.currency) ?? d.exponent
        let fmt = { (m: Int64) in Money.format(m, exponent: exponent, code: t.currency) }
        let shares = t.items.map(model.itemShares)
        var owed: [String: Int64] = [:]
        for (item, parts) in zip(t.items, shares) {
            for (s, p) in zip(item.shares, parts) { owed[s.memberId, default: 0] += p }
        }
        return List {
            Section {
                VStack(alignment: .leading, spacing: 4) {
                    Text(fmt(t.items.reduce(0) { $0 + $1.totalMinor }))
                        .font(.title.weight(.semibold).monospacedDigit())
                    if let base = t.baseTotalMinor, t.currency != d.group.baseCurrency {
                        Text("About \(d.format(base))").foregroundStyle(.secondary)
                    }
                    Text(t.kind == .income ? "Received by \(d.name(t.payerId))" : "Paid by \(d.name(t.payerId))")
                    Text(DateDays.date(t.dateDays).formatted(date: .abbreviated, time: .omitted) + " · "
                         + String(localized: "\(t.items.count) items"))
                        .font(.subheadline).foregroundStyle(.secondary)
                }
                .padding(.vertical, 4)
            } footer: {
                Text("Tap people to add or remove them from an item. Everyone in the group sees the change.")
            }

            ForEach(Array(t.items.enumerated()), id: \.offset) { index, item in
                Section { itemRow(d, t, index, item, shares[index], fmt) }
            }

            Section("Per person") {
                ForEach(d.members.filter { owed[$0.id] != nil }, id: \.id) { m in
                    HStack {
                        Avatar(id: m.id, name: m.displayName, size: 28)
                        Text(m.id == d.meId ? String(localized: "\(m.displayName) (you)") : m.displayName)
                        Spacer()
                        Text(fmt(owed[m.id] ?? 0)).monospacedDigit().fontWeight(.semibold)
                    }
                }
            }
        }
        .navigationTitle(t.title)
        .navigationBarTitleDisplayMode(.inline)
        .refreshable { await model.refresh() }
        .toolbar {
            ToolbarItem(placement: .confirmationAction) { Button("OK") { dismiss() } }
            ToolbarItem(placement: .destructiveAction) {
                Button(role: .destructive) { confirmDelete = true } label: { Image(systemName: "trash") }
                    .accessibilityLabel("Delete")
            }
        }
        .confirmationDialog("Delete “\(t.title)”?", isPresented: $confirmDelete, titleVisibility: .visible) {
            Button("Delete", role: .destructive) {
                dismiss()
                Task { await model.deleteTransaction(transactionId) }
            }
        }
    }

    @ViewBuilder
    private func itemRow(_ d: GroupDetail, _ t: TransactionDto, _ index: Int, _ item: ItemDto,
                         _ parts: [Int64], _ fmt: @escaping (Int64) -> String) -> some View {
        let inItem = Set(item.shares.map(\.memberId))
        let byParts = item.shares.allSatisfy { $0.parts != nil }
        HStack {
            Text(item.label ?? t.title).font(.headline)
            Spacer()
            Text(fmt(item.totalMinor)).monospacedDigit()
        }
        ScrollView(.horizontal, showsIndicators: false) {
            HStack(spacing: 8) {
                // Active people, plus inactive ones still in the item (they can only leave).
                ForEach(d.members.filter { $0.active || inItem.contains($0.id) }, id: \.id) { m in
                    let on = inItem.contains(m.id)
                    let enabled = byParts && !(on && inItem.count == 1) && (on || m.active)
                    Button {
                        Task {
                            await model.setItemMember(transactionId: transactionId, itemIndex: index,
                                                      memberId: m.id, included: !on)
                        }
                    } label: {
                        Text(m.displayName).font(.subheadline)
                            .padding(.horizontal, 12).padding(.vertical, 6)
                            .background(Capsule().fill(on ? Color.teal : Color(.secondarySystemFill)))
                            .foregroundStyle(on ? .white : .primary)
                    }
                    .buttonStyle(.plain)
                    .disabled(!enabled)
                    .opacity(enabled ? 1 : 0.6)
                }
            }
        }
        if !byParts {
            Text("Split by exact amounts: change it from the editor.").font(.caption).foregroundStyle(.secondary)
        } else if inItem.count > 1, let low = parts.min(), let high = parts.max() {
            // Cents left over go to some people, so a range when they differ.
            Text("\(low == high ? fmt(high) : fmt(low) + "–" + fmt(high)) each")
                .font(.caption).foregroundStyle(.secondary)
        }
    }
}
