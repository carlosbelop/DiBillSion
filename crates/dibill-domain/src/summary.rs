//! One member's view of a group's spending ("My summary", SPEC §7).

use crate::{Expense, MemberId, Minor, Portion, SplitError, split_item};

/// A member's part in one expense.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MemberShare {
    /// What the member's items cost them, in the expense's currency (minor units).
    pub amount: Minor,
    /// Items the member takes part in (a non-zero share).
    pub items_in: usize,
    /// Items in the expense.
    pub items: usize,
}

impl MemberShare {
    /// Whether the member takes part in at least one item.
    #[must_use]
    pub fn is_in(&self) -> bool {
        self.items_in > 0
    }
}

/// `member`'s part in `expense`: the sum of their exact shares over every item
/// (the same split [`balances`](crate::balances) uses) and how many items they are in.
/// The kind (expense or income) is ignored: amounts are never negative.
///
/// # Errors
/// [`SplitError`] from an invalid item.
pub fn member_share(expense: &Expense, member: MemberId) -> Result<MemberShare, SplitError> {
    let mut share = MemberShare {
        items: expense.items.len(),
        ..MemberShare::default()
    };
    for item in &expense.items {
        let takes_part = item.shares.iter().any(|s| {
            s.member == member
                && match s.portion {
                    Portion::Parts(w) => w > 0,
                    Portion::Amount(a) => a > 0,
                }
        });
        if !takes_part {
            continue;
        }
        share.items_in += 1;
        let owed = split_item(item.total, &item.shares)?
            .into_iter()
            .find(|(m, _)| *m == member)
            .map_or(0, |(_, a)| a);
        share.amount = share
            .amount
            .checked_add(owed)
            .ok_or(SplitError::AmountsExceedTotal)?;
    }
    Ok(share)
}

/// `part` as a share of `whole` in basis points (hundredths of a percent, 0..=10000
/// when `0 <= part <= whole`), rounded half up. 0 when `whole` is not positive.
#[must_use]
pub fn basis_points(part: Minor, whole: Minor) -> u32 {
    if whole <= 0 || part <= 0 {
        return 0;
    }
    let (part, whole) = (i128::from(part), i128::from(whole));
    let bp = (part * 20_000 + whole) / (whole * 2);
    u32::try_from(bp).unwrap_or(u32::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ExpenseItem, ItemShare, balances};
    use proptest::prelude::*;
    use uuid::Uuid;

    fn m(n: u128) -> MemberId {
        MemberId(Uuid::from_u128(n))
    }

    fn equal(members: &[u128]) -> Vec<ItemShare> {
        members.iter().map(|&n| ItemShare::parts(m(n), 1)).collect()
    }

    #[test]
    fn receipt_counts_only_my_items() {
        // Pizza 12.00 (A, B), salad 8.50 (C), drinks 11.00 (A, B, C).
        let receipt = Expense {
            kind: crate::ExpenseKind::Expense,
            payer: m(1),
            items: vec![
                ExpenseItem {
                    total: 1200,
                    shares: equal(&[1, 2]),
                },
                ExpenseItem {
                    total: 850,
                    shares: equal(&[3]),
                },
                ExpenseItem {
                    total: 1100,
                    shares: equal(&[1, 2, 3]),
                },
            ],
        };
        let a = member_share(&receipt, m(1)).unwrap();
        assert_eq!((a.amount, a.items_in, a.items), (600 + 367, 2, 3));
        let c = member_share(&receipt, m(3)).unwrap();
        assert_eq!((c.amount, c.items_in), (850 + 366, 2));
        assert!(!member_share(&receipt, m(9)).unwrap().is_in());
    }

    #[test]
    fn zero_parts_and_amounts_are_not_taking_part() {
        let e = Expense::single(
            m(1),
            1000,
            vec![
                ItemShare::parts(m(1), 0),
                ItemShare::amount(m(2), 0),
                ItemShare::parts(m(3), 1),
            ],
        );
        assert!(!member_share(&e, m(1)).unwrap().is_in());
        assert!(!member_share(&e, m(2)).unwrap().is_in());
        assert_eq!(member_share(&e, m(3)).unwrap().amount, 1000);
    }

    #[test]
    fn basis_points_round_half_up() {
        assert_eq!(basis_points(1, 3), 3333);
        assert_eq!(basis_points(2, 3), 6667);
        assert_eq!(basis_points(1, 8), 1250);
        assert_eq!(basis_points(5, 5), 10_000);
        assert_eq!(basis_points(0, 5), 0);
        assert_eq!(basis_points(5, 0), 0);
        assert_eq!(basis_points(i64::MAX, i64::MAX), 10_000);
    }

    proptest! {
        /// Everyone's shares add up to the expense total, and match what balances charge.
        #[test]
        fn shares_sum_to_total(
            items in prop::collection::vec((0i64..1_000_000, prop::collection::vec(0u32..5, 1..5)), 1..6),
        ) {
            let items: Vec<ExpenseItem> = items
                .into_iter()
                .map(|(total, weights)| {
                    let mut shares: Vec<ItemShare> = weights
                        .iter()
                        .enumerate()
                        .map(|(i, &w)| ItemShare::parts(m(i as u128 + 1), w))
                        .collect();
                    // Keep every item valid: someone always takes part.
                    if weights.iter().all(|&w| w == 0) {
                        shares[0] = ItemShare::parts(m(1), 1);
                    }
                    ExpenseItem { total, shares }
                })
                .collect();
            let e = Expense { kind: crate::ExpenseKind::Expense, payer: m(99), items };
            let total = e.total().unwrap();
            let sum: Minor = (1..=5).map(|n| member_share(&e, m(n)).unwrap().amount).sum();
            prop_assert_eq!(sum, total);
            let net = balances(std::slice::from_ref(&e), &[]).unwrap();
            for n in 1..=5 {
                let owed = member_share(&e, m(n)).unwrap().amount;
                prop_assert_eq!(net.get(&m(n)).copied().unwrap_or(0), -owed);
            }
            let bp: u32 = (1..=5).map(|n| basis_points(member_share(&e, m(n)).unwrap().amount, total)).sum();
            prop_assert!(total == 0 || (9_995..=10_005).contains(&bp));
        }
    }
}
