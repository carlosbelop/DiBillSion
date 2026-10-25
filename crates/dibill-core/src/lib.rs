//! App-facing facade. Kotlin (Android) and Swift (iOS) call into this crate
//! through UniFFI-generated bindings. Keep the API coarse-grained:
//! one call per screen/action, not many small getters.
//!
//! IDs cross the boundary as UUID strings; money as `i64` minor units with an
//! ISO 4217 code; exchange rates as decimal strings. No floats.

// UniFFI lifts arguments into owned values, so exported signatures take them by value.
#![allow(clippy::needless_pass_by_value)]

use std::collections::BTreeMap;
use std::sync::{Mutex, MutexGuard};

use dibill_domain::{
    Currency, CurrencyError, FxError, FxRate, ItemShare, MemberError, MemberId, MemberStatus,
    Portion, convert,
};
use dibill_store::{
    GroupId, GroupKind, Item, Store, StoreError, Transaction, TransactionId, TransactionKind,
};
use uuid::Uuid;

pub use dibill_domain as domain;

uniffi::setup_scaffolding!();

/// Version string shown in the app's "About" screen.
#[must_use]
#[uniffi::export]
pub fn core_version() -> String {
    env!("CARGO_PKG_VERSION").to_owned()
}

/// Decimal digits of a currency's minor unit (EUR 2, JPY 0), for formatting.
///
/// # Errors
/// [`CoreError::InvalidInput`] for an unknown code.
#[uniffi::export]
pub fn currency_exponent(code: String) -> Result<u8, CoreError> {
    Ok(currency(&code)?.exponent())
}

/// Typed so the apps can show a localized message per case; `reason` carries
/// English detail for logs only.
#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum CoreError {
    #[error("not found")]
    NotFound,
    #[error("unknown currency: {code}")]
    UnknownCurrency { code: String },
    #[error("a foreign-currency transaction needs an exchange rate")]
    MissingRate,
    #[error("the exchange rate must be a positive number")]
    InvalidRate,
    #[error("member is not in this group")]
    UnknownMember,
    #[error("member is inactive")]
    MemberInactive,
    #[error("member appears in transactions; deactivate them instead")]
    MemberHasHistory,
    // Not `message`: that would clash with `Exception.message` in Kotlin/Swift.
    #[error("the split doesn't add up: {reason}")]
    InvalidSplit { reason: String },
    #[error("invalid input: {reason}")]
    InvalidInput { reason: String },
    #[error("storage error: {reason}")]
    Storage { reason: String },
}

impl From<StoreError> for CoreError {
    fn from(e: StoreError) -> Self {
        let reason = e.to_string();
        match e {
            StoreError::NotFound => Self::NotFound,
            StoreError::Sqlite(_) | StoreError::Corrupt(_) => Self::Storage { reason },
            StoreError::Currency(CurrencyError::UnknownCode(code)) => {
                Self::UnknownCurrency { code }
            }
            StoreError::MissingRate => Self::MissingRate,
            StoreError::Fx(FxError::NonPositiveRate | FxError::InvalidRate) => Self::InvalidRate,
            StoreError::Member(MemberError::Unknown(_)) => Self::UnknownMember,
            StoreError::Member(MemberError::Inactive(_)) => Self::MemberInactive,
            StoreError::MemberHasHistory => Self::MemberHasHistory,
            StoreError::Split(_) | StoreError::MalformedTransfer => Self::InvalidSplit { reason },
            _ => Self::InvalidInput { reason },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum GroupKindDto {
    Trip,
    Home,
    Couple,
    Event,
    Project,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum TransactionKindDto {
    Expense,
    Income,
    Transfer,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct GroupDto {
    pub id: String,
    pub name: String,
    pub emoji: Option<String>,
    pub kind: GroupKindDto,
    pub base_currency: String,
    pub archived: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct MemberDto {
    pub id: String,
    pub display_name: String,
    pub active: bool,
}

/// One member's portion of an item: exactly one of `parts` or `amount_minor`.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct ShareDto {
    pub member_id: String,
    pub parts: Option<u32>,
    pub amount_minor: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct ItemDto {
    pub label: Option<String>,
    pub total_minor: i64,
    pub shares: Vec<ShareDto>,
}

/// A transaction as edited and as listed. `id` is `None` for a new one.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct TransactionDto {
    pub id: Option<String>,
    pub group_id: String,
    pub kind: TransactionKindDto,
    pub title: String,
    pub payer_id: String,
    pub currency: String,
    pub fx_rate: Option<String>,
    pub date_days: i32,
    pub items: Vec<ItemDto>,
    /// Total in the group's base currency when `currency` is foreign. Filled
    /// by the core when listing; ignored when saving.
    pub base_total_minor: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct MemberBalanceDto {
    pub member_id: String,
    pub display_name: String,
    pub amount_minor: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct SuggestionDto {
    pub from_id: String,
    pub from_name: String,
    pub to_id: String,
    pub to_name: String,
    pub amount_minor: i64,
}

/// Everything the balances screen shows, in the group's base currency.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct BalancesDto {
    pub currency: String,
    pub balances: Vec<MemberBalanceDto>,
    pub suggestions: Vec<SuggestionDto>,
}

/// The app's single entry point. Thread-safe; calls are short and blocking,
/// so the apps call them off the UI thread.
#[derive(Debug, uniffi::Object)]
pub struct Core {
    store: Mutex<Store>,
}

#[uniffi::export]
impl Core {
    /// Opens (or creates) the database file.
    ///
    /// # Errors
    /// [`CoreError::Storage`] if the file can't be opened or migrated.
    #[uniffi::constructor]
    pub fn open(path: String) -> Result<Self, CoreError> {
        Ok(Self {
            store: Mutex::new(Store::open(path)?),
        })
    }

    /// An in-memory database, for previews and tests.
    ///
    /// # Errors
    /// [`CoreError::Storage`].
    #[uniffi::constructor]
    pub fn in_memory() -> Result<Self, CoreError> {
        Ok(Self {
            store: Mutex::new(Store::open_in_memory()?),
        })
    }

    /// # Errors
    /// [`CoreError::InvalidInput`] for an unknown currency.
    pub fn create_group(
        &self,
        name: String,
        emoji: Option<String>,
        kind: GroupKindDto,
        base_currency: String,
    ) -> Result<String, CoreError> {
        let id = self.store().create_group(
            &name,
            emoji.as_deref(),
            kind.into(),
            currency(&base_currency)?,
        )?;
        Ok(id.0.to_string())
    }

    /// # Errors
    /// [`CoreError::Storage`].
    pub fn groups(&self) -> Result<Vec<GroupDto>, CoreError> {
        Ok(self
            .store()
            .groups()?
            .into_iter()
            .map(|g| GroupDto {
                id: g.id.0.to_string(),
                name: g.name,
                emoji: g.emoji,
                kind: g.kind.into(),
                base_currency: g.base_currency.code().to_owned(),
                archived: g.archived,
            })
            .collect())
    }

    /// # Errors
    /// [`CoreError::NotFound`].
    pub fn set_group_archived(&self, group_id: String, archived: bool) -> Result<(), CoreError> {
        Ok(self
            .store()
            .set_archived(GroupId(uuid(&group_id)?), archived)?)
    }

    /// # Errors
    /// [`CoreError::NotFound`].
    pub fn delete_group(&self, group_id: String) -> Result<(), CoreError> {
        Ok(self.store().delete_group(GroupId(uuid(&group_id)?))?)
    }

    /// # Errors
    /// [`CoreError::InvalidInput`] for an unknown group.
    pub fn add_member(&self, group_id: String, display_name: String) -> Result<String, CoreError> {
        let id = self
            .store()
            .add_member(GroupId(uuid(&group_id)?), &display_name)?;
        Ok(id.0.to_string())
    }

    /// # Errors
    /// [`CoreError::Storage`].
    pub fn members(&self, group_id: String) -> Result<Vec<MemberDto>, CoreError> {
        Ok(self
            .store()
            .members(GroupId(uuid(&group_id)?))?
            .into_iter()
            .map(|m| MemberDto {
                id: m.id.0.to_string(),
                display_name: m.display_name,
                active: m.status == MemberStatus::Active,
            })
            .collect())
    }

    /// # Errors
    /// [`CoreError::NotFound`].
    pub fn set_member_active(&self, member_id: String, active: bool) -> Result<(), CoreError> {
        let status = if active {
            MemberStatus::Active
        } else {
            MemberStatus::Inactive
        };
        Ok(self
            .store()
            .set_member_status(member(&member_id)?, status)?)
    }

    /// Deletes a member without history; otherwise fails and the app offers
    /// to deactivate them.
    ///
    /// # Errors
    /// [`CoreError::InvalidInput`] if the member has history.
    pub fn delete_member(&self, member_id: String) -> Result<(), CoreError> {
        Ok(self.store().delete_member(member(&member_id)?)?)
    }

    /// Creates or updates a transaction and returns its id.
    ///
    /// # Errors
    /// [`CoreError::InvalidInput`] with a readable reason when validation fails.
    pub fn save_transaction(&self, t: TransactionDto) -> Result<String, CoreError> {
        let t = from_dto(t)?;
        self.store().save_transaction(&t)?;
        Ok(t.id.0.to_string())
    }

    /// # Errors
    /// [`CoreError::NotFound`].
    pub fn delete_transaction(&self, transaction_id: String) -> Result<(), CoreError> {
        Ok(self
            .store()
            .delete_transaction(TransactionId(uuid(&transaction_id)?))?)
    }

    /// Live transactions, newest date first.
    ///
    /// # Errors
    /// [`CoreError::Storage`].
    pub fn transactions(&self, group_id: String) -> Result<Vec<TransactionDto>, CoreError> {
        let store = self.store();
        let group = GroupId(uuid(&group_id)?);
        let base = store.group(group)?.base_currency;
        store
            .transactions(group)?
            .into_iter()
            .map(|t| {
                let base_total = match t.fx_rate {
                    Some(rate) => {
                        let total = t.items.iter().map(|i| i.total).sum();
                        Some(
                            convert(total, t.currency, base, rate)
                                .map_err(|e| invalid(e.to_string()))?,
                        )
                    }
                    None => None,
                };
                Ok(TransactionDto {
                    base_total_minor: base_total,
                    ..to_dto(t)
                })
            })
            .collect()
    }

    /// # Errors
    /// [`CoreError::NotFound`] for an unknown group.
    pub fn balances(&self, group_id: String) -> Result<BalancesDto, CoreError> {
        let store = self.store();
        let group = GroupId(uuid(&group_id)?);
        let gb = store.group_balances(group)?;
        let names: BTreeMap<MemberId, String> = store
            .members(group)?
            .into_iter()
            .map(|m| (m.id, m.display_name))
            .collect();
        let name = |id: &MemberId| names.get(id).cloned().unwrap_or_default();
        Ok(BalancesDto {
            currency: gb.currency.code().to_owned(),
            balances: gb
                .balances
                .iter()
                .map(|(id, amount)| MemberBalanceDto {
                    member_id: id.0.to_string(),
                    display_name: name(id),
                    amount_minor: *amount,
                })
                .collect(),
            suggestions: gb
                .suggestions
                .iter()
                .map(|s| SuggestionDto {
                    from_id: s.from.0.to_string(),
                    from_name: name(&s.from),
                    to_id: s.to.0.to_string(),
                    to_name: name(&s.to),
                    amount_minor: s.amount,
                })
                .collect(),
        })
    }
}

impl Core {
    fn store(&self) -> MutexGuard<'_, Store> {
        // A panic while holding the lock can't leave SQLite half-written
        // (every write is one SQL transaction), so recover the guard.
        self.store
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

impl From<GroupKindDto> for GroupKind {
    fn from(k: GroupKindDto) -> Self {
        match k {
            GroupKindDto::Trip => Self::Trip,
            GroupKindDto::Home => Self::Home,
            GroupKindDto::Couple => Self::Couple,
            GroupKindDto::Event => Self::Event,
            GroupKindDto::Project => Self::Project,
            GroupKindDto::Other => Self::Other,
        }
    }
}

impl From<GroupKind> for GroupKindDto {
    fn from(k: GroupKind) -> Self {
        match k {
            GroupKind::Trip => Self::Trip,
            GroupKind::Home => Self::Home,
            GroupKind::Couple => Self::Couple,
            GroupKind::Event => Self::Event,
            GroupKind::Project => Self::Project,
            GroupKind::Other => Self::Other,
        }
    }
}

fn invalid(reason: impl Into<String>) -> CoreError {
    CoreError::InvalidInput {
        reason: reason.into(),
    }
}

fn uuid(s: &str) -> Result<Uuid, CoreError> {
    Uuid::parse_str(s).map_err(|_| invalid(format!("not an id: {s:?}")))
}

fn member(s: &str) -> Result<MemberId, CoreError> {
    uuid(s).map(MemberId)
}

fn currency(code: &str) -> Result<Currency, CoreError> {
    Currency::from_code(code).map_err(|_| CoreError::UnknownCurrency {
        code: code.to_owned(),
    })
}

fn from_dto(t: TransactionDto) -> Result<Transaction, CoreError> {
    let items = t
        .items
        .into_iter()
        .map(|i| {
            let shares = i
                .shares
                .into_iter()
                .map(|s| {
                    let portion = match (s.parts, s.amount_minor) {
                        (Some(w), None) => Portion::Parts(w),
                        (None, Some(a)) => Portion::Amount(a),
                        _ => return Err(invalid("a share needs exactly one of parts or amount")),
                    };
                    Ok(ItemShare {
                        member: member(&s.member_id)?,
                        portion,
                    })
                })
                .collect::<Result<_, _>>()?;
            Ok(Item {
                label: i.label,
                total: i.total_minor,
                shares,
            })
        })
        .collect::<Result<_, CoreError>>()?;
    Ok(Transaction {
        id: t
            .id
            .as_deref()
            .map(uuid)
            .transpose()?
            .map_or_else(TransactionId::new, TransactionId),
        group: GroupId(uuid(&t.group_id)?),
        kind: match t.kind {
            TransactionKindDto::Expense => TransactionKind::Expense,
            TransactionKindDto::Income => TransactionKind::Income,
            TransactionKindDto::Transfer => TransactionKind::Transfer,
        },
        title: t.title,
        payer: member(&t.payer_id)?,
        currency: currency(&t.currency)?,
        fx_rate: t
            .fx_rate
            .as_deref()
            .map(str::parse::<FxRate>)
            .transpose()
            .map_err(|_| CoreError::InvalidRate)?,
        date_days: t.date_days,
        items,
    })
}

fn to_dto(t: Transaction) -> TransactionDto {
    TransactionDto {
        id: Some(t.id.0.to_string()),
        group_id: t.group.0.to_string(),
        kind: match t.kind {
            TransactionKind::Expense => TransactionKindDto::Expense,
            TransactionKind::Income => TransactionKindDto::Income,
            TransactionKind::Transfer => TransactionKindDto::Transfer,
        },
        title: t.title,
        payer_id: t.payer.0.to_string(),
        currency: t.currency.code().to_owned(),
        fx_rate: t.fx_rate.map(|r| r.value().to_string()),
        date_days: t.date_days,
        base_total_minor: None,
        items: t
            .items
            .into_iter()
            .map(|i| ItemDto {
                label: i.label,
                total_minor: i.total,
                shares: i
                    .shares
                    .into_iter()
                    .map(|s| {
                        let (parts, amount_minor) = match s.portion {
                            Portion::Parts(w) => (Some(w), None),
                            Portion::Amount(a) => (None, Some(a)),
                        };
                        ShareDto {
                            member_id: s.member.0.to_string(),
                            parts,
                            amount_minor,
                        }
                    })
                    .collect(),
            })
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn equal(ids: &[&String]) -> Vec<ShareDto> {
        ids.iter()
            .map(|id| ShareDto {
                member_id: (*id).clone(),
                parts: Some(1),
                amount_minor: None,
            })
            .collect()
    }

    #[test]
    fn full_flow_through_the_facade() {
        let core = Core::in_memory().unwrap();
        let g = core
            .create_group("Lisbon".into(), None, GroupKindDto::Trip, "EUR".into())
            .unwrap();
        let a = core.add_member(g.clone(), "Ana".into()).unwrap();
        let b = core.add_member(g.clone(), "Bea".into()).unwrap();
        let dinner = TransactionDto {
            id: None,
            group_id: g.clone(),
            kind: TransactionKindDto::Expense,
            title: "Dinner".into(),
            payer_id: a.clone(),
            currency: "USD".into(),
            fx_rate: Some("0.9215".into()),
            date_days: 20_000,
            items: vec![ItemDto {
                label: None,
                total_minor: 1000,
                shares: equal(&[&a, &b]),
            }],
            base_total_minor: None,
        };
        let id = core.save_transaction(dinner.clone()).unwrap();
        let listed = core.transactions(g.clone()).unwrap();
        assert_eq!(
            listed,
            vec![TransactionDto {
                id: Some(id),
                base_total_minor: Some(922),
                ..dinner
            }]
        );

        // 10.00 USD -> 9.22 EUR, split 4.61 / 4.61.
        let bal = core.balances(g.clone()).unwrap();
        assert_eq!(bal.currency, "EUR");
        assert_eq!(
            bal.suggestions,
            vec![SuggestionDto {
                from_id: b.clone(),
                from_name: "Bea".into(),
                to_id: a.clone(),
                to_name: "Ana".into(),
                amount_minor: 461
            }]
        );
        assert_eq!(core.groups().unwrap()[0].name, "Lisbon");
        assert_eq!(currency_exponent("JPY".into()).unwrap(), 0);
    }

    #[test]
    fn errors_are_readable() {
        let core = Core::in_memory().unwrap();
        assert!(matches!(
            core.create_group("X".into(), None, GroupKindDto::Other, "EURO".into()),
            Err(CoreError::UnknownCurrency { .. })
        ));
        assert!(matches!(
            core.delete_group(Uuid::nil().to_string()),
            Err(CoreError::NotFound)
        ));
        assert!(matches!(
            core.members("nope".into()),
            Err(CoreError::InvalidInput { .. })
        ));

        let g = core
            .create_group("X".into(), None, GroupKindDto::Other, "EUR".into())
            .unwrap();
        let a = core.add_member(g.clone(), "A".into()).unwrap();
        let bad = TransactionDto {
            base_total_minor: None,
            id: None,
            group_id: g.clone(),
            kind: TransactionKindDto::Expense,
            title: String::new(),
            payer_id: a.clone(),
            currency: "EUR".into(),
            fx_rate: None,
            date_days: 0,
            items: vec![ItemDto {
                label: None,
                total_minor: 100,
                shares: vec![ShareDto {
                    member_id: a.clone(),
                    parts: Some(1),
                    amount_minor: Some(1),
                }],
            }],
        };
        assert!(matches!(
            core.save_transaction(bad.clone()),
            Err(CoreError::InvalidInput { .. })
        ));

        // Each failure the app shows gets its own variant.
        let ok_share = vec![ShareDto {
            member_id: a.clone(),
            parts: Some(1),
            amount_minor: None,
        }];
        let base = TransactionDto {
            items: vec![ItemDto {
                label: None,
                total_minor: 100,
                shares: ok_share,
            }],
            ..bad
        };
        let usd = TransactionDto {
            currency: "USD".into(),
            ..base.clone()
        };
        assert!(matches!(
            core.save_transaction(usd.clone()),
            Err(CoreError::MissingRate)
        ));
        let bad_rate = TransactionDto {
            fx_rate: Some("-1".into()),
            ..usd
        };
        assert!(matches!(
            core.save_transaction(bad_rate),
            Err(CoreError::InvalidRate)
        ));
        let nobody = TransactionDto {
            items: vec![ItemDto {
                label: None,
                total_minor: 100,
                shares: vec![],
            }],
            ..base.clone()
        };
        assert!(matches!(
            core.save_transaction(nobody),
            Err(CoreError::InvalidSplit { .. })
        ));
        core.save_transaction(base.clone()).unwrap();
        assert!(matches!(
            core.delete_member(a.clone()),
            Err(CoreError::MemberHasHistory)
        ));
        core.set_member_active(a, false).unwrap();
        assert!(matches!(
            core.save_transaction(base),
            Err(CoreError::MemberInactive)
        ));
    }
}
