use std::collections::BTreeMap;

use crate::{MemberId, Minor, SplitError, split_weighted};

/// An expense already converted to the group's base currency.
#[derive(Debug, Clone)]
pub struct Expense {
    pub payer: MemberId,
    pub total: Minor,
    pub weights: Vec<(MemberId, u32)>,
}

/// Net balance per member: positive = is owed money, negative = owes money.
/// The sum of all balances is always zero.
///
/// # Errors
/// Propagates [`SplitError`] from invalid expenses.
pub fn balances(expenses: &[Expense]) -> Result<BTreeMap<MemberId, Minor>, SplitError> {
    let mut net: BTreeMap<MemberId, Minor> = BTreeMap::new();
    for e in expenses {
        *net.entry(e.payer).or_default() += e.total;
        for (member, owed) in split_weighted(e.total, &e.weights)? {
            *net.entry(member).or_default() -= owed;
        }
    }
    Ok(net)
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn m(n: u128) -> MemberId {
        MemberId(Uuid::from_u128(n))
    }

    #[test]
    fn simple_dinner() {
        let all = vec![(m(1), 1), (m(2), 1), (m(3), 1)];
        let b = balances(&[
            Expense {
                payer: m(1),
                total: 3000,
                weights: all.clone(),
            },
            Expense {
                payer: m(2),
                total: 600,
                weights: all,
            },
        ])
        .unwrap();
        assert_eq!(b[&m(1)], 1800);
        assert_eq!(b[&m(2)], -600);
        assert_eq!(b[&m(3)], -1200);
        assert_eq!(b.values().sum::<i64>(), 0);
    }
}
