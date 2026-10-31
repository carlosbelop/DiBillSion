package com.dibillsion.app

import android.app.Application
import android.net.Uri
import android.util.Log
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.viewModelScope
import com.dibillsion.core.BalancesDto
import com.dibillsion.core.Core
import com.dibillsion.core.CoreException
import com.dibillsion.core.GroupDto
import com.dibillsion.core.GroupKindDto
import com.dibillsion.core.ItemDto
import com.dibillsion.core.MemberDto
import com.dibillsion.core.ShareDto
import com.dibillsion.core.TransactionDto
import com.dibillsion.core.TransactionKindDto
import com.dibillsion.core.currencyExponent
import com.dibillsion.core.previewSplit
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import java.io.File
import java.io.IOException
import java.time.LocalDate

/** Everything one group's screen shows. */
data class GroupDetail(
    val group: GroupDto,
    val members: List<MemberDto>,
    val transactions: List<TransactionDto>,
    val balances: BalancesDto,
    val exponent: Int,
    /** Synced through the server (ADR 0007). */
    val shared: Boolean = false,
) {
    fun name(memberId: String): String = members.firstOrNull { it.id == memberId }?.displayName ?: "?"

    /** Who uses this device in the group, if chosen. */
    val meId: String? get() = group.meId
}

data class UiState(
    val groups: List<GroupDto> = emptyList(),
    /** People per group, for the group cards. */
    val memberCounts: Map<String, Int> = emptyMap(),
    val detail: GroupDetail? = null,
    val error: String? = null,
    val refreshing: Boolean = false,
    /** Groups synced through the server, for the cards. */
    val sharedIds: Set<String> = emptySet(),
    /** An invite ready to hand to the share sheet; cleared once shown. */
    val invite: PendingInvite? = null,
    /** A receipt being read or reviewed in the open group. */
    val scan: ScanState? = null,
)

sealed interface ScanState {
    /** The photo is on its way to the server's model. */
    data object Reading : ScanState

    data class Review(val draft: ReceiptDraft) : ScanState
}

data class PendingInvite(val groupName: String, val code: String) {
    val link: String get() = "dibillsion://join/$code"
}

/**
 * Talks to the Rust core. Every core call is short and blocking, so it runs on
 * [Dispatchers.IO]; the UI only observes [state].
 */
class AppViewModel(app: Application) : AndroidViewModel(app) {
    private val coreLazy = lazy { Core.open(File(app.filesDir, "dibill.db").absolutePath) }
    private val core: Core by coreLazy

    /** Null when this build has no server (release until hosting is decided). */
    private val transport: SyncTransport? = BuildConfig.SYNC_URL.takeIf { it.isNotEmpty() }?.let(::SyncTransport)

    private val _state = MutableStateFlow(UiState())
    private var scanJob: Job? = null
    val state: StateFlow<UiState> = _state.asStateFlow()

    init {
        refreshGroups()
    }

    fun exponentOf(code: String): Int? = try {
        currencyExponent(code.uppercase()).toInt()
    } catch (_: CoreException) {
        null
    }

    fun dismissError() = _state.update { it.copy(error = null) }

    fun refreshGroups() = launchCore { loadGroups() }

    private fun loadGroups() {
        val groups = core.groups()
        val counts = groups.associate { it.id to core.members(it.id).size }
        val shared = groups.filter { core.isShared(it.id) }.map { it.id }.toSet()
        _state.update { it.copy(groups = groups, memberCounts = counts, sharedIds = shared) }
    }

    /** Creates a group; [me] (if given) becomes "who I am" on this device. */
    fun createGroup(name: String, emoji: String?, kind: GroupKindDto, currency: String, me: String, others: List<String>) =
        launchCore {
            val id = core.createGroup(name.trim(), emoji, kind, currency.trim().uppercase())
            me.trim().takeIf { it.isNotEmpty() }?.let { core.setMe(id, core.addMember(id, it)) }
            others.map { it.trim() }.filter { it.isNotEmpty() }.forEach { core.addMember(id, it) }
            loadGroups()
            loadDetail(id)
        }

    /** Pull-to-refresh: syncs shared groups with the server, then reloads. */
    fun refresh() {
        _state.update { it.copy(refreshing = true) }
        viewModelScope.launch {
            // Keep the indicator visible long enough to read as feedback.
            val minimum = launch { delay(450) }
            withContext(Dispatchers.IO) {
                try {
                    transport?.let { core.sync(it) }
                } catch (e: CoreException) {
                    _state.update { it.copy(error = describe(e)) }
                }
                try {
                    loadGroups()
                    _state.value.detail?.let { loadDetail(it.group.id) }
                } catch (e: CoreException) {
                    _state.update { it.copy(error = describe(e)) }
                }
            }
            minimum.join()
            _state.update { it.copy(refreshing = false) }
        }
    }

    fun setMe(memberId: String?) = inGroup { core.setMe(it.group.id, memberId) }

    fun setArchived(archived: Boolean) = inGroup { core.setGroupArchived(it.group.id, archived) }

    fun deleteGroup() = launchCore {
        val d = _state.value.detail ?: return@launchCore
        core.deleteGroup(d.group.id)
        _state.update { it.copy(detail = null) }
        loadGroups()
    }

    /**
     * What each member's equal share of [totalMinor] comes to, computed by the core so
     * the editor shows the same cents that will be saved. Empty when it can't split.
     */
    fun previewEqualSplit(totalMinor: Long, among: List<String>): List<Long> = try {
        previewSplit(totalMinor, among.map { ShareDto(it, 1u, null) })
    } catch (_: CoreException) {
        emptyList()
    }

    fun openGroup(id: String) = launchCore {
        loadDetail(id)
        syncQuietly()
    }

    /**
     * Shares the open group (first time) and prepares its invite for the share sheet.
     * Only the person who shared it can invite (ADR 0006).
     */
    fun shareGroup() = launchCore {
        val t = transportOrExplain() ?: return@launchCore
        val d = _state.value.detail ?: return@launchCore
        if (!core.isShared(d.group.id)) core.shareGroup(t, d.group.id)
        val code = core.inviteCode(t, d.group.id)
        loadDetail(d.group.id)
        loadGroups()
        _state.update { it.copy(invite = PendingInvite(d.group.name, code)) }
    }

    fun inviteShown() = _state.update { it.copy(invite = null) }

    /** Turns the invite link off; inviting again makes a new one. */
    fun turnOffInvite() = launchCore {
        val t = transportOrExplain() ?: return@launchCore
        val d = _state.value.detail ?: return@launchCore
        core.turnOffInvite(t, d.group.id)
        _state.update { it.copy(error = getApplication<Application>().getString(R.string.invite_turned_off)) }
    }

    /** Joins from an invite link or a bare code, then opens the group. */
    fun join(linkOrCode: String) = launchCore {
        val t = transportOrExplain() ?: return@launchCore
        val code = linkOrCode.trim().substringAfterLast('/').substringBefore('?')
        val id = core.joinGroup(t, code)
        loadGroups()
        loadDetail(id)
    }

    private fun transportOrExplain(): SyncTransport? = transport ?: run {
        _state.update { it.copy(error = getApplication<Application>().getString(R.string.err_no_sync)) }
        null
    }

    /** Background sync: offline or server trouble stays quiet until the next try. */
    private fun syncQuietly() {
        val t = transport ?: return
        viewModelScope.launch(Dispatchers.IO) {
            try {
                if (core.sync(t).pulled > 0u) {
                    loadGroups()
                    _state.value.detail?.let { loadDetail(it.group.id) }
                }
            } catch (e: CoreException) {
                Log.i("DiBillSion", "background sync failed", e)
            }
        }
    }

    fun closeGroup() {
        scanJob?.cancel()
        _state.update { it.copy(detail = null, scan = null) }
        refreshGroups()
    }

    fun addMember(name: String) = inGroup { core.addMember(it.group.id, name.trim()) }

    fun setMemberActive(id: String, active: Boolean) = inGroup { core.setMemberActive(id, active) }

    fun deleteMember(id: String) = inGroup { core.deleteMember(id) }

    fun deleteTransaction(id: String) = inGroup { core.deleteTransaction(id) }

    /** Adds [memberId] to, or removes them from, one item of a saved expense (ADR 0009). */
    fun setItemMember(transactionId: String, itemIndex: Int, memberId: String, included: Boolean) =
        inGroup { core.setItemMember(transactionId, itemIndex.toUInt(), memberId, included) }

    /** Each share of [item] in its currency, as the core splits it; empty when it can't. */
    fun itemShares(item: ItemDto): List<Long> = try {
        previewSplit(item.totalMinor, item.shares)
    } catch (_: CoreException) {
        emptyList()
    }

    /** An equal split of [amountMinor] between [among], paid by [payerId]. */
    fun addExpense(
        title: String,
        amountMinor: Long,
        currency: String,
        fxRate: String?,
        payerId: String,
        among: List<String>,
        dateDays: Int,
    ) = inGroup { d ->
        core.saveTransaction(
            TransactionDto(
                id = null,
                groupId = d.group.id,
                kind = TransactionKindDto.EXPENSE,
                title = title.trim(),
                payerId = payerId,
                currency = currency,
                fxRate = fxRate,
                dateDays = dateDays,
                items = listOf(ItemDto(null, amountMinor, among.map { ShareDto(it, 1u, null) })),
                baseTotalMinor = null,
            ),
        )
    }

    /**
     * Reads the receipt photo at [uri] (camera or gallery) through the server (ADR 0008),
     * then opens the review screen with everyone on every item.
     */
    fun scanReceipt(uri: Uri) {
        val t = transportOrExplain() ?: return
        val d = _state.value.detail ?: return
        val app = getApplication<Application>()
        _state.update { it.copy(scan = ScanState.Reading) }
        scanJob?.cancel()
        scanJob = viewModelScope.launch {
            val outcome: Any = withContext(Dispatchers.IO) {
                try {
                    core.scanReceipt(t, d.group.id, ReceiptImage.load(app, uri), "image/jpeg")
                } catch (e: CoreException) {
                    describe(e)
                } catch (e: IOException) {
                    Log.w("DiBillSion", "can't read image", e)
                    app.getString(R.string.err_image)
                } catch (e: SecurityException) {
                    Log.w("DiBillSion", "can't read image", e)
                    app.getString(R.string.err_image)
                }
            }
            when (outcome) {
                is com.dibillsion.core.ScannedReceiptDto -> {
                    val active = d.members.filter { it.active }
                    val draft = ReceiptDraft.from(
                        r = outcome,
                        exponent = exponentOf(outcome.currency) ?: d.exponent,
                        everyone = active.map { it.id }.toSet(),
                        payerId = d.meId?.takeIf { me -> active.any { it.id == me } } ?: active.firstOrNull()?.id,
                        today = today(),
                        untitled = app.getString(R.string.receipt),
                    )
                    _state.update { it.copy(scan = ScanState.Review(draft)) }
                }
                else -> _state.update { it.copy(scan = null, error = outcome as String) }
            }
        }
    }

    fun editReceipt(change: (ReceiptDraft) -> ReceiptDraft) = _state.update { s ->
        val review = s.scan as? ScanState.Review ?: return@update s
        s.copy(scan = ScanState.Review(change(review.draft)))
    }

    /** Leaves the scan: a reading in flight is dropped when it comes back. */
    fun closeScan() {
        scanJob?.cancel()
        _state.update { it.copy(scan = null) }
    }

    /** Saves the reviewed receipt as one expense with an item per line. */
    fun saveReceipt() {
        val d = _state.value.detail ?: return
        val draft = (_state.value.scan as? ScanState.Review)?.draft ?: return
        val exponent = exponentOf(draft.currency) ?: return
        val payer = draft.payerId ?: return
        val foreign = draft.currency != d.group.baseCurrency
        val tx = TransactionDto(
            id = null,
            groupId = d.group.id,
            kind = TransactionKindDto.EXPENSE,
            title = draft.title.trim(),
            payerId = payer,
            currency = draft.currency,
            fxRate = if (foreign) Money.parseRate(draft.rate) else null,
            dateDays = draft.dateDays,
            items = draft.items(exponent, d.members.map { it.id }),
            baseTotalMinor = null,
        )
        // The draft stays open if saving fails, so nothing typed is lost.
        inGroup {
            core.saveTransaction(tx)
            _state.update { it.copy(scan = null) }
        }
    }

    /** What each member owes for the draft, summed over its items exactly as the core splits them. */
    fun receiptShares(draft: ReceiptDraft, memberOrder: List<String>): Map<String, Long> {
        val exponent = exponentOf(draft.currency) ?: return emptyMap()
        val totals = mutableMapOf<String, Long>()
        for (item in draft.items(exponent, memberOrder)) {
            val parts = try {
                previewSplit(item.totalMinor, item.shares)
            } catch (_: CoreException) {
                return emptyMap()
            }
            item.shares.zip(parts).forEach { (share, part) -> totals.merge(share.memberId, part, Long::plus) }
        }
        return totals
    }

    /** Records a suggested reimbursement as paid (a transfer in the base currency). */
    fun markPaid(fromId: String, toId: String, amountMinor: Long) = inGroup { d ->
        core.saveTransaction(
            TransactionDto(
                id = null,
                groupId = d.group.id,
                kind = TransactionKindDto.TRANSFER,
                title = "",
                payerId = fromId,
                currency = d.group.baseCurrency,
                fxRate = null,
                dateDays = today(),
                items = listOf(ItemDto(null, amountMinor, listOf(ShareDto(toId, null, amountMinor)))),
                baseTotalMinor = null,
            ),
        )
    }

    private fun today(): Int = LocalDate.now().toEpochDay().toInt()

    private fun loadDetail(groupId: String) {
        val group = core.groups().first { it.id == groupId }
        val detail = GroupDetail(
            group = group,
            members = core.members(groupId),
            transactions = core.transactions(groupId),
            balances = core.balances(groupId),
            exponent = currencyExponent(group.baseCurrency).toInt(),
            shared = core.isShared(groupId),
        )
        _state.update { it.copy(detail = detail) }
    }

    /** Runs [block] against the open group, then reloads it. */
    private fun inGroup(block: (GroupDetail) -> Unit) = launchCore {
        val d = _state.value.detail ?: return@launchCore
        block(d)
        loadDetail(d.group.id)
        loadGroups() // the cards show my balance
        syncQuietly()
    }

    private fun launchCore(block: () -> Unit) {
        viewModelScope.launch {
            val error = withContext(Dispatchers.IO) {
                try {
                    block()
                    null
                } catch (e: CoreException) {
                    describe(e)
                }
            }
            if (error != null) _state.update { it.copy(error = error) }
        }
    }

    /** A message for the user in their language; English detail goes to the log only. */
    private fun describe(e: CoreException): String {
        val app = getApplication<Application>()
        val res = when (e) {
            is CoreException.NotFound -> R.string.err_not_found
            is CoreException.UnknownCurrency -> return app.getString(R.string.err_unknown_currency, e.code)
            is CoreException.MissingRate -> R.string.error_rate
            is CoreException.InvalidRate -> R.string.err_invalid_rate
            is CoreException.UnknownMember -> R.string.err_unknown_member
            is CoreException.MemberInactive -> R.string.err_member_inactive
            is CoreException.MemberHasHistory -> R.string.err_member_has_history
            is CoreException.InvalidSplit -> R.string.err_invalid_split
            is CoreException.InvalidInput -> R.string.err_invalid_input
            is CoreException.Storage -> R.string.err_storage
            is CoreException.Offline -> R.string.err_offline
            is CoreException.Server -> when (e.code) {
                "not_found" -> R.string.err_invite_invalid
                "forbidden" -> R.string.err_forbidden
                "receipts_disabled" -> R.string.err_receipts_disabled
                "limit_reached" -> R.string.err_limit_reached
                "unreadable" -> R.string.err_unreadable
                "provider_failed" -> R.string.err_provider_failed
                "too_large" -> R.string.err_image
                else -> R.string.err_server
            }
        }
        Log.w("DiBillSion", "core error", e)
        return app.getString(res)
    }

    override fun onCleared() {
        if (coreLazy.isInitialized()) core.close()
    }
}
