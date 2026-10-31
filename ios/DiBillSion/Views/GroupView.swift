import SwiftUI

private enum Tab: Hashable { case expenses, balances, people }

/// One group: expenses, balances (with settle) and people, plus sharing and "who am I".
struct GroupView: View {
    let groupId: String
    @Environment(AppModel.self) private var model
    @State private var tab = Tab.expenses
    @State private var addingExpense = false
    @State private var choosingMe = false
    @State private var askedMe = false
    @State private var confirmDelete = false

    var body: some View {
        Group {
            if let d = model.detail, d.group.id == groupId {
                content(d)
            } else {
                ProgressView()
            }
        }
        .task(id: groupId) { await model.open(groupId) }
        .onDisappear { if model.path.isEmpty { model.close() } }
    }

    @ViewBuilder
    private func content(_ d: GroupDetail) -> some View {
        List {
            Section { header(d) }
            Picker("", selection: $tab) {
                Text("Expenses").tag(Tab.expenses)
                Text("Balances").tag(Tab.balances)
                Text("People").tag(Tab.people)
            }
            .pickerStyle(.segmented)
            .listRowBackground(Color.clear)
            .listRowInsets(EdgeInsets())
            switch tab {
            case .expenses: ExpensesSection(d: d)
            case .balances: BalancesSection(d: d)
            case .people: PeopleSection(d: d)
            }
        }
        .listStyle(.insetGrouped)
        .navigationTitle(d.group.name)
        .navigationBarTitleDisplayMode(.inline)
        .refreshable { await model.refresh() }
        .toolbar {
            ToolbarItem(placement: .topBarTrailing) { menu(d) }
            ToolbarItem(placement: .bottomBar) {
                ScanReceiptButton().disabled(d.members.filter(\.active).isEmpty)
            }
            ToolbarItem(placement: .bottomBar) {
                Button { addingExpense = true } label: {
                    Label("Add expense", systemImage: "plus.circle.fill").labelStyle(.titleAndIcon)
                }
                .buttonStyle(.borderedProminent)
                .disabled(d.members.filter(\.active).isEmpty)
            }
        }
        .sheet(isPresented: $addingExpense) { AddExpenseView(d: d) }
        .fullScreenCover(isPresented: Binding(get: { model.scan != nil },
                                              set: { if !$0 { model.closeScan() } })) {
            ReceiptFlowView()
        }
        .sheet(isPresented: $choosingMe) { WhoAmIView(d: d) }
        .sheet(item: Binding(get: { model.invite }, set: { model.invite = $0 })) { invite in
            ShareSheet(items: [String(localized: "Join “\(invite.groupName)” on DiBillSion: \(invite.link) (invite code: \(invite.code))")])
                .presentationDetents([.medium, .large])
        }
        .confirmationDialog("Delete “\(d.group.name)”?", isPresented: $confirmDelete, titleVisibility: .visible) {
            Button("Delete group", role: .destructive) { Task { await model.deleteGroup() } }
        } message: {
            Text("Its expenses, balances and people are removed from this device. This can’t be undone.")
        }
        .onAppear {
            // Ask once per visit who uses this phone, so balances can say "you".
            if d.meId == nil, !d.members.isEmpty, !askedMe {
                askedMe = true
                choosingMe = true
            }
        }
    }

    private func header(_ d: GroupDetail) -> some View {
        HStack(spacing: 12) {
            Avatar(id: d.group.id, name: d.group.name, emoji: d.group.emoji, size: 52)
            VStack(alignment: .leading, spacing: 4) {
                HStack(spacing: 6) {
                    Text("\(d.members.count) people")
                    if d.shared {
                        Label("Shared", systemImage: "icloud").labelStyle(.titleAndIcon)
                    }
                }
                .font(.caption).foregroundStyle(.secondary)
                if let mine = d.group.myBalanceMinor {
                    MyBalanceLabel(minor: mine, format: d.format).font(.headline)
                } else {
                    Button("Who are you?") { choosingMe = true }.font(.subheadline)
                }
            }
        }
        .padding(.vertical, 4)
    }

    private func menu(_ d: GroupDetail) -> some View {
        Menu {
            Button { Task { await model.shareGroup() } } label: {
                Label("Invite people", systemImage: "person.crop.circle.badge.plus")
            }
            if d.shared {
                Button { Task { await model.turnOffInvite() } } label: {
                    Label("Turn off invite link", systemImage: "link.badge.plus")
                }
            }
            Button { choosingMe = true } label: { Label("Change who I am", systemImage: "person.fill.questionmark") }
            Divider()
            if d.group.archived {
                Button { Task { await model.setArchived(false) } } label: {
                    Label("Restore group", systemImage: "tray.and.arrow.up")
                }
            } else {
                Button { Task { await model.setArchived(true) } } label: {
                    Label("Archive group", systemImage: "archivebox")
                }
            }
            Button(role: .destructive) { confirmDelete = true } label: {
                Label("Delete group", systemImage: "trash")
            }
        } label: {
            Label("More options", systemImage: "ellipsis.circle")
        }
    }
}

// MARK: - Expenses

private struct ExpensesSection: View {
    let d: GroupDetail
    @Environment(AppModel.self) private var model
    @State private var deleting: TransactionDto?

    var body: some View {
        if d.transactions.isEmpty {
            Section {
                VStack(spacing: 8) {
                    Text("No expenses yet").font(.headline)
                    Text("Add what someone paid and it will be split among the group.")
                        .font(.subheadline).foregroundStyle(.secondary).multilineTextAlignment(.center)
                }
                .frame(maxWidth: .infinity).padding(.vertical, 12)
            }
        } else {
            let spent = d.transactions.filter { $0.kind == .expense }
                .reduce(Int64(0)) { $0 + ($1.baseTotalMinor ?? total($1)) }
            Section {
                LabeledContent("Group spending", value: d.format(spent))
            }
            ForEach(days, id: \.self) { day in
                Section(dayLabel(day)) {
                    ForEach(d.transactions.filter { $0.dateDays == day }, id: \.id) { t in
                        row(t)
                            .swipeActions {
                                Button("Delete", role: .destructive) { deleting = t }
                            }
                    }
                }
            }
            .confirmationDialog(deleting.map { String(localized: "Delete “\(title($0))”?") } ?? "",
                                isPresented: Binding(get: { deleting != nil }, set: { if !$0 { deleting = nil } }),
                                titleVisibility: .visible) {
                Button("Delete", role: .destructive) {
                    if let id = deleting?.id { Task { await model.deleteTransaction(id) } }
                }
            }
        }
    }

    /// Newest day first; the core lists transactions newest first already.
    private var days: [Int32] {
        var seen = Set<Int32>()
        return d.transactions.map(\.dateDays).filter { seen.insert($0).inserted }
    }

    private func dayLabel(_ day: Int32) -> String {
        let today = DateDays.today
        if day == today { return String(localized: "Today") }
        if day == today - 1 { return String(localized: "Yesterday") }
        return DateDays.date(day).formatted(date: .abbreviated, time: .omitted)
    }

    private func total(_ t: TransactionDto) -> Int64 { t.items.reduce(0) { $0 + $1.totalMinor } }

    private func title(_ t: TransactionDto) -> String {
        if t.kind == .transfer, let to = t.items.first?.shares.first?.memberId {
            return String(localized: "\(d.name(t.payerId)) paid back \(d.name(to))")
        }
        return t.title
    }

    private func row(_ t: TransactionDto) -> some View {
        let exp = model.exponent(of: t.currency) ?? d.exponent
        return HStack(spacing: 12) {
            Image(systemName: t.kind == .transfer ? "arrow.left.arrow.right"
                  : t.kind == .income ? "arrow.down.circle" : "cart")
                .foregroundStyle(Color.teal).frame(width: 28)
            VStack(alignment: .leading, spacing: 2) {
                Text(title(t)).font(.body)
                Group {
                    switch t.kind {
                    case .expense: Text("Paid by \(d.name(t.payerId))")
                    case .income: Text("Received by \(d.name(t.payerId))")
                    case .transfer: EmptyView()
                    }
                }
                .font(.caption).foregroundStyle(.secondary)
            }
            Spacer()
            VStack(alignment: .trailing, spacing: 2) {
                Text(Money.format(total(t), exponent: exp, code: t.currency)).font(.body.monospacedDigit())
                if let base = t.baseTotalMinor, t.currency != d.group.baseCurrency {
                    Text("About \(d.format(base))").font(.caption).foregroundStyle(.secondary)
                }
            }
        }
    }
}

// MARK: - Balances

private struct BalancesSection: View {
    let d: GroupDetail
    @Environment(AppModel.self) private var model
    @State private var settling: SuggestionDto?

    var body: some View {
        if let me = d.meId, let mine = d.balances.balances.first(where: { $0.memberId == me }) {
            Section("Your balance") {
                MyBalanceLabel(minor: mine.amountMinor, format: d.format).font(.title3.weight(.semibold))
            }
        }
        Section("Who is up, who is down") {
            let largest = d.balances.balances.map { $0.amountMinor.magnitude }.max() ?? 0
            ForEach(d.balances.balances, id: \.memberId) { b in
                BalanceBar(name: b.memberId == d.meId ? String(localized: "\(b.displayName) (you)") : b.displayName,
                           amount: b.amountMinor, largest: largest, text: d.format(b.amountMinor))
            }
        }
        let mineFirst = d.balances.suggestions.filter { involvesMe($0) } + d.balances.suggestions.filter { !involvesMe($0) }
        Section(d.meId == nil ? LocalizedStringKey("Suggested payments") : LocalizedStringKey("Payments")) {
            if mineFirst.isEmpty {
                Text("Everyone is square.").foregroundStyle(.secondary)
            }
            ForEach(mineFirst, id: \.self) { s in
                HStack {
                    VStack(alignment: .leading, spacing: 2) {
                        suggestionText(s)
                        Text(d.format(s.amountMinor)).font(.headline.monospacedDigit())
                    }
                    Spacer()
                    Button("Settle") { settling = s }.buttonStyle(.bordered)
                }
            }
        }
        .confirmationDialog(
            settling.map { String(localized: "Record that \($0.fromName) paid \(d.format($0.amountMinor)) to \($0.toName)?") } ?? "",
            isPresented: Binding(get: { settling != nil }, set: { if !$0 { settling = nil } }),
            titleVisibility: .visible
        ) {
            Button("Mark as paid") { if let s = settling { Task { await model.markPaid(s) } } }
        }
    }

    private func involvesMe(_ s: SuggestionDto) -> Bool { s.fromId == d.meId || s.toId == d.meId }

    private func suggestionText(_ s: SuggestionDto) -> Text {
        if s.fromId == d.meId { return Text("You pay \(s.toName)") }
        if s.toId == d.meId { return Text("\(s.fromName) pays you") }
        return Text("\(s.fromName) pays \(s.toName)")
    }
}

/// A bar centred on zero: right and green when owed, left and coral when owing.
private struct BalanceBar: View {
    let name: String
    let amount: Int64
    let largest: UInt64
    let text: String

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            HStack {
                Text(name)
                Spacer()
                Text(text).monospacedDigit().foregroundStyle(amount < 0 ? Color.coral : amount > 0 ? Color.owed : .secondary)
            }
            GeometryReader { geo in
                let half = geo.size.width / 2
                // Display only: a ratio of two integers, never money arithmetic.
                let ratio = largest == 0 ? 0 : CGFloat(amount.magnitude) / CGFloat(largest)
                let width = max(ratio * half, amount == 0 ? 0 : 3)
                ZStack(alignment: .leading) {
                    Capsule().fill(Color.secondary.opacity(0.12))
                    Capsule().fill(amount < 0 ? Color.coral : Color.owed)
                        .frame(width: width)
                        .offset(x: amount < 0 ? half - width : half)
                }
            }
            .frame(height: 8)
        }
        .padding(.vertical, 2)
        .accessibilityElement(children: .combine)
    }
}

// MARK: - People

private struct PeopleSection: View {
    let d: GroupDetail
    @Environment(AppModel.self) private var model
    @State private var adding = false
    @State private var newName = ""
    @State private var removing: MemberDto?

    var body: some View {
        Section {
            ForEach(d.members, id: \.id) { m in
                HStack(spacing: 12) {
                    Avatar(id: m.id, name: m.displayName, size: 34)
                    VStack(alignment: .leading) {
                        Text(m.id == d.meId ? String(localized: "\(m.displayName) (you)") : m.displayName)
                        Text(m.active ? LocalizedStringKey("Takes part in new expenses") : LocalizedStringKey("Not in new expenses"))
                            .font(.caption).foregroundStyle(.secondary)
                    }
                    Spacer()
                    Toggle("Takes part in new expenses", isOn: Binding(
                        get: { m.active },
                        set: { on in Task { await model.setMemberActive(m.id, on) } }))
                        .labelsHidden()
                }
                .swipeActions {
                    Button("Remove", role: .destructive) { removing = m }
                }
            }
            Button { adding = true } label: { Label("Add person", systemImage: "person.badge.plus") }
        }
        .alert("Add person", isPresented: $adding) {
            TextField("Name", text: $newName)
            Button("Cancel", role: .cancel) { newName = "" }
            Button("Add") {
                let name = newName
                newName = ""
                Task { await model.addMember(name) }
            }
        }
        .confirmationDialog(removing.map { String(localized: "Remove \($0.displayName) from the group?") } ?? "",
                            isPresented: Binding(get: { removing != nil }, set: { if !$0 { removing = nil } }),
                            titleVisibility: .visible) {
            Button("Remove", role: .destructive) {
                if let id = removing?.id { Task { await model.deleteMember(id) } }
            }
        }
    }
}

// MARK: - Who am I

/// Pick which person uses this phone; only this phone remembers it.
struct WhoAmIView: View {
    let d: GroupDetail
    @Environment(AppModel.self) private var model
    @Environment(\.dismiss) private var dismiss

    var body: some View {
        NavigationStack {
            List {
                Section {
                    ForEach(d.members.filter(\.active), id: \.id) { m in
                        Button {
                            dismiss()
                            Task { await model.setMe(m.id) }
                        } label: {
                            HStack(spacing: 12) {
                                Avatar(id: m.id, name: m.displayName, size: 34)
                                Text(m.displayName).foregroundStyle(.primary)
                                Spacer()
                                if m.id == d.meId { Image(systemName: "checkmark").foregroundStyle(Color.teal) }
                            }
                        }
                    }
                } footer: {
                    Text("Pick yourself so DiBillSion can show what you owe and are owed. Only this phone remembers it.")
                }
            }
            .navigationTitle("Who are you?")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) { Button("Not now") { dismiss() } }
            }
        }
        .presentationDetents([.medium, .large])
    }
}
