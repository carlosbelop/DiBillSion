//! Domain model and money algorithms for DiBillSion.
//!
//! All amounts are integer *minor units* (cents). Algorithms work in a single
//! currency; [`convert`] brings foreign amounts into it first.

mod balance;
mod currency;
mod settle;
mod split;

pub use balance::{Expense, ExpenseItem, ExpenseKind, balances};
pub use currency::{Currency, CurrencyError, FxError, FxRate, convert, convert_items};
pub use rust_decimal::Decimal;
pub use settle::{Transfer, settle_up};
pub use split::{ItemShare, Portion, SplitError, allocate, split_item, split_weighted};

use uuid::Uuid;

/// Amount in minor units (e.g. cents). Never use floats for money.
pub type Minor = i64;

/// Stable member identifier. Ordering is used to break ties deterministically,
/// so every device computes identical results.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MemberId(pub Uuid);

impl MemberId {
    #[must_use]
    pub fn new() -> Self {
        Self(Uuid::now_v7())
    }
}

impl Default for MemberId {
    fn default() -> Self {
        Self::new()
    }
}
