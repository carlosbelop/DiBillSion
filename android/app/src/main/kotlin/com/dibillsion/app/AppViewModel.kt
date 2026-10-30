package com.dibillsion.app

import android.app.Application
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
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import java.io.File
import java.time.LocalDate

/** Everything one group's screen shows. */
data class GroupDetail(
    val group: GroupDto,
    val members: List<MemberDto>,
    val transactions: List<TransactionDto>,
    val balances: BalancesDto,
    val exponent: Int,
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
)

/**
 * Talks to the Rust core. Every core call is short and blocking, so it runs on
 * [Dispatchers.IO]; the UI only observes [state].
 */
class AppViewModel(app: Application) : AndroidViewModel(app) {
    private val coreLazy = lazy { Core.open(File(app.filesDir, "dibill.db").absolutePath) }
    private val core: Core by coreLazy

    private val _state = MutableStateFlow(UiState())
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
        _state.update { it.copy(groups = groups, memberCounts = counts) }
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

    /** Pull-to-refresh: reloads from the core (and, once sync exists, from the server). */
    fun refresh() {
        _state.update { it.copy(refreshing = true) }
        viewModelScope.launch {
            // Keep the indicator visible long enough to read as feedback.
            val minimum = launch { delay(450) }
            withContext(Dispatchers.IO) {
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

    fun openGroup(id: String) = launchCore { loadDetail(id) }

    fun closeGroup() {
        _state.update { it.copy(detail = null) }
        refreshGroups()
    }

    fun addMember(name: String) = inGroup { core.addMember(it.group.id, name.trim()) }

    fun setMemberActive(id: String, active: Boolean) = inGroup { core.setMemberActive(id, active) }

    fun deleteMember(id: String) = inGroup { core.deleteMember(id) }

    fun deleteTransaction(id: String) = inGroup { core.deleteTransaction(id) }

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
        )
        _state.update { it.copy(detail = detail) }
    }

    /** Runs [block] against the open group, then reloads it. */
    private fun inGroup(block: (GroupDetail) -> Unit) = launchCore {
        val d = _state.value.detail ?: return@launchCore
        block(d)
        loadDetail(d.group.id)
        loadGroups() // the cards show my balance
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
