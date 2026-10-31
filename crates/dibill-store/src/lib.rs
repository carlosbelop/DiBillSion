//! On-device persistence: one SQLite database per install, same schema on
//! Android and iOS (bundled SQLite). Money is stored as `INTEGER` minor units
//! and exchange rates as decimal `TEXT`, never as floating point.
//!
//! Every write is validated with `dibill-domain` first, so the database only
//! ever holds transactions that split and convert cleanly.
//!
//! Every write is also recorded in the sync op log, and the rows are a
//! projection of it (ADR 0005): see [`Store::apply_remote`].

mod schema;
mod sync;

use std::collections::BTreeMap;
use std::path::Path;
use std::str::FromStr;
use std::time::{SystemTime, UNIX_EPOCH};

use dibill_domain::{
    Currency, CurrencyError, Expense, ExpenseItem, ExpenseKind, FxError, FxRate, ItemShare,
    MemberError, MemberId, MemberStatus, Minor, Portion, SplitError, Transfer, balances,
    check_expense_members, check_transfer_members, convert_items, settle_up, split_item,
};
use rusqlite::{Connection, OptionalExtension};
use uuid::Uuid;

pub use dibill_sync::{Change, DeviceId, OpId, Stamp};
pub use sync::{Entity, Field, SyncOp};
use sync::{json, set};

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error(transparent)]
    Sqlite(#[from] rusqlite::Error),
    #[error(transparent)]
    Split(#[from] SplitError),
    #[error(transparent)]
    Fx(#[from] FxError),
    #[error(transparent)]
    Member(#[from] MemberError),
    #[error(transparent)]
    Currency(#[from] CurrencyError),
    #[error("not found")]
    NotFound,
    #[error("a transaction in a foreign currency needs an exchange rate")]
    MissingRate,
    #[error("a transaction in the base currency must not have an exchange rate")]
    UnexpectedRate,
    #[error("a transfer has exactly one item with one receiver taking the whole amount")]
    MalformedTransfer,
    #[error("member appears in transactions; deactivate them instead")]
    MemberHasHistory,
    #[error("stored data is invalid: {0}")]
    Corrupt(String),
}

pub type Result<T> = std::result::Result<T, StoreError>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GroupId(pub Uuid);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TransactionId(pub Uuid);

impl TransactionId {
    #[must_use]
    pub fn new() -> Self {
        Self(Uuid::now_v7())
    }
}

impl Default for TransactionId {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GroupKind {
    Trip,
    Home,
    Couple,
    Event,
    Project,
    Other,
}

impl GroupKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Trip => "trip",
            Self::Home => "home",
            Self::Couple => "couple",
            Self::Event => "event",
            Self::Project => "project",
            Self::Other => "other",
        }
    }

    fn parse(s: &str) -> Result<Self> {
        Ok(match s {
            "trip" => Self::Trip,
            "home" => Self::Home,
            "couple" => Self::Couple,
            "event" => Self::Event,
            "project" => Self::Project,
            "other" => Self::Other,
            _ => return Err(StoreError::Corrupt(format!("group kind {s:?}"))),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Group {
    pub id: GroupId,
    pub name: String,
    pub emoji: Option<String>,
    pub kind: GroupKind,
    pub base_currency: Currency,
    pub archived: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Member {
    pub id: MemberId,
    pub display_name: String,
    pub status: MemberStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransactionKind {
    Expense,
    Income,
    /// `payer` sent money to the single member of the single item.
    Transfer,
}

impl TransactionKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Expense => "expense",
            Self::Income => "income",
            Self::Transfer => "transfer",
        }
    }

    fn parse(s: &str) -> Result<Self> {
        Ok(match s {
            "expense" => Self::Expense,
            "income" => Self::Income,
            "transfer" => Self::Transfer,
            _ => return Err(StoreError::Corrupt(format!("transaction kind {s:?}"))),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Item {
    pub label: Option<String>,
    /// In the transaction's own currency.
    pub total: Minor,
    pub shares: Vec<ItemShare>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Transaction {
    pub id: TransactionId,
    pub group: GroupId,
    pub kind: TransactionKind,
    pub title: String,
    pub payer: MemberId,
    pub currency: Currency,
    /// Required when `currency` differs from the group's base currency.
    pub fx_rate: Option<FxRate>,
    /// Days since 1970-01-01 (the expense date, not the time it was entered).
    pub date_days: i32,
    pub items: Vec<Item>,
}

impl Transaction {
    /// A reimbursement of `amount` from `from` to `to`.
    #[must_use]
    pub fn transfer(
        group: GroupId,
        from: MemberId,
        to: MemberId,
        amount: Minor,
        currency: Currency,
        date_days: i32,
    ) -> Self {
        Self {
            id: TransactionId::new(),
            group,
            kind: TransactionKind::Transfer,
            title: String::new(),
            payer: from,
            currency,
            fx_rate: None,
            date_days,
            items: vec![Item {
                label: None,
                total: amount,
                shares: vec![ItemShare::amount(to, amount)],
            }],
        }
    }
}

/// Balances and suggested reimbursements of a group, in its base currency.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GroupBalances {
    pub currency: Currency,
    pub balances: BTreeMap<MemberId, Minor>,
    pub suggestions: Vec<Transfer>,
}

#[derive(Debug)]
pub struct Store {
    conn: Connection,
}

impl Store {
    /// Opens (or creates) the database at `path` and migrates it.
    ///
    /// # Errors
    /// SQLite errors opening or migrating the file.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        Self::init(Connection::open(path)?)
    }

    /// A private in-memory database, for tests and previews.
    ///
    /// # Errors
    /// SQLite errors creating the schema.
    pub fn open_in_memory() -> Result<Self> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(mut conn: Connection) -> Result<Self> {
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        schema::migrate(&mut conn)?;
        let tx = conn.transaction()?;
        if sync::clock(&tx)?.1 {
            sync::backfill(&tx)?;
        }
        tx.commit()?;
        Ok(Self { conn })
    }

    /// Records a local change and rebuilds the entity's rows, atomically.
    fn write(
        &self,
        group: GroupId,
        entity: Entity,
        changes: Vec<Change<Field, String>>,
    ) -> Result<()> {
        let tx = self.conn.unchecked_transaction()?;
        for change in changes {
            sync::record(&tx, group, entity, change)?;
        }
        match entity {
            Entity::Group => sync::materialize_group(&tx, group)?,
            Entity::Member(m) => sync::materialize_member(&tx, group, m)?,
            Entity::Transaction(t) => sync::materialize_transaction(&tx, group, t)?,
        }
        tx.commit()?;
        Ok(())
    }

    /// # Errors
    /// SQLite errors.
    pub fn create_group(
        &self,
        name: &str,
        emoji: Option<&str>,
        kind: GroupKind,
        base_currency: Currency,
    ) -> Result<GroupId> {
        let id = GroupId(Uuid::now_v7());
        self.write(
            id,
            Entity::Group,
            vec![
                set(Field::BaseCurrency, json(base_currency.code())),
                set(Field::Name, json(name)),
                set(Field::Emoji, json(&emoji)),
                set(Field::Kind, json(kind.as_str())),
            ],
        )?;
        Ok(id)
    }

    /// # Errors
    /// [`StoreError::NotFound`] for an unknown group.
    pub fn group(&self, id: GroupId) -> Result<Group> {
        self.conn
            .query_row(
                "SELECT name, emoji, kind, base_currency, archived_at IS NOT NULL FROM groups WHERE id = ?1",
                [id.0],
                |r| Ok((r.get::<_, String>(0)?, r.get(1)?, r.get::<_, String>(2)?, r.get::<_, String>(3)?, r.get(4)?)),
            )
            .optional()?
            .ok_or(StoreError::NotFound)
            .and_then(|(name, emoji, kind, cur, archived)| {
                Ok(Group {
                    id,
                    name,
                    emoji,
                    kind: GroupKind::parse(&kind)?,
                    base_currency: Currency::from_code(&cur)?,
                    archived,
                })
            })
    }

    /// Groups ordered by creation (oldest first).
    ///
    /// # Errors
    /// SQLite errors or corrupt rows.
    pub fn groups(&self) -> Result<Vec<Group>> {
        let ids: Vec<Uuid> = self
            .conn
            .prepare_cached("SELECT id FROM groups ORDER BY created_at, id")?
            .query_map([], |r| r.get(0))?
            .collect::<rusqlite::Result<_>>()?;
        ids.into_iter().map(|id| self.group(GroupId(id))).collect()
    }

    /// # Errors
    /// [`StoreError::NotFound`] for an unknown group.
    pub fn set_archived(&self, id: GroupId, archived: bool) -> Result<()> {
        self.group(id)?;
        self.write(
            id,
            Entity::Group,
            vec![set(Field::Archived, json(&archived))],
        )
    }

    /// Deletes a group and everything in it.
    ///
    /// # Errors
    /// [`StoreError::NotFound`] for an unknown group.
    pub fn delete_group(&mut self, id: GroupId) -> Result<()> {
        self.group(id)?;
        self.write(id, Entity::Group, vec![Change::Delete])
    }

    /// # Errors
    /// [`StoreError::NotFound`] for an unknown group.
    pub fn add_member(&self, group: GroupId, display_name: &str) -> Result<MemberId> {
        self.group(group)?;
        let id = MemberId::new();
        self.write(
            group,
            Entity::Member(id),
            vec![
                set(Field::Name, json(display_name)),
                set(Field::Status, sync::status_value(MemberStatus::Active)),
            ],
        )?;
        Ok(id)
    }

    /// Members ordered by creation. A deleted member still used by a live
    /// transaction (a concurrent edit on another device) shows as inactive.
    ///
    /// # Errors
    /// SQLite errors or corrupt rows.
    pub fn members(&self, group: GroupId) -> Result<Vec<Member>> {
        self.conn
            .prepare_cached(
                "SELECT id, display_name, CASE WHEN deleted_at IS NULL THEN status ELSE 'inactive' END
                 FROM members m WHERE group_id = ?1 AND (
                     deleted_at IS NULL
                     OR EXISTS (SELECT 1 FROM transactions t WHERE t.payer_id = m.id AND t.deleted_at IS NULL)
                     OR EXISTS (SELECT 1 FROM item_shares s JOIN transactions t ON t.id = s.transaction_id
                                WHERE s.member_id = m.id AND t.deleted_at IS NULL))
                 ORDER BY created_at, id",
            )?
            .query_map([group.0], |r| Ok((r.get::<_, Uuid>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?)))?
            .map(|row| {
                let (id, display_name, status) = row?;
                Ok(Member {
                    id: MemberId(id),
                    display_name,
                    status: parse_status(&status)?,
                })
            })
            .collect()
    }

    /// # Errors
    /// [`StoreError::NotFound`] for an unknown member.
    pub fn set_member_status(&self, member: MemberId, status: MemberStatus) -> Result<()> {
        let group = self.member_group(member)?;
        self.write(
            group,
            Entity::Member(member),
            vec![set(Field::Status, sync::status_value(status))],
        )
    }

    fn member_group(&self, member: MemberId) -> Result<GroupId> {
        self.conn
            .query_row(
                "SELECT group_id FROM members WHERE id = ?1 AND deleted_at IS NULL",
                [member.0],
                |r| r.get(0),
            )
            .optional()?
            .map(GroupId)
            .ok_or(StoreError::NotFound)
    }

    /// # Errors
    /// [`StoreError::MemberHasHistory`] if any transaction (even deleted, as
    /// sync may still need it) mentions the member; [`StoreError::NotFound`].
    pub fn delete_member(&self, member: MemberId) -> Result<()> {
        let used: bool = self.conn.query_row(
            "SELECT EXISTS (SELECT 1 FROM transactions WHERE payer_id = ?1)
                 OR EXISTS (SELECT 1 FROM item_shares WHERE member_id = ?1)",
            [member.0],
            |r| r.get(0),
        )?;
        if used {
            return Err(StoreError::MemberHasHistory);
        }
        let group = self.member_group(member)?;
        self.write(group, Entity::Member(member), vec![Change::Delete])
    }

    /// Inserts or replaces a transaction after validating it: members known
    /// and active (transfers may involve inactive members), every item splits
    /// cleanly, and a rate is present exactly when the currency is foreign.
    ///
    /// # Errors
    /// The first validation error, or SQLite errors.
    pub fn save_transaction(&mut self, t: &Transaction) -> Result<()> {
        let group = self.group(t.group)?;
        let statuses: BTreeMap<MemberId, MemberStatus> = self
            .members(t.group)?
            .into_iter()
            .map(|m| (m.id, m.status))
            .collect();
        validate(t, group.base_currency, &statuses)?;

        let owner: Option<Uuid> = self
            .conn
            .query_row(
                "SELECT group_id FROM transactions WHERE id = ?1",
                [t.id.0],
                |r| r.get(0),
            )
            .optional()?;
        // Deletes are final (ADR 0004): a deleted transaction can't be saved again.
        if owner.is_some_and(|g| g != t.group.0)
            || sync::is_deleted(&self.conn, t.group, Entity::Transaction(t.id))?
        {
            return Err(StoreError::NotFound);
        }
        self.write(
            t.group,
            Entity::Transaction(t.id),
            vec![set(Field::Body, sync::body(t))],
        )
    }

    /// Sets how many parts `member` takes of the item at `item` of an expense or
    /// income, 0 meaning they leave it (ADR 0009). Synced on its own, so people
    /// changing different items or different members at once all keep their change.
    ///
    /// # Errors
    /// [`StoreError::NotFound`] for an unknown or deleted transaction, item or
    /// member of another group; [`SplitError::NoParticipants`] when the item
    /// would have nobody left; [`MemberError::Inactive`] to add an inactive
    /// member; [`StoreError::MalformedTransfer`] for transfers and
    /// [`SplitError::AmountsDontAddUp`] for items split by exact amounts.
    pub fn set_item_share(
        &self,
        id: TransactionId,
        item: u32,
        member: MemberId,
        parts: u32,
    ) -> Result<()> {
        let group: Uuid = self
            .conn
            .query_row(
                "SELECT group_id FROM transactions WHERE id = ?1 AND deleted_at IS NULL",
                [id.0],
                |r| r.get(0),
            )
            .optional()?
            .ok_or(StoreError::NotFound)?;
        let group = GroupId(group);
        if self.member_group(member)? != group {
            return Err(StoreError::NotFound);
        }
        let t = self
            .transactions(group)?
            .into_iter()
            .find(|t| t.id == id)
            .ok_or(StoreError::NotFound)?;
        if t.kind == TransactionKind::Transfer {
            return Err(StoreError::MalformedTransfer);
        }
        let current = t
            .items
            .get(usize::try_from(item).map_err(|_| StoreError::NotFound)?)
            .ok_or(StoreError::NotFound)?;
        if current
            .shares
            .iter()
            .any(|s| matches!(s.portion, Portion::Amount(_)))
        {
            return Err(SplitError::AmountsDontAddUp.into());
        }
        let others = current
            .shares
            .iter()
            .any(|s| s.member != member && matches!(s.portion, Portion::Parts(w) if w > 0));
        if parts == 0 && !others {
            return Err(SplitError::NoParticipants.into());
        }
        let shared = current.shares.iter().any(|s| s.member == member);
        if parts > 0
            && !shared
            && self
                .members(group)?
                .iter()
                .any(|m| m.id == member && m.status == MemberStatus::Inactive)
        {
            return Err(MemberError::Inactive(member).into());
        }
        self.write(
            group,
            Entity::Transaction(id),
            vec![set(Field::Share { item, member }, json(&parts))],
        )
    }

    /// Marks a transaction deleted (a tombstone kept for sync).
    ///
    /// # Errors
    /// [`StoreError::NotFound`] if it doesn't exist or is already deleted.
    pub fn delete_transaction(&self, id: TransactionId) -> Result<()> {
        let group: Uuid = self
            .conn
            .query_row(
                "SELECT group_id FROM transactions WHERE id = ?1 AND deleted_at IS NULL",
                [id.0],
                |r| r.get(0),
            )
            .optional()?
            .ok_or(StoreError::NotFound)?;
        self.write(
            GroupId(group),
            Entity::Transaction(id),
            vec![Change::Delete],
        )
    }

    /// Records which member is the person using this device in `group`, or
    /// clears it. Device-local: it is not synced (each phone has its own "me").
    ///
    /// # Errors
    /// [`StoreError::NotFound`] if the group doesn't exist or the member isn't in it.
    pub fn set_me(&self, group: GroupId, member: Option<MemberId>) -> Result<()> {
        self.group(group)?;
        if let Some(m) = member
            && self.member_group(m)? != group
        {
            return Err(StoreError::NotFound);
        }
        self.conn.execute(
            "INSERT INTO group_prefs (group_id, me) VALUES (?1, ?2)
             ON CONFLICT (group_id) DO UPDATE SET me = ?2",
            rusqlite::params![group.0, member.map(|m| m.0)],
        )?;
        Ok(())
    }

    /// The member chosen with [`set_me`](Self::set_me), if they are still shown in the group.
    ///
    /// # Errors
    /// SQLite errors.
    pub fn me(&self, group: GroupId) -> Result<Option<MemberId>> {
        let me: Option<Uuid> = self
            .conn
            .query_row(
                "SELECT me FROM group_prefs WHERE group_id = ?1",
                [group.0],
                |r| r.get(0),
            )
            .optional()?
            .flatten();
        let Some(me) = me.map(MemberId) else {
            return Ok(None);
        };
        Ok(self
            .members(group)?
            .iter()
            .any(|m| m.id == me)
            .then_some(me))
    }

    /// The server token of this device, once registered.
    ///
    /// # Errors
    /// SQLite errors.
    pub fn sync_token(&self) -> Result<Option<String>> {
        sync::clock(&self.conn)?;
        Ok(self
            .conn
            .query_row("SELECT token FROM sync_device", [], |r| r.get(0))?)
    }

    /// # Errors
    /// SQLite errors.
    pub fn set_sync_token(&self, token: &str) -> Result<()> {
        sync::clock(&self.conn)?;
        self.conn
            .execute("UPDATE sync_device SET token = ?1", [token])?;
        Ok(())
    }

    /// Records that `group` is shared through the server (idempotent).
    ///
    /// # Errors
    /// SQLite errors.
    pub fn mark_shared(&self, group: GroupId) -> Result<()> {
        self.conn.execute(
            "INSERT OR IGNORE INTO group_sync (group_id) VALUES (?1)",
            [group.0],
        )?;
        Ok(())
    }

    /// # Errors
    /// SQLite errors.
    pub fn is_shared(&self, group: GroupId) -> Result<bool> {
        Ok(self.conn.query_row(
            "SELECT EXISTS (SELECT 1 FROM group_sync WHERE group_id = ?1)",
            [group.0],
            |r| r.get(0),
        )?)
    }

    /// Shared groups with the server cursor each was last pulled up to.
    ///
    /// # Errors
    /// SQLite errors.
    pub fn shared_groups(&self) -> Result<Vec<(GroupId, u64)>> {
        self.conn
            .prepare_cached("SELECT group_id, cursor FROM group_sync ORDER BY group_id")?
            .query_map([], |r| Ok((GroupId(r.get(0)?), r.get::<_, i64>(1)?)))?
            .map(|row| {
                let (g, c) = row?;
                Ok((g, c.unsigned_abs()))
            })
            .collect()
    }

    /// # Errors
    /// [`StoreError::NotFound`] if the group isn't shared.
    pub fn set_cursor(&self, group: GroupId, cursor: u64) -> Result<()> {
        let cursor = i64::try_from(cursor).map_err(|_| StoreError::Corrupt("cursor".into()))?;
        let n = self.conn.execute(
            "UPDATE group_sync SET cursor = ?2 WHERE group_id = ?1",
            rusqlite::params![group.0, cursor],
        )?;
        if n == 0 {
            return Err(StoreError::NotFound);
        }
        Ok(())
    }

    /// Local ops of one group the server hasn't acknowledged, oldest first.
    ///
    /// # Errors
    /// SQLite errors.
    pub fn pending_group_ops(&self, group: GroupId, limit: u32) -> Result<Vec<SyncOp>> {
        sync::pending_in_group(&self.conn, group, limit)
    }

    /// This install's sync identity.
    ///
    /// # Errors
    /// SQLite errors.
    pub fn device_id(&self) -> Result<DeviceId> {
        Ok(sync::clock(&self.conn)?.0.device())
    }

    /// Merges ops from other devices, in any order and with duplicates, and
    /// rebuilds the groups they touch. Returns how many ops were new.
    ///
    /// # Errors
    /// SQLite errors.
    pub fn apply_remote(&mut self, ops: &[SyncOp]) -> Result<usize> {
        let tx = self.conn.transaction()?;
        let n = sync::apply_remote(&tx, ops)?;
        tx.commit()?;
        Ok(n)
    }

    /// Local ops the server hasn't acknowledged yet, oldest first.
    ///
    /// # Errors
    /// SQLite errors.
    pub fn pending_ops(&self, limit: u32) -> Result<Vec<SyncOp>> {
        sync::pending(&self.conn, limit)
    }

    /// # Errors
    /// SQLite errors.
    pub fn mark_pushed(&mut self, ids: &[OpId]) -> Result<()> {
        let tx = self.conn.transaction()?;
        sync::mark_pushed(&tx, ids)?;
        tx.commit()?;
        Ok(())
    }

    /// Every op of a group, oldest first.
    ///
    /// # Errors
    /// SQLite errors.
    pub fn group_ops(&self, group: GroupId) -> Result<Vec<SyncOp>> {
        sync::group_ops(&self.conn, group)
    }
    /// Live transactions of a group, newest date first.
    ///
    /// # Errors
    /// SQLite errors or corrupt rows.
    pub fn transactions(&self, group: GroupId) -> Result<Vec<Transaction>> {
        let rows: Vec<TxRow> = self
            .conn
            .prepare_cached(
                "SELECT id, kind, title, payer_id, currency, fx_rate, date_days FROM transactions
                 WHERE group_id = ?1 AND deleted_at IS NULL ORDER BY date_days DESC, created_at DESC, id",
            )?
            .query_map([group.0], |r| {
                Ok(TxRow {
                    id: r.get(0)?,
                    kind: r.get(1)?,
                    title: r.get(2)?,
                    payer: r.get(3)?,
                    currency: r.get(4)?,
                    fx_rate: r.get(5)?,
                    date_days: r.get(6)?,
                })
            })?
            .collect::<rusqlite::Result<_>>()?;
        rows.into_iter()
            .map(|row| load_row(&self.conn, group, row))
            .collect()
    }

    /// Balances and settle-up suggestions in the group's base currency.
    ///
    /// # Errors
    /// SQLite errors, or domain errors on corrupt data.
    pub fn group_balances(&self, group: GroupId) -> Result<GroupBalances> {
        let base = self.group(group)?.base_currency;
        let mut expenses = Vec::new();
        let mut transfers = Vec::new();
        for t in self.transactions(group)? {
            let totals: Vec<Minor> = t.items.iter().map(|i| i.total).collect();
            let converted = match t.fx_rate {
                Some(rate) => convert_items(&totals, t.currency, base, rate)?,
                None => totals,
            };
            if t.kind == TransactionKind::Transfer {
                let to = t.items[0].shares[0].member;
                transfers.push(Transfer {
                    from: t.payer,
                    to,
                    amount: converted[0],
                });
                continue;
            }
            let kind = if t.kind == TransactionKind::Income {
                ExpenseKind::Income
            } else {
                ExpenseKind::Expense
            };
            let items = t
                .items
                .into_iter()
                .zip(converted)
                .map(|(item, total)| ExpenseItem {
                    total,
                    shares: rescale_amounts(&item, total),
                })
                .collect();
            expenses.push(Expense {
                kind,
                payer: t.payer,
                items,
            });
        }
        let mut balances = balances(&expenses, &transfers)?;
        for m in self.members(group)? {
            balances.entry(m.id).or_default();
        }
        let suggestions = settle_up(&balances);
        Ok(GroupBalances {
            currency: base,
            balances,
            suggestions,
        })
    }
}

/// A transaction's rows, deleted or not.
pub(crate) fn load_transaction(
    conn: &Connection,
    group: GroupId,
    id: TransactionId,
) -> Result<Transaction> {
    let row = conn
        .query_row(
            "SELECT id, kind, title, payer_id, currency, fx_rate, date_days FROM transactions WHERE id = ?1",
            [id.0],
            |r| {
                Ok(TxRow {
                    id: r.get(0)?,
                    kind: r.get(1)?,
                    title: r.get(2)?,
                    payer: r.get(3)?,
                    currency: r.get(4)?,
                    fx_rate: r.get(5)?,
                    date_days: r.get(6)?,
                })
            },
        )
        .optional()?
        .ok_or(StoreError::NotFound)?;
    load_row(conn, group, row)
}

fn load_row(conn: &Connection, group: GroupId, row: TxRow) -> Result<Transaction> {
    let mut items: Vec<Item> = conn
        .prepare_cached(
            "SELECT label, total_minor FROM items WHERE transaction_id = ?1 ORDER BY position",
        )?
        .query_map([row.id], |r| {
            Ok(Item {
                label: r.get(0)?,
                total: r.get(1)?,
                shares: Vec::new(),
            })
        })?
        .collect::<rusqlite::Result<_>>()?;
    let mut stmt = conn.prepare_cached(
            "SELECT position, member_id, parts, amount_minor FROM item_shares WHERE transaction_id = ?1 ORDER BY position, rowid",
        )?;
    let shares = stmt.query_map([row.id], |r| {
        Ok((
            r.get::<_, u32>(0)?,
            r.get::<_, Uuid>(1)?,
            r.get::<_, Option<u32>>(2)?,
            r.get::<_, Option<Minor>>(3)?,
        ))
    })?;
    for share in shares {
        let (pos, member, parts, amount) = share?;
        let portion = match (parts, amount) {
            (Some(w), None) => Portion::Parts(w),
            (None, Some(a)) => Portion::Amount(a),
            _ => return Err(StoreError::Corrupt("share without a single portion".into())),
        };
        items
            .get_mut(pos as usize)
            .ok_or_else(|| StoreError::Corrupt("share for a missing item".into()))?
            .shares
            .push(ItemShare {
                member: MemberId(member),
                portion,
            });
    }
    Ok(Transaction {
        id: TransactionId(row.id),
        group,
        kind: TransactionKind::parse(&row.kind)?,
        title: row.title,
        payer: MemberId(row.payer),
        currency: Currency::from_code(&row.currency)?,
        fx_rate: row.fx_rate.as_deref().map(FxRate::from_str).transpose()?,
        date_days: row.date_days,
        items,
    })
}

struct TxRow {
    id: Uuid,
    kind: String,
    title: String,
    payer: Uuid,
    currency: String,
    fx_rate: Option<String>,
    date_days: i32,
}

fn validate(
    t: &Transaction,
    base_currency: Currency,
    members: &BTreeMap<MemberId, MemberStatus>,
) -> Result<()> {
    match (t.currency == base_currency, t.fx_rate.is_some()) {
        (false, false) => return Err(StoreError::MissingRate),
        (true, true) => return Err(StoreError::UnexpectedRate),
        _ => {}
    }
    if t.kind == TransactionKind::Transfer {
        let [item] = t.items.as_slice() else {
            return Err(StoreError::MalformedTransfer);
        };
        let [share] = item.shares.as_slice() else {
            return Err(StoreError::MalformedTransfer);
        };
        if share.portion != Portion::Amount(item.total) {
            return Err(StoreError::MalformedTransfer);
        }
        let transfer = Transfer {
            from: t.payer,
            to: share.member,
            amount: item.total,
        };
        check_transfer_members(&transfer, members)?;
        // Same rule as balances: positive amount, two different members.
        balances(&[], &[transfer])?;
        return Ok(());
    }
    if t.items.is_empty() {
        return Err(SplitError::NoItems.into());
    }
    for item in &t.items {
        split_item(item.total, &item.shares)?;
    }
    let domain = Expense {
        kind: ExpenseKind::Expense,
        payer: t.payer,
        items: t
            .items
            .iter()
            .map(|i| ExpenseItem {
                total: i.total,
                shares: i.shares.clone(),
            })
            .collect(),
    };
    check_expense_members(&domain, members)?;
    Ok(())
}

/// Exact amounts are entered in the transaction currency; once the item is
/// converted they become weights over the converted total, so the converted
/// split stays proportional and still sums exactly.
fn rescale_amounts(item: &Item, converted_total: Minor) -> Vec<ItemShare> {
    if converted_total == item.total
        || !item
            .shares
            .iter()
            .any(|s| matches!(s.portion, Portion::Amount(_)))
    {
        return item.shares.clone();
    }
    let fixed: Vec<Minor> = item
        .shares
        .iter()
        .map(|s| match s.portion {
            Portion::Amount(a) => a,
            Portion::Parts(_) => 0,
        })
        .collect();
    let rest = item.total - fixed.iter().sum::<Minor>();
    // Allocate the converted total over [fixed amounts..., remainder].
    let mut weights = fixed.clone();
    weights.push(rest);
    let Ok(alloc) = dibill_domain::allocate(converted_total, &weights) else {
        return item.shares.clone();
    };
    item.shares
        .iter()
        .zip(&alloc)
        .map(|(s, a)| match s.portion {
            Portion::Amount(_) => ItemShare::amount(s.member, *a),
            Portion::Parts(_) => *s,
        })
        .collect()
}

fn status_str(status: MemberStatus) -> &'static str {
    match status {
        MemberStatus::Active => "active",
        MemberStatus::Inactive => "inactive",
    }
}

fn parse_status(s: &str) -> Result<MemberStatus> {
    match s {
        "active" => Ok(MemberStatus::Active),
        "inactive" => Ok(MemberStatus::Inactive),
        _ => Err(StoreError::Corrupt(format!("member status {s:?}"))),
    }
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX))
}

#[cfg(test)]
mod tests;
