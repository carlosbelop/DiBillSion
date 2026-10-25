//! Domain model and money algorithms for DiBillSion.
//!
//! All amounts are integer *minor units* (cents) of a single currency.
//! Currency conversion happens before values reach this crate.

mod balance;
mod settle;
mod split;

pub use balance::{Expense, balances};
pub use settle::{Transfer, settle_up};
pub use split::{SplitError, split_weighted};

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
