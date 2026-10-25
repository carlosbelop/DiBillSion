use std::collections::BTreeMap;

use crate::{MemberId, Minor};

/// Money sent from one member to another: a suggested or recorded reimbursement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Transfer {
    pub from: MemberId,
    pub to: MemberId,
    pub amount: Minor,
}

/// Suggests reimbursements that zero out all balances.
///
/// Greedy: repeatedly match the largest debtor with the largest creditor.
/// Produces at most `n - 1` transfers (the true minimum is NP-hard) and is
/// deterministic thanks to `MemberId` tie-breaking.
#[must_use]
pub fn settle_up(balances: &BTreeMap<MemberId, Minor>) -> Vec<Transfer> {
    let by_amount_desc =
        |a: &(MemberId, Minor), b: &(MemberId, Minor)| b.1.cmp(&a.1).then(a.0.cmp(&b.0));

    let mut creditors: Vec<(MemberId, Minor)> = balances
        .iter()
        .filter(|(_, v)| **v > 0)
        .map(|(k, v)| (*k, *v))
        .collect();
    let mut debtors: Vec<(MemberId, Minor)> = balances
        .iter()
        .filter(|(_, v)| **v < 0)
        .map(|(k, v)| (*k, -*v))
        .collect();
    creditors.sort_unstable_by(by_amount_desc);
    debtors.sort_unstable_by(by_amount_desc);

    let mut transfers = Vec::with_capacity(creditors.len() + debtors.len());
    let (mut ci, mut di) = (0, 0);
    while ci < creditors.len() && di < debtors.len() {
        let amount = creditors[ci].1.min(debtors[di].1);
        transfers.push(Transfer {
            from: debtors[di].0,
            to: creditors[ci].0,
            amount,
        });
        creditors[ci].1 -= amount;
        debtors[di].1 -= amount;
        if creditors[ci].1 == 0 {
            ci += 1;
        }
        if debtors[di].1 == 0 {
            di += 1;
        }
    }
    transfers
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use uuid::Uuid;

    fn m(n: u128) -> MemberId {
        MemberId(Uuid::from_u128(n))
    }

    #[test]
    fn three_people() {
        let b = BTreeMap::from([(m(1), 1800), (m(2), -600), (m(3), -1200)]);
        assert_eq!(
            settle_up(&b),
            vec![
                Transfer {
                    from: m(3),
                    to: m(1),
                    amount: 1200
                },
                Transfer {
                    from: m(2),
                    to: m(1),
                    amount: 600
                },
            ]
        );
    }

    #[test]
    fn matches_largest_pairs_first() {
        // Paying creditors one after another in list order would need 3 transfers.
        let b = BTreeMap::from([(m(1), 600), (m(2), 400), (m(3), -400), (m(4), -600)]);
        assert_eq!(
            settle_up(&b),
            vec![
                Transfer {
                    from: m(4),
                    to: m(1),
                    amount: 600
                },
                Transfer {
                    from: m(3),
                    to: m(2),
                    amount: 400
                },
            ]
        );
    }

    proptest! {
        #[test]
        fn transfers_zero_out_balances(vals in proptest::collection::vec(-1_000_000i64..1_000_000, 1..30)) {
            // Make the balances sum to zero by adjusting the last member.
            let mut b: BTreeMap<MemberId, Minor> =
                vals.iter().enumerate().map(|(i, v)| (m(i as u128), *v)).collect();
            let s: i64 = b.values().sum();
            *b.entry(m(10_000)).or_default() -= s;

            let ts = settle_up(&b);
            let n_nonzero = b.values().filter(|v| **v != 0).count();
            prop_assert!(ts.len() <= n_nonzero.saturating_sub(1));

            let mut after = b.clone();
            for t in &ts {
                prop_assert!(t.amount > 0);
                *after.get_mut(&t.from).unwrap() += t.amount;
                *after.get_mut(&t.to).unwrap() -= t.amount;
            }
            prop_assert!(after.values().all(|v| *v == 0));
        }
    }
}
