use std::collections::BTreeMap;

use crate::{MemberId, Minor, SplitError, Transfer, split_weighted};

/// Who shares one item, and in what proportion. Equal split = all weights 1;
/// an exact amount per member = weights in minor units that sum to the total.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ItemShare {
    pub member: MemberId,
    pub weight: u32,
}

/// One line of an expense: a receipt row, or the whole amount of a manual
/// expense. Each item is split independently among its own members.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpenseItem {
    pub total: Minor,
    pub shares: Vec<ItemShare>,
}

/// Whether money left or entered the group through the payer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ExpenseKind {
    /// The payer paid for the items on behalf of their members.
    #[default]
    Expense,
    /// The payer received money that belongs to the items' members (a refund,
    /// a returned deposit). Signs flip: the payer ends up owing it.
    Income,
}

/// An expense or income already converted to the group's base currency.
/// What the payer paid (or received) is the sum of the item totals.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Expense {
    pub kind: ExpenseKind,
    pub payer: MemberId,
    pub items: Vec<ExpenseItem>,
}

impl Expense {
    /// A manual expense: a single item covering the whole amount.
    #[must_use]
    pub fn single(payer: MemberId, total: Minor, shares: Vec<ItemShare>) -> Self {
        Self {
            kind: ExpenseKind::Expense,
            payer,
            items: vec![ExpenseItem { total, shares }],
        }
    }

    /// Money `receiver` got on behalf of the members in `shares`.
    #[must_use]
    pub fn income(receiver: MemberId, total: Minor, shares: Vec<ItemShare>) -> Self {
        Self {
            kind: ExpenseKind::Income,
            ..Self::single(receiver, total, shares)
        }
    }

    /// Sum of the item totals, or `None` on overflow.
    #[must_use]
    pub fn total(&self) -> Option<Minor> {
        self.items
            .iter()
            .try_fold(0, |acc: Minor, i| acc.checked_add(i.total))
    }
}

/// Net balance per member: positive = is owed money, negative = owes money.
/// The sum of all balances is always zero.
///
/// An income counts with the opposite sign of an identical expense.
///
/// `transfers` are reimbursements already made: the sender's balance goes up
/// and the receiver's goes down, so applying every transfer suggested by
/// [`settle_up`](crate::settle_up) brings all balances to zero.
///
/// # Errors
/// Propagates [`SplitError`] from invalid items; [`SplitError::NoItems`] for
/// an expense without items; [`SplitError::InvalidTransfer`] for a transfer
/// with a non-positive amount or the same sender and receiver.
pub fn balances(
    expenses: &[Expense],
    transfers: &[Transfer],
) -> Result<BTreeMap<MemberId, Minor>, SplitError> {
    let mut net: BTreeMap<MemberId, Minor> = BTreeMap::new();
    let mut weights = Vec::new();
    for e in expenses {
        if e.items.is_empty() {
            return Err(SplitError::NoItems);
        }
        let sign = match e.kind {
            ExpenseKind::Expense => 1,
            ExpenseKind::Income => -1,
        };
        for item in &e.items {
            weights.clear();
            weights.extend(item.shares.iter().map(|s| (s.member, s.weight)));
            *net.entry(e.payer).or_default() += sign * item.total;
            for (member, owed) in split_weighted(item.total, &weights)? {
                *net.entry(member).or_default() -= sign * owed;
            }
        }
    }
    for t in transfers {
        if t.amount <= 0 || t.from == t.to {
            return Err(SplitError::InvalidTransfer);
        }
        *net.entry(t.from).or_default() += t.amount;
        *net.entry(t.to).or_default() -= t.amount;
    }
    Ok(net)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settle_up;
    use proptest::prelude::*;
    use uuid::Uuid;

    fn m(n: u128) -> MemberId {
        MemberId(Uuid::from_u128(n))
    }

    fn equal(members: &[u128]) -> Vec<ItemShare> {
        members
            .iter()
            .map(|&n| ItemShare {
                member: m(n),
                weight: 1,
            })
            .collect()
    }

    #[test]
    fn simple_dinner() {
        let b = balances(
            &[
                Expense::single(m(1), 3000, equal(&[1, 2, 3])),
                Expense::single(m(2), 600, equal(&[1, 2, 3])),
            ],
            &[],
        )
        .unwrap();
        assert_eq!(b[&m(1)], 1800);
        assert_eq!(b[&m(2)], -600);
        assert_eq!(b[&m(3)], -1200);
        assert_eq!(b.values().sum::<i64>(), 0);
    }

    #[test]
    fn receipt_items_charge_only_their_members() {
        // A pays a 31.50 receipt: pizza 12.00 (A, B), salad 8.50 (C only),
        // drinks 11.00 shared by all three.
        let receipt = Expense {
            kind: ExpenseKind::Expense,
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
        assert_eq!(receipt.total(), Some(3150));
        let b = balances(&[receipt], &[]).unwrap();
        // Drinks: 11.00 / 3 = 3.67, 3.67, 3.66 (leftover cents go to the lowest ids).
        assert_eq!(b[&m(1)], 3150 - 600 - 367);
        assert_eq!(b[&m(2)], -600 - 367);
        assert_eq!(b[&m(3)], -850 - 366);
        assert_eq!(b.values().sum::<i64>(), 0);
    }

    #[test]
    fn expense_without_items_is_rejected() {
        let e = Expense {
            kind: ExpenseKind::Expense,
            payer: m(1),
            items: vec![],
        };
        assert_eq!(balances(&[e], &[]), Err(SplitError::NoItems));
    }

    #[test]
    fn transfers_move_balances() {
        let dinner = [Expense::single(m(1), 3000, equal(&[1, 2, 3]))];
        // B pays A back 10.00 of the 10.00 they owe.
        let paid_back = [Transfer {
            from: m(2),
            to: m(1),
            amount: 1000,
        }];
        let b = balances(&dinner, &paid_back).unwrap();
        assert_eq!(b[&m(1)], 1000);
        assert_eq!(b[&m(2)], 0);
        assert_eq!(b[&m(3)], -1000);
    }

    #[test]
    fn income_flips_signs() {
        // A gets a 30.00 deposit back that belonged to A, B and C.
        let b = balances(&[Expense::income(m(1), 3000, equal(&[1, 2, 3]))], &[]).unwrap();
        assert_eq!(b[&m(1)], -2000);
        assert_eq!(b[&m(2)], 1000);
        assert_eq!(b[&m(3)], 1000);
    }

    #[test]
    fn invalid_transfers_are_rejected() {
        for t in [
            Transfer {
                from: m(1),
                to: m(2),
                amount: 0,
            },
            Transfer {
                from: m(1),
                to: m(2),
                amount: -5,
            },
            Transfer {
                from: m(1),
                to: m(1),
                amount: 5,
            },
        ] {
            assert_eq!(balances(&[], &[t]), Err(SplitError::InvalidTransfer));
        }
    }

    fn arb_item() -> impl Strategy<Value = ExpenseItem> {
        (
            0i64..10_000_000,
            proptest::collection::btree_map(0u128..8, 0u32..5, 1..8),
        )
            .prop_filter("needs a non-zero weight", |(_, ws)| {
                ws.values().any(|w| *w > 0)
            })
            .prop_map(|(total, ws)| ExpenseItem {
                total,
                shares: ws
                    .into_iter()
                    .map(|(n, weight)| ItemShare {
                        member: m(n),
                        weight,
                    })
                    .collect(),
            })
    }

    fn arb_expense() -> impl Strategy<Value = Expense> {
        (
            any::<bool>(),
            0u128..8,
            proptest::collection::vec(arb_item(), 1..6),
        )
            .prop_map(|(income, payer, items)| Expense {
                kind: if income {
                    ExpenseKind::Income
                } else {
                    ExpenseKind::Expense
                },
                payer: m(payer),
                items,
            })
    }

    proptest! {
        #[test]
        fn balances_sum_to_zero(
            es in proptest::collection::vec(arb_expense(), 0..20),
            ts in proptest::collection::vec((0u128..8, 0u128..8, 1i64..1_000_000), 0..10),
        ) {
            let ts: Vec<_> = ts
                .into_iter()
                .filter(|(f, t, _)| f != t)
                .map(|(f, t, amount)| Transfer { from: m(f), to: m(t), amount })
                .collect();
            let b = balances(&es, &ts).unwrap();
            prop_assert_eq!(b.values().sum::<i64>(), 0);
        }

        #[test]
        fn income_cancels_identical_expense(e in arb_expense()) {
            let flipped = Expense {
                kind: match e.kind {
                    ExpenseKind::Expense => ExpenseKind::Income,
                    ExpenseKind::Income => ExpenseKind::Expense,
                },
                ..e.clone()
            };
            let b = balances(&[e, flipped], &[]).unwrap();
            prop_assert!(b.values().all(|v| *v == 0));
        }

        #[test]
        fn settling_up_zeroes_every_balance(es in proptest::collection::vec(arb_expense(), 0..20)) {
            let before = balances(&es, &[]).unwrap();
            let after = balances(&es, &settle_up(&before)).unwrap();
            prop_assert!(after.values().all(|v| *v == 0));
        }

        #[test]
        fn single_item_matches_whole_expense_split(e in arb_expense()) {
            let item = &e.items[0];
            let one = Expense { kind: e.kind, payer: e.payer, items: vec![item.clone()] };
            let b = balances(&[one], &[]).unwrap();
            let weights: Vec<_> = item.shares.iter().map(|s| (s.member, s.weight)).collect();
            for (member, owed) in split_weighted(item.total, &weights).unwrap() {
                let sign = if e.kind == ExpenseKind::Income { -1 } else { 1 };
                let paid = if member == e.payer { item.total } else { 0 };
                prop_assert_eq!(b[&member], sign * (paid - owed));
            }
        }
    }
}
