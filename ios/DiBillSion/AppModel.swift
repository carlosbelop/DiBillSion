import Foundation
import Observation

/// Everything one group's screen shows.
struct GroupDetail: Sendable {
    var group: GroupDto
    var members: [MemberDto]
    var transactions: [TransactionDto]
    var balances: BalancesDto
    var exponent: Int
    /// Synced through the server (ADR 0007).
    var shared: Bool

    func name(_ memberId: String) -> String {
        members.first { $0.id == memberId }?.displayName ?? "?"
    }

    /// Who uses this device in the group, if chosen.
    var meId: String? { group.meId }

    func format(_ minor: Int64) -> String {
        Money.format(minor, exponent: exponent, code: group.baseCurrency)
    }
}

/// A receipt being read or reviewed in the open group.
enum ScanState: Equatable {
    /// The photo is on its way to the server's model.
    case reading
    case review(ReceiptDraft)
}

struct PendingInvite: Identifiable {
    let groupName: String
    let code: String
    var id: String { code }
    var link: String { "dibillsion://join/\(code)" }
}

/// Talks to the Rust core. Every core call is short and blocking, so it runs off the
/// main actor; views only read the published state.
@MainActor
@Observable
final class AppModel {
    private(set) var groups: [GroupDto] = []
    /// People per group, for the group cards.
    private(set) var memberCounts: [String: Int] = [:]
    /// Groups synced through the server, for the cards.
    private(set) var sharedIds: Set<String> = []
    /// The open group, if any.
    private(set) var detail: GroupDetail?
    /// Navigation path of the groups list (group ids).
    var path: [String] = []
    /// A message for the user; cleared when dismissed.
    var message: String?
    /// An invite ready for the share sheet; cleared once shown.
    var invite: PendingInvite?
    /// A receipt being read or reviewed.
    private(set) var scan: ScanState?
    private var scanTask: Task<Void, Never>?

    private let core: Core
    /// Nil when this build has no server.
    private let transport: SyncTransport?

    init(core: Core, transport: SyncTransport?) {
        self.core = core
        self.transport = transport
    }

    /// The app's model: the database lives in Application Support.
    static func live() -> AppModel {
        let dir = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0]
        try? FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        let path = dir.appendingPathComponent("dibill.db").path
        do {
            return AppModel(core: try Core.open(path: path), transport: SyncTransport.fromBundle())
        } catch {
            // Without its database the app can't work at all; crash with the reason.
            fatalError("could not open \(path): \(error)")
        }
    }

    var canSync: Bool { transport != nil }

    func exponent(of code: String) -> Int? {
        (try? currencyExponent(code: code.uppercased())).map(Int.init)
    }

    // MARK: - Loading

    func refreshGroups() async {
        await run { core in try Self.loadGroups(core) } apply: { self.apply($0) }
    }

    /// Pull-to-refresh: syncs with the server, then reloads.
    func refresh() async {
        let started = Date()
        if let transport {
            await run { core in _ = try core.sync(transport: transport) }
        }
        await refreshGroups()
        if let id = detail?.group.id { await reload(id) }
        // Keep the indicator visible long enough to read as feedback.
        let left = 0.45 - Date().timeIntervalSince(started)
        if left > 0 { try? await Task.sleep(nanoseconds: UInt64(left * 1_000_000_000)) }
    }

    func open(_ groupId: String) async {
        await reload(groupId)
        syncQuietly()
    }

    func close() {
        closeScan()
        detail = nil
        Task { await refreshGroups() }
    }

    // MARK: - Groups

    /// Creates a group; `me` (if given) becomes "who I am" on this device.
    func createGroup(name: String, emoji: String?, kind: GroupKindDto, currency: String,
                     me: String, others: [String]) async {
        let name = name.trimmingCharacters(in: .whitespaces)
        let currency = currency.trimmingCharacters(in: .whitespaces).uppercased()
        let me = me.trimmingCharacters(in: .whitespaces)
        let others = others.map { $0.trimmingCharacters(in: .whitespaces) }.filter { !$0.isEmpty }
        let id = await run { core -> String in
            let id = try core.createGroup(name: name, emoji: emoji, kind: kind, baseCurrency: currency)
            if !me.isEmpty {
                try core.setMe(groupId: id, memberId: try core.addMember(groupId: id, displayName: me))
            }
            for other in others { _ = try core.addMember(groupId: id, displayName: other) }
            return id
        }
        guard let id else { return }
        await refreshGroups()
        path = [id]
    }

    func setMe(_ memberId: String?) async {
        await inGroup { core, d in try core.setMe(groupId: d.group.id, memberId: memberId) }
    }

    func setArchived(_ archived: Bool) async {
        await inGroup { core, d in try core.setGroupArchived(groupId: d.group.id, archived: archived) }
    }

    func deleteGroup() async {
        guard let id = detail?.group.id else { return }
        await run { core in try core.deleteGroup(groupId: id) }
        detail = nil
        path = []
        await refreshGroups()
    }

    // MARK: - People

    func addMember(_ name: String) async {
        let name = name.trimmingCharacters(in: .whitespaces)
        guard !name.isEmpty else { return }
        await inGroup { core, d in _ = try core.addMember(groupId: d.group.id, displayName: name) }
    }

    func setMemberActive(_ id: String, _ active: Bool) async {
        await inGroup { core, _ in try core.setMemberActive(memberId: id, active: active) }
    }

    func deleteMember(_ id: String) async {
        await inGroup { core, _ in try core.deleteMember(memberId: id) }
    }

    // MARK: - Transactions

    /// What each member's equal share of `totalMinor` comes to, computed by the core so
    /// the editor shows the same cents that will be saved. Empty when it can't split.
    func previewEqualSplit(_ totalMinor: Int64, among: [String]) -> [Int64] {
        (try? previewSplit(totalMinor: totalMinor,
                           shares: among.map { ShareDto(memberId: $0, parts: 1, amountMinor: nil) })) ?? []
    }

    /// An equal split of `amountMinor` between `among`, paid by `payerId`. True when saved.
    func addExpense(title: String, amountMinor: Int64, currency: String, fxRate: String?,
                    payerId: String, among: [String], dateDays: Int32) async -> Bool {
        let title = title.trimmingCharacters(in: .whitespaces)
        var saved = false
        await inGroup { core, d in
            _ = try core.saveTransaction(t: TransactionDto(
                id: nil, groupId: d.group.id, kind: .expense, title: title, payerId: payerId,
                currency: currency, fxRate: fxRate, dateDays: dateDays,
                items: [ItemDto(label: nil, totalMinor: amountMinor,
                                shares: among.map { ShareDto(memberId: $0, parts: 1, amountMinor: nil) })],
                baseTotalMinor: nil))
        } onSuccess: { saved = true }
        return saved
    }

    /// Records a suggested reimbursement as paid (a transfer in the base currency).
    func markPaid(_ s: SuggestionDto) async {
        await inGroup { core, d in
            _ = try core.saveTransaction(t: TransactionDto(
                id: nil, groupId: d.group.id, kind: .transfer, title: "", payerId: s.fromId,
                currency: d.group.baseCurrency, fxRate: nil, dateDays: DateDays.today,
                items: [ItemDto(label: nil, totalMinor: s.amountMinor,
                                shares: [ShareDto(memberId: s.toId, parts: nil, amountMinor: s.amountMinor)])],
                baseTotalMinor: nil))
        }
    }

    func deleteTransaction(_ id: String) async {
        await inGroup { core, _ in try core.deleteTransaction(transactionId: id) }
    }

    // MARK: - Receipts (ADR 0008)

    /// Reads a receipt photo (already downscaled JPEG) through the server, then opens the
    /// review with everyone on every item.
    func scanReceipt(jpeg: Data) {
        guard let transport = transportOrExplain(), let d = detail else { return }
        scan = .reading
        scanTask?.cancel()
        let groupId = d.group.id
        scanTask = Task {
            let scanned = await run { core in
                try core.scanReceipt(transport: transport, groupId: groupId, image: jpeg, mime: "image/jpeg")
            }
            guard !Task.isCancelled, scan == .reading else { return }
            guard let scanned else {
                scan = nil
                return
            }
            let active = d.members.filter(\.active)
            let payer = d.meId.flatMap { me in active.contains { $0.id == me } ? me : nil } ?? active.first?.id ?? ""
            scan = .review(ReceiptDraft.from(
                scanned, exponent: exponent(of: scanned.currency) ?? d.exponent,
                everyone: Set(active.map(\.id)), payerId: payer, today: DateDays.today,
                untitled: String(localized: "Receipt")))
        }
    }

    /// Couldn't turn the picked image into a JPEG.
    func receiptImageFailed() {
        message = String(localized: "Couldn’t open that image. Try another one.")
    }

    func editReceipt(_ change: (inout ReceiptDraft) -> Void) {
        guard case .review(var draft) = scan else { return }
        change(&draft)
        scan = .review(draft)
    }

    /// Leaves the scan: a reading in flight is dropped when it comes back.
    func closeScan() {
        scanTask?.cancel()
        scanTask = nil
        scan = nil
    }

    /// Saves the reviewed receipt as one expense with an item per line. The draft stays
    /// open if saving fails, so nothing typed is lost.
    func saveReceipt() async {
        guard case .review(let draft) = scan, let d = detail,
              let exp = exponent(of: draft.currency) else { return }
        let foreign = draft.currency != d.group.baseCurrency
        let items = draft.items(exponent: exp, memberOrder: d.members.map(\.id))
        let title = draft.title.trimmingCharacters(in: .whitespaces)
        let rate = foreign ? Money.parseRate(draft.rate) : nil
        await inGroup { core, d in
            _ = try core.saveTransaction(t: TransactionDto(
                id: nil, groupId: d.group.id, kind: .expense, title: title, payerId: draft.payerId,
                currency: draft.currency, fxRate: rate, dateDays: draft.dateDays, items: items,
                baseTotalMinor: nil))
        } onSuccess: { scan = nil }
    }

    /// What each member owes for the draft, summed over its items exactly as the core splits them.
    func receiptShares(_ draft: ReceiptDraft, memberOrder: [String]) -> [String: Int64] {
        guard let exp = exponent(of: draft.currency) else { return [:] }
        var totals: [String: Int64] = [:]
        for item in draft.items(exponent: exp, memberOrder: memberOrder) {
            guard let parts = try? previewSplit(totalMinor: item.totalMinor, shares: item.shares) else { return [:] }
            for (share, part) in zip(item.shares, parts) { totals[share.memberId, default: 0] += part }
        }
        return totals
    }

    // MARK: - Sharing (ADR 0006/0007)

    /// Shares the open group (first time) and prepares its invite for the share sheet.
    /// Only the person who shared it can invite.
    func shareGroup() async {
        guard let transport = transportOrExplain(), let d = detail else { return }
        let id = d.group.id
        let code = await run { core -> String in
            if try !core.isShared(groupId: id) { _ = try core.shareGroup(transport: transport, groupId: id) }
            return try core.inviteCode(transport: transport, groupId: id)
        }
        guard let code else { return }
        await reload(id)
        await refreshGroups()
        invite = PendingInvite(groupName: d.group.name, code: code)
    }

    /// Turns the invite link off; inviting again makes a new one.
    func turnOffInvite() async {
        guard let transport = transportOrExplain(), let id = detail?.group.id else { return }
        let done: Void? = await run { core in try core.turnOffInvite(transport: transport, groupId: id) }
        if done != nil {
            message = String(localized: "The invite link is off. Inviting again creates a new link.")
        }
    }

    /// Joins from an invite link or a bare code, then opens the group.
    func join(_ linkOrCode: String) async {
        guard let transport = transportOrExplain() else { return }
        let code = Self.inviteCode(from: linkOrCode)
        let id = await run { core in try core.joinGroup(transport: transport, code: code) }
        guard let id else { return }
        await refreshGroups()
        path = [id]
    }

    /// "dibillsion://join/abc?x" or "https://…/join/abc" or " abc " -> "abc".
    nonisolated static func inviteCode(from linkOrCode: String) -> String {
        let trimmed = linkOrCode.trimmingCharacters(in: .whitespacesAndNewlines)
        let last = trimmed.split(separator: "/").last.map(String.init) ?? trimmed
        return String(last.split(separator: "?", omittingEmptySubsequences: false).first ?? "")
    }

    private func transportOrExplain() -> SyncTransport? {
        if transport == nil {
            message = String(localized: "This version of the app isn’t connected to a sync server yet.")
        }
        return transport
    }

    /// Background sync: offline or server trouble stays quiet until the next try.
    private func syncQuietly() {
        guard let transport else { return }
        Task {
            let summary = await run(quiet: true) { core in try core.sync(transport: transport) }
            if let summary, summary.pulled > 0 {
                await refreshGroups()
                if let id = detail?.group.id { await reload(id) }
            }
        }
    }

    // MARK: - Plumbing

    private struct Loaded: Sendable {
        var groups: [GroupDto]
        var counts: [String: Int]
        var shared: Set<String>
    }

    nonisolated private static func loadGroups(_ core: Core) throws -> Loaded {
        let groups = try core.groups()
        var counts: [String: Int] = [:]
        var shared: Set<String> = []
        for g in groups {
            counts[g.id] = try core.members(groupId: g.id).count
            if try core.isShared(groupId: g.id) { shared.insert(g.id) }
        }
        return Loaded(groups: groups, counts: counts, shared: shared)
    }

    private func apply(_ loaded: Loaded) {
        groups = loaded.groups
        memberCounts = loaded.counts
        sharedIds = loaded.shared
    }

    private func reload(_ groupId: String) async {
        let loaded = await run { core -> GroupDetail? in
            guard let group = try core.groups().first(where: { $0.id == groupId }) else { return nil }
            return GroupDetail(
                group: group,
                members: try core.members(groupId: groupId),
                transactions: try core.transactions(groupId: groupId),
                balances: try core.balances(groupId: groupId),
                exponent: Int(try currencyExponent(code: group.baseCurrency)),
                shared: try core.isShared(groupId: groupId))
        }
        if let loaded { detail = loaded }
    }

    /// Runs `work` against the open group, then reloads it and the cards, and syncs.
    private func inGroup(_ work: @escaping @Sendable (Core, GroupDetail) throws -> Void,
                         onSuccess: () -> Void = {}) async {
        guard let d = detail else { return }
        let ok: Void? = await run { core in try work(core, d) }
        guard ok != nil else { return }
        onSuccess()
        await reload(d.group.id)
        await refreshGroups() // the cards show my balance
        syncQuietly()
    }

    /// Runs blocking core work off the main actor. Errors become `message` (unless
    /// `quiet`) and a nil result.
    @discardableResult
    private func run<T>(quiet: Bool = false, _ work: @escaping @Sendable (Core) throws -> T,
                        apply: (T) -> Void = { _ in }) async -> T? {
        let core = self.core
        let result = await Task.detached(priority: .userInitiated) { () -> Result<T, Error> in
            Result { try work(core) }
        }.value
        switch result {
        case .success(let value):
            apply(value)
            return value
        case .failure(let error):
            if !quiet { message = Self.describe(error) }
            return nil
        }
    }

    /// A message for the user in their language.
    nonisolated static func describe(_ error: Error) -> String {
        guard let e = error as? CoreError else {
            return String(localized: "Something in the form isn’t valid.")
        }
        switch e {
        case .NotFound: return String(localized: "That item no longer exists.")
        case .UnknownCurrency(let code):
            return String(localized: "“\(code)” isn’t a currency we know. Use a three-letter code such as EUR.")
        case .MissingRate: return String(localized: "Enter the exchange rate.")
        case .InvalidRate: return String(localized: "The exchange rate must be a number greater than zero.")
        case .UnknownMember: return String(localized: "That person isn’t in this group.")
        case .MemberInactive:
            return String(localized: "Someone in this expense no longer takes part in new expenses.")
        case .MemberHasHistory:
            return String(localized: "This person already appears in expenses, so they can’t be removed. Deactivate them instead.")
        case .InvalidSplit: return String(localized: "The split doesn’t add up. Check the amounts and who is included.")
        case .InvalidInput: return String(localized: "Something in the form isn’t valid.")
        case .Storage: return String(localized: "Couldn’t save to this device. Try again.")
        case .Offline:
            return String(localized: "No connection. Your changes are saved on this phone and will sync later.")
        case .Server(let code, _):
            switch code {
            case "not_found": return String(localized: "That invite link isn’t valid any more. Ask for a new one.")
            case "forbidden": return String(localized: "Only the person who shared this group can do that.")
            case "receipts_disabled":
                return String(localized: "Receipt scanning isn’t available on this server yet.")
            case "limit_reached": return String(localized: "No receipt scans left for now. Try again tomorrow.")
            case "unreadable":
                return String(localized: "Couldn’t read a receipt in that photo. Try again with the whole receipt flat, in focus and well lit.")
            case "provider_failed": return String(localized: "The receipt reader isn’t responding. Try again in a moment.")
            case "too_large": return String(localized: "Couldn’t open that image. Try another one.")
            default: return String(localized: "The server couldn’t handle that right now. Try again in a moment.")
            }
        }
    }
}
