//! The op log (ADR 0004, ADR 0005). Every write is recorded as an op, and the
//! `groups`, `members`, `transactions`, `items` and `item_shares` rows are a
//! projection of the ops: each entity is rebuilt from its winning field values
//! with the [`dibill_sync`] merge rule, so local and remote ops share one path.

use std::collections::{BTreeMap, BTreeSet};
use std::str::FromStr;

use dibill_domain::{Currency, FxRate, ItemShare, MemberId, MemberStatus, Minor, Portion};
use dibill_proto::{EntityKind, WireOp};
use dibill_sync::{Change, DeviceId, EntityState, LamportClock, Op, OpId, Replica, Stamp};
use rusqlite::{Connection, OptionalExtension, params};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    GroupId, GroupKind, Item, Result, StoreError, Transaction, TransactionId, TransactionKind,
    now_ms, parse_status, status_str, validate,
};

/// What an op changes, inside its group.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Entity {
    Group,
    Member(MemberId),
    Transaction(TransactionId),
}

impl Entity {
    fn kind(self) -> &'static str {
        match self {
            Self::Group => "group",
            Self::Member(_) => "member",
            Self::Transaction(_) => "transaction",
        }
    }

    fn id(self, group: GroupId) -> Uuid {
        match self {
            Self::Group => group.0,
            Self::Member(m) => m.0,
            Self::Transaction(t) => t.0,
        }
    }

    fn parse(kind: &str, id: Uuid) -> Result<Self> {
        Ok(match kind {
            "group" => Self::Group,
            "member" => Self::Member(MemberId(id)),
            "transaction" => Self::Transaction(TransactionId(id)),
            _ => return Err(StoreError::Corrupt(format!("op entity {kind:?}"))),
        })
    }
}

/// A synced field. Values are JSON: strings for names, kinds, currencies and
/// statuses, `null` or a string for the emoji, a boolean for `Archived`, the
/// whole transaction for `Body`, and a number of parts for `Share`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Field {
    Name,
    Emoji,
    Kind,
    BaseCurrency,
    Archived,
    Status,
    Body,
    /// How many parts `member` takes of item `item` (by position) of a
    /// transaction, 0 meaning they left it (ADR 0009). Wins over the body only
    /// when newer than it, so editing the whole transaction resets them.
    Share {
        item: u32,
        member: MemberId,
    },
}

impl Field {
    fn name(self) -> String {
        match self {
            Self::Name => "name".into(),
            Self::Emoji => "emoji".into(),
            Self::Kind => "kind".into(),
            Self::BaseCurrency => "base_currency".into(),
            Self::Archived => "archived".into(),
            Self::Status => "status".into(),
            Self::Body => "body".into(),
            Self::Share { item, member } => format!("share:{item}:{}", member.0),
        }
    }

    fn parse(s: &str) -> Result<Self> {
        if let Some((item, member)) = s.strip_prefix("share:").and_then(|r| r.split_once(':')) {
            return match (item.parse(), Uuid::parse_str(member)) {
                (Ok(item), Ok(member)) => Ok(Self::Share {
                    item,
                    member: MemberId(member),
                }),
                _ => Err(StoreError::Corrupt(format!("op field {s:?}"))),
            };
        }
        Ok(match s {
            "name" => Self::Name,
            "emoji" => Self::Emoji,
            "kind" => Self::Kind,
            "base_currency" => Self::BaseCurrency,
            "archived" => Self::Archived,
            "status" => Self::Status,
            "body" => Self::Body,
            _ => return Err(StoreError::Corrupt(format!("op field {s:?}"))),
        })
    }
}

/// One op of the log, as exchanged with other devices. The value is JSON.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncOp {
    pub id: OpId,
    pub group: GroupId,
    pub stamp: Stamp,
    pub entity: Entity,
    pub change: Change<Field, String>,
}

impl SyncOp {
    /// Whether the value has the shape its entity and field expect. Ops that
    /// don't are dropped on arrival, so every stored value parses.
    fn is_well_formed(&self) -> bool {
        if i64::try_from(self.stamp.lamport).is_err() {
            return false;
        }
        let Change::Set { field, value } = &self.change else {
            return true;
        };
        match (self.entity, field) {
            (Entity::Group | Entity::Member(_), Field::Name) => parse::<String>(value).is_some(),
            (Entity::Group, Field::Emoji) => parse::<Option<String>>(value).is_some(),
            (Entity::Group, Field::Kind) => {
                parse::<String>(value).is_some_and(|k| GroupKind::parse(&k).is_ok())
            }
            (Entity::Group, Field::BaseCurrency) => {
                parse::<String>(value).is_some_and(|c| Currency::from_code(&c).is_ok())
            }
            (Entity::Group, Field::Archived) => parse::<bool>(value).is_some(),
            (Entity::Member(_), Field::Status) => {
                parse::<String>(value).is_some_and(|s| parse_status(&s).is_ok())
            }
            (Entity::Transaction(id), Field::Body) => {
                parse::<Body>(value).is_some_and(|b| b.into_transaction(id, self.group).is_ok())
            }
            (Entity::Transaction(_), Field::Share { .. }) => parse::<u32>(value).is_some(),
            _ => false,
        }
    }
}

impl From<&SyncOp> for WireOp {
    fn from(op: &SyncOp) -> Self {
        let (field, value) = match &op.change {
            Change::Set { field, value } => (Some(field.name()), Some(value.clone())),
            Change::Delete => (None, None),
        };
        Self {
            id: op.id.0,
            group: op.group.0,
            lamport: op.stamp.lamport,
            device: op.stamp.device.0,
            entity: match op.entity {
                Entity::Group => EntityKind::Group,
                Entity::Member(_) => EntityKind::Member,
                Entity::Transaction(_) => EntityKind::Transaction,
            },
            entity_id: op.entity.id(op.group),
            field,
            value,
        }
    }
}

impl TryFrom<WireOp> for SyncOp {
    type Error = StoreError;

    /// Fails for envelopes this version doesn't understand (a field added by a
    /// newer app, say); callers drop such ops like malformed ones.
    fn try_from(op: WireOp) -> Result<Self> {
        let change = match (op.field, op.value) {
            (Some(field), Some(value)) => Change::Set {
                field: Field::parse(&field)?,
                value,
            },
            (None, None) => Change::Delete,
            _ => return Err(StoreError::Corrupt("op field without value".into())),
        };
        let entity = match op.entity {
            EntityKind::Group if op.entity_id == op.group => Entity::Group,
            EntityKind::Group => return Err(StoreError::Corrupt("group op for another id".into())),
            EntityKind::Member => Entity::Member(MemberId(op.entity_id)),
            EntityKind::Transaction => Entity::Transaction(TransactionId(op.entity_id)),
        };
        Ok(Self {
            id: OpId(op.id),
            group: GroupId(op.group),
            stamp: Stamp {
                lamport: op.lamport,
                device: DeviceId(op.device),
            },
            entity,
            change,
        })
    }
}

fn parse<T: DeserializeOwned>(value: &str) -> Option<T> {
    serde_json::from_str(value).ok()
}

pub(crate) fn json<T: Serialize + ?Sized>(value: &T) -> String {
    // Plain data with string keys: serializing cannot fail.
    serde_json::to_string(value).unwrap_or_default()
}

/// A transaction as stored in a `Body` value.
#[derive(Debug, Serialize, Deserialize)]
struct Body {
    kind: String,
    title: String,
    payer: Uuid,
    currency: String,
    fx_rate: Option<String>,
    date_days: i32,
    items: Vec<BodyItem>,
}

#[derive(Debug, Serialize, Deserialize)]
struct BodyItem {
    label: Option<String>,
    total: Minor,
    shares: Vec<BodyShare>,
}

#[derive(Debug, Serialize, Deserialize)]
struct BodyShare {
    member: Uuid,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    parts: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    amount: Option<Minor>,
}

impl Body {
    fn from_transaction(t: &Transaction) -> Self {
        Self {
            kind: t.kind.as_str().to_owned(),
            title: t.title.clone(),
            payer: t.payer.0,
            currency: t.currency.code().to_owned(),
            fx_rate: t.fx_rate.map(|r| r.value().to_string()),
            date_days: t.date_days,
            items: t
                .items
                .iter()
                .map(|i| BodyItem {
                    label: i.label.clone(),
                    total: i.total,
                    shares: i
                        .shares
                        .iter()
                        .map(|s| {
                            let (parts, amount) = match s.portion {
                                Portion::Parts(w) => (Some(w), None),
                                Portion::Amount(a) => (None, Some(a)),
                            };
                            BodyShare {
                                member: s.member.0,
                                parts,
                                amount,
                            }
                        })
                        .collect(),
                })
                .collect(),
        }
    }

    fn into_transaction(self, id: TransactionId, group: GroupId) -> Result<Transaction> {
        let items = self
            .items
            .into_iter()
            .map(|i| {
                let shares = i
                    .shares
                    .into_iter()
                    .map(|s| {
                        let portion = match (s.parts, s.amount) {
                            (Some(w), None) => Portion::Parts(w),
                            (None, Some(a)) => Portion::Amount(a),
                            _ => return Err(StoreError::Corrupt("share portion".into())),
                        };
                        Ok(ItemShare {
                            member: MemberId(s.member),
                            portion,
                        })
                    })
                    .collect::<Result<_>>()?;
                Ok(Item {
                    label: i.label,
                    total: i.total,
                    shares,
                })
            })
            .collect::<Result<_>>()?;
        Ok(Transaction {
            id,
            group,
            kind: TransactionKind::parse(&self.kind)?,
            title: self.title,
            payer: MemberId(self.payer),
            currency: Currency::from_code(&self.currency)?,
            fx_rate: self.fx_rate.as_deref().map(FxRate::from_str).transpose()?,
            date_days: self.date_days,
            items,
        })
    }
}

pub(crate) fn body(t: &Transaction) -> String {
    json(&Body::from_transaction(t))
}

fn lamport_sql(lamport: u64) -> Result<i64> {
    i64::try_from(lamport).map_err(|_| StoreError::Corrupt("lamport overflow".into()))
}

/// This install's clock, creating the device on first use. Returns whether it was created.
pub(crate) fn clock(conn: &Connection) -> Result<(LamportClock, bool)> {
    let row: Option<(Uuid, i64)> = conn
        .query_row("SELECT id, lamport FROM sync_device", [], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })
        .optional()?;
    if let Some((id, lamport)) = row {
        let lamport = u64::try_from(lamport).map_err(|_| StoreError::Corrupt("lamport".into()))?;
        return Ok((LamportClock::resume(DeviceId(id), lamport), false));
    }
    let id = Uuid::now_v7();
    conn.execute("INSERT INTO sync_device (id, lamport) VALUES (?1, 0)", [id])?;
    Ok((LamportClock::new(DeviceId(id)), true))
}

fn save_clock(conn: &Connection, clock: &LamportClock) -> Result<()> {
    conn.execute(
        "UPDATE sync_device SET lamport = ?1",
        [lamport_sql(clock.counter())?],
    )?;
    Ok(())
}

/// Inserts an op unless it is known. Returns whether it was new.
fn insert(conn: &Connection, op: &SyncOp, pushed: bool) -> Result<bool> {
    let (field, value) = match &op.change {
        Change::Set { field, value } => (Some(field.name()), Some(value.as_str())),
        Change::Delete => (None, None),
    };
    let n = conn
        .prepare_cached(
            "INSERT OR IGNORE INTO ops (id, group_id, lamport, device, entity, entity_id, field, value, pushed)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        )?
        .execute(params![
            op.id.0,
            op.group.0,
            lamport_sql(op.stamp.lamport)?,
            op.stamp.device.0,
            op.entity.kind(),
            op.entity.id(op.group),
            field,
            value,
            pushed
        ])?;
    Ok(n == 1)
}

/// Records a local change. The caller materializes the entity afterwards.
pub(crate) fn record(
    conn: &Connection,
    group: GroupId,
    entity: Entity,
    change: Change<Field, String>,
) -> Result<()> {
    let (mut clock, _) = clock(conn)?;
    let op = SyncOp {
        id: OpId::new(),
        group,
        stamp: clock.tick(),
        entity,
        change,
    };
    insert(conn, &op, false)?;
    save_clock(conn, &clock)
}

pub(crate) fn set(field: Field, value: String) -> Change<Field, String> {
    Change::Set { field, value }
}

fn row_to_op(r: &rusqlite::Row<'_>) -> rusqlite::Result<RawOp> {
    Ok(RawOp {
        id: r.get(0)?,
        group: r.get(1)?,
        lamport: r.get(2)?,
        device: r.get(3)?,
        entity: r.get(4)?,
        entity_id: r.get(5)?,
        field: r.get(6)?,
        value: r.get(7)?,
    })
}

const OP_COLUMNS: &str = "id, group_id, lamport, device, entity, entity_id, field, value";

struct RawOp {
    id: Uuid,
    group: Uuid,
    lamport: i64,
    device: Uuid,
    entity: String,
    entity_id: Uuid,
    field: Option<String>,
    value: Option<String>,
}

impl RawOp {
    fn decode(self) -> Result<SyncOp> {
        let change = match (self.field, self.value) {
            (Some(f), Some(value)) => Change::Set {
                field: Field::parse(&f)?,
                value,
            },
            (None, None) => Change::Delete,
            _ => return Err(StoreError::Corrupt("op without a value".into())),
        };
        Ok(SyncOp {
            id: OpId(self.id),
            group: GroupId(self.group),
            stamp: Stamp {
                lamport: u64::try_from(self.lamport)
                    .map_err(|_| StoreError::Corrupt("lamport".into()))?,
                device: DeviceId(self.device),
            },
            entity: Entity::parse(&self.entity, self.entity_id)?,
            change,
        })
    }
}

fn query_ops(conn: &Connection, sql: &str, args: impl rusqlite::Params) -> Result<Vec<SyncOp>> {
    conn.prepare_cached(sql)?
        .query_map(args, row_to_op)?
        .map(|r| r?.decode())
        .collect()
}

/// The merged state of one entity, or `None` if no op mentions it.
fn fold(
    conn: &Connection,
    group: GroupId,
    entity: Entity,
) -> Result<Option<EntityState<Field, String>>> {
    let ops = query_ops(
        conn,
        &format!(
            "SELECT {OP_COLUMNS} FROM ops WHERE group_id = ?1 AND entity = ?2 AND entity_id = ?3"
        ),
        params![group.0, entity.kind(), entity.id(group)],
    )?;
    let mut replica: Replica<(), Field, String> = Replica::new(DeviceId(Uuid::nil()));
    for op in ops {
        replica.apply(&Op {
            id: op.id,
            stamp: op.stamp,
            entity: (),
            change: op.change,
        });
    }
    Ok(replica.entity(&()).cloned())
}

fn field<T: DeserializeOwned>(state: &EntityState<Field, String>, f: Field) -> Option<T> {
    state.field(&f).and_then(|v| parse(v))
}

pub(crate) fn is_deleted(conn: &Connection, group: GroupId, entity: Entity) -> Result<bool> {
    Ok(fold(conn, group, entity)?.is_some_and(|s| s.is_deleted()))
}

/// Milliseconds since the epoch encoded in a UUID v7 (0 for other versions),
/// so creation order is the same on every device.
pub(crate) fn uuid_ms(id: Uuid) -> i64 {
    id.get_timestamp().map_or(0, |ts| {
        let (secs, nanos) = ts.to_unix();
        i64::try_from(secs)
            .unwrap_or(i64::MAX / 1000)
            .saturating_mul(1000)
            .saturating_add(i64::from(nanos / 1_000_000))
    })
}

pub(crate) fn materialize_group(conn: &Connection, group: GroupId) -> Result<()> {
    let Some(state) = fold(conn, group, Entity::Group)? else {
        return Ok(());
    };
    if state.is_deleted() {
        // Shares and transactions reference members, so remove them first.
        conn.execute("DELETE FROM transactions WHERE group_id = ?1", [group.0])?;
        conn.execute("DELETE FROM groups WHERE id = ?1", [group.0])?;
        return Ok(());
    }
    let (Some(name), Some(kind), Some(base)) = (
        field::<String>(&state, Field::Name),
        field::<String>(&state, Field::Kind),
        field::<String>(&state, Field::BaseCurrency),
    ) else {
        return Ok(()); // Not all of the creation has arrived yet.
    };
    let emoji = field::<Option<String>>(&state, Field::Emoji).flatten();
    let archived = field::<bool>(&state, Field::Archived).unwrap_or(false);
    conn.execute(
        "INSERT INTO groups (id, name, emoji, kind, base_currency, created_at, archived_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, CASE WHEN ?7 THEN ?8 END)
         ON CONFLICT (id) DO UPDATE SET name = ?2, emoji = ?3, kind = ?4, base_currency = ?5,
             archived_at = CASE WHEN ?7 THEN coalesce(archived_at, ?8) END",
        params![
            group.0,
            name,
            emoji,
            kind,
            base,
            uuid_ms(group.0),
            archived,
            now_ms()
        ],
    )?;
    Ok(())
}

fn group_currency(conn: &Connection, group: GroupId) -> Result<Option<Currency>> {
    conn.query_row(
        "SELECT base_currency FROM groups WHERE id = ?1",
        [group.0],
        |r| r.get::<_, String>(0),
    )
    .optional()?
    .map(|c| Currency::from_code(&c).map_err(Into::into))
    .transpose()
}

pub(crate) fn materialize_member(
    conn: &Connection,
    group: GroupId,
    member: MemberId,
) -> Result<()> {
    if group_currency(conn, group)?.is_none() {
        return Ok(());
    }
    let Some(state) = fold(conn, group, Entity::Member(member))? else {
        return Ok(());
    };
    let (Some(name), Some(status)) = (
        field::<String>(&state, Field::Name),
        field::<String>(&state, Field::Status),
    ) else {
        return Ok(());
    };
    // A member id claimed by another group is ignored.
    conn.execute(
        "INSERT INTO members (id, group_id, display_name, status, created_at, deleted_at)
         VALUES (?1, ?2, ?3, ?4, ?5, CASE WHEN ?6 THEN ?7 END)
         ON CONFLICT (id) DO UPDATE SET display_name = ?3, status = ?4,
             deleted_at = CASE WHEN ?6 THEN coalesce(deleted_at, ?7) END
         WHERE group_id = ?2",
        params![
            member.0,
            group.0,
            name,
            status,
            uuid_ms(member.0),
            state.is_deleted(),
            now_ms()
        ],
    )?;
    Ok(())
}

/// Rebuilds a transaction's rows. The winning body must be valid against the
/// group as it is now (members known, splits clean); otherwise the rows are
/// removed until ops that make it valid arrive. Member status is not checked:
/// a concurrent deactivation must not hide an expense.
pub(crate) fn materialize_transaction(
    conn: &Connection,
    group: GroupId,
    id: TransactionId,
) -> Result<()> {
    let owner: Option<Uuid> = conn
        .query_row(
            "SELECT group_id FROM transactions WHERE id = ?1",
            [id.0],
            |r| r.get(0),
        )
        .optional()?;
    if owner.is_some_and(|g| g != group.0) {
        return Ok(()); // The id belongs to another group.
    }
    let state = fold(conn, group, Entity::Transaction(id))?;
    let body = state.as_ref().and_then(|s| field::<Body>(s, Field::Body));
    let valid = match (group_currency(conn, group)?, body) {
        (Some(base), Some(body)) => {
            let members: BTreeMap<MemberId, MemberStatus> = conn
                .prepare_cached("SELECT id FROM members WHERE group_id = ?1")?
                .query_map([group.0], |r| r.get::<_, Uuid>(0))?
                .map(|id| Ok((MemberId(id?), MemberStatus::Active)))
                .collect::<Result<_>>()?;
            body.into_transaction(id, group)
                .ok()
                .map(|mut t| {
                    if let Some(state) = &state {
                        apply_shares(&mut t, state, &members);
                    }
                    t
                })
                .filter(|t| validate(t, base, &members).is_ok())
        }
        _ => None,
    };
    let Some(t) = valid else {
        conn.execute("DELETE FROM transactions WHERE id = ?1", [id.0])?;
        return Ok(());
    };
    let deleted = state.is_some_and(|s| s.is_deleted());
    write_rows(conn, &t, deleted)
}

/// Applies the `Share` fields newer than the body (ADR 0009), in field order:
/// unknown members, unknown items and items with exact amounts are skipped, and
/// a change that would leave an item with nobody is ignored.
fn apply_shares(
    t: &mut Transaction,
    state: &EntityState<Field, String>,
    members: &BTreeMap<MemberId, MemberStatus>,
) {
    let Some(body) = state.field_stamp(&Field::Body) else {
        return;
    };
    for (field, stamp, value) in state.fields() {
        let Field::Share { item, member } = *field else {
            continue;
        };
        let (Some(parts), Ok(index)) = (parse::<u32>(value), usize::try_from(item)) else {
            continue;
        };
        if stamp <= body || !members.contains_key(&member) {
            continue;
        }
        let Some(item) = t.items.get_mut(index) else {
            continue;
        };
        if item
            .shares
            .iter()
            .any(|s| matches!(s.portion, Portion::Amount(_)))
        {
            continue;
        }
        let mut shares = item.shares.clone();
        match (shares.iter().position(|s| s.member == member), parts) {
            (Some(i), 0) => {
                shares.remove(i);
            }
            (Some(i), w) => shares[i].portion = Portion::Parts(w),
            (None, 0) => {}
            (None, w) => shares.push(ItemShare::parts(member, w)),
        }
        if shares
            .iter()
            .any(|s| matches!(s.portion, Portion::Parts(w) if w > 0))
        {
            item.shares = shares;
        }
    }
}

fn write_rows(conn: &Connection, t: &Transaction, deleted: bool) -> Result<()> {
    let now = now_ms();
    conn.prepare_cached(
        "INSERT INTO transactions (id, group_id, kind, title, payer_id, currency, fx_rate, date_days, created_at, updated_at, deleted_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, CASE WHEN ?11 THEN ?10 END)
         ON CONFLICT (id) DO UPDATE SET kind = ?3, title = ?4, payer_id = ?5, currency = ?6, fx_rate = ?7,
             date_days = ?8, updated_at = ?10, deleted_at = CASE WHEN ?11 THEN coalesce(deleted_at, ?10) END",
    )?
    .execute(params![
        t.id.0,
        t.group.0,
        t.kind.as_str(),
        t.title,
        t.payer.0,
        t.currency.code(),
        t.fx_rate.map(|r| r.value().to_string()),
        t.date_days,
        uuid_ms(t.id.0),
        now,
        deleted
    ])?;
    conn.execute("DELETE FROM items WHERE transaction_id = ?1", [t.id.0])?;
    let mut item_stmt = conn.prepare_cached(
        "INSERT INTO items (transaction_id, position, label, total_minor) VALUES (?1, ?2, ?3, ?4)",
    )?;
    let mut share_stmt = conn.prepare_cached(
        "INSERT INTO item_shares (transaction_id, position, member_id, parts, amount_minor) VALUES (?1, ?2, ?3, ?4, ?5)",
    )?;
    for (pos, item) in (0_u32..).zip(&t.items) {
        item_stmt.execute(params![t.id.0, pos, item.label, item.total])?;
        for s in &item.shares {
            let (parts, amount) = match s.portion {
                Portion::Parts(w) => (Some(w), None),
                Portion::Amount(a) => (None, Some(a)),
            };
            share_stmt.execute(params![t.id.0, pos, s.member.0, parts, amount])?;
        }
    }
    Ok(())
}

fn entity_ids(conn: &Connection, group: GroupId, kind: &str) -> Result<Vec<Uuid>> {
    Ok(conn
        .prepare_cached("SELECT DISTINCT entity_id FROM ops WHERE group_id = ?1 AND entity = ?2")?
        .query_map(params![group.0, kind], |r| r.get(0))?
        .collect::<rusqlite::Result<_>>()?)
}

/// Rebuilds a whole group: the group, then its members, then its transactions,
/// so ops that arrived before what they reference materialize now.
fn materialize_all(conn: &Connection, group: GroupId) -> Result<()> {
    materialize_group(conn, group)?;
    for id in entity_ids(conn, group, "member")? {
        materialize_member(conn, group, MemberId(id))?;
    }
    for id in entity_ids(conn, group, "transaction")? {
        materialize_transaction(conn, group, TransactionId(id))?;
    }
    Ok(())
}

/// Merges ops from other devices. Known ops are skipped (and count as
/// pushed: the server has them); malformed ones are dropped.
pub(crate) fn apply_remote(conn: &Connection, ops: &[SyncOp]) -> Result<usize> {
    let (mut clock, _) = clock(conn)?;
    let mut groups = BTreeSet::new();
    let mut new = 0;
    for op in ops.iter().filter(|op| op.is_well_formed()) {
        if insert(conn, op, true)? {
            new += 1;
            clock.observe(op.stamp);
            groups.insert(op.group);
        } else {
            conn.execute("UPDATE ops SET pushed = 1 WHERE id = ?1", [op.id.0])?;
        }
    }
    save_clock(conn, &clock)?;
    for group in groups {
        materialize_all(conn, group)?;
    }
    Ok(new)
}

/// Local ops not yet acknowledged by the server, oldest first.
pub(crate) fn pending(conn: &Connection, limit: u32) -> Result<Vec<SyncOp>> {
    query_ops(
        conn,
        &format!("SELECT {OP_COLUMNS} FROM ops WHERE pushed = 0 ORDER BY lamport, id LIMIT ?1"),
        [limit],
    )
}

pub(crate) fn pending_in_group(
    conn: &Connection,
    group: GroupId,
    limit: u32,
) -> Result<Vec<SyncOp>> {
    query_ops(
        conn,
        &format!(
            "SELECT {OP_COLUMNS} FROM ops WHERE pushed = 0 AND group_id = ?1 ORDER BY lamport, id LIMIT ?2"
        ),
        params![group.0, limit],
    )
}

pub(crate) fn mark_pushed(conn: &Connection, ids: &[OpId]) -> Result<()> {
    let mut stmt = conn.prepare_cached("UPDATE ops SET pushed = 1 WHERE id = ?1")?;
    for id in ids {
        stmt.execute([id.0])?;
    }
    Ok(())
}

/// Every op of a group, oldest first (e.g. to upload a group when it is shared).
pub(crate) fn group_ops(conn: &Connection, group: GroupId) -> Result<Vec<SyncOp>> {
    query_ops(
        conn,
        &format!("SELECT {OP_COLUMNS} FROM ops WHERE group_id = ?1 ORDER BY lamport, id"),
        [group.0],
    )
}

/// Records ops for data written before the op log existed (schema v1), so it
/// can be shared like everything else. Runs once, when the device is created.
pub(crate) fn backfill(conn: &Connection) -> Result<()> {
    type GroupRow = (Uuid, String, Option<String>, String, String, bool);
    let groups: Vec<GroupRow> = conn
        .prepare(
            "SELECT id, name, emoji, kind, base_currency, archived_at IS NOT NULL FROM groups",
        )?
        .query_map([], |r| {
            Ok((
                r.get(0)?,
                r.get(1)?,
                r.get(2)?,
                r.get(3)?,
                r.get(4)?,
                r.get(5)?,
            ))
        })?
        .collect::<rusqlite::Result<_>>()?;
    for (id, name, emoji, kind, base, archived) in groups {
        let g = GroupId(id);
        record(
            conn,
            g,
            Entity::Group,
            set(Field::BaseCurrency, json(&base)),
        )?;
        record(conn, g, Entity::Group, set(Field::Name, json(&name)))?;
        record(conn, g, Entity::Group, set(Field::Emoji, json(&emoji)))?;
        record(conn, g, Entity::Group, set(Field::Kind, json(&kind)))?;
        if archived {
            record(conn, g, Entity::Group, set(Field::Archived, json(&true)))?;
        }
    }
    let members: Vec<(Uuid, Uuid, String, String)> = conn
        .prepare("SELECT id, group_id, display_name, status FROM members")?
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?
        .collect::<rusqlite::Result<_>>()?;
    for (id, group, name, status) in members {
        let (g, e) = (GroupId(group), Entity::Member(MemberId(id)));
        record(conn, g, e, set(Field::Name, json(&name)))?;
        record(conn, g, e, set(Field::Status, json(&status)))?;
    }
    let transactions: Vec<(Uuid, Uuid, bool)> = conn
        .prepare("SELECT id, group_id, deleted_at IS NOT NULL FROM transactions")?
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
        .collect::<rusqlite::Result<_>>()?;
    for (id, group, deleted) in transactions {
        let (g, tid) = (GroupId(group), TransactionId(id));
        let t = crate::load_transaction(conn, g, tid)?;
        record(
            conn,
            g,
            Entity::Transaction(tid),
            set(Field::Body, body(&t)),
        )?;
        if deleted {
            record(conn, g, Entity::Transaction(tid), Change::Delete)?;
        }
    }
    Ok(())
}

pub(crate) fn status_value(status: MemberStatus) -> String {
    json(status_str(status))
}
