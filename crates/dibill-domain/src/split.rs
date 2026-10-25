use crate::{MemberId, Minor};

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SplitError {
    #[error("amount must not be negative")]
    NegativeAmount,
    #[error("at least one participant with a non-zero weight is required")]
    NoParticipants,
    #[error("member listed more than once")]
    DuplicateMember,
    #[error("an expense needs at least one item")]
    NoItems,
    #[error("a transfer needs a positive amount and two different members")]
    InvalidTransfer,
    #[error("exact amounts add up to more than the total")]
    AmountsExceedTotal,
    #[error("exact amounts must add up to the total when nobody shares by parts")]
    AmountsDontAddUp,
}

/// How much of an item one member takes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Portion {
    /// A weight; what is left after exact amounts is split by weight.
    /// Equal split = every member with weight 1. Weight 0 = excluded.
    Parts(u32),
    /// An exact amount in minor units.
    Amount(Minor),
}

/// Who shares one item, and how.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ItemShare {
    pub member: MemberId,
    pub portion: Portion,
}

impl ItemShare {
    #[must_use]
    pub fn parts(member: MemberId, weight: u32) -> Self {
        Self {
            member,
            portion: Portion::Parts(weight),
        }
    }

    #[must_use]
    pub fn amount(member: MemberId, amount: Minor) -> Self {
        Self {
            member,
            portion: Portion::Amount(amount),
        }
    }
}

/// Splits one item: exact amounts are taken first, then the remainder is
/// split by parts with [`split_weighted`]. With only exact amounts they must
/// add up to `total`. Output keeps the input order.
///
/// # Errors
/// See [`SplitError`].
pub fn split_item(
    total: Minor,
    shares: &[ItemShare],
) -> Result<Vec<(MemberId, Minor)>, SplitError> {
    if total < 0 {
        return Err(SplitError::NegativeAmount);
    }
    let mut fixed: i128 = 0;
    let mut weights = Vec::with_capacity(shares.len());
    for s in shares {
        match s.portion {
            Portion::Amount(a) if a < 0 => return Err(SplitError::NegativeAmount),
            Portion::Amount(a) => fixed += i128::from(a),
            Portion::Parts(w) => weights.push((s.member, w)),
        }
    }
    let mut ids: Vec<MemberId> = shares.iter().map(|s| s.member).collect();
    ids.sort_unstable();
    if ids.windows(2).any(|w| w[0] == w[1]) {
        return Err(SplitError::DuplicateMember);
    }
    let rest =
        Minor::try_from(i128::from(total) - fixed).map_err(|_| SplitError::AmountsExceedTotal)?;
    if rest < 0 {
        return Err(SplitError::AmountsExceedTotal);
    }

    let by_parts = if weights.iter().any(|(_, w)| *w > 0) {
        split_weighted(rest, &weights)?
    } else if rest == 0 {
        weights.iter().map(|(id, _)| (*id, 0)).collect()
    } else if weights.len() == shares.len() {
        return Err(SplitError::NoParticipants);
    } else {
        return Err(SplitError::AmountsDontAddUp);
    };

    let mut by_parts = by_parts.into_iter();
    Ok(shares
        .iter()
        .map(|s| match s.portion {
            Portion::Amount(a) => (s.member, a),
            Portion::Parts(_) => by_parts.next().unwrap_or((s.member, 0)),
        })
        .collect())
}

/// Splits `total` among members proportionally to their weights using the
/// largest-remainder method.
///
/// Guarantees: parts sum exactly to `total`, each part differs from the exact
/// proportional value by less than one minor unit, and ties are broken by
/// `MemberId` so the result is deterministic. Equal split = all weights 1.
///
/// The output keeps the input order and includes zero-weight members (with 0).
///
/// # Errors
/// See [`SplitError`].
pub fn split_weighted(
    total: Minor,
    weights: &[(MemberId, u32)],
) -> Result<Vec<(MemberId, Minor)>, SplitError> {
    if total < 0 {
        return Err(SplitError::NegativeAmount);
    }
    let mut ids: Vec<MemberId> = weights.iter().map(|(id, _)| *id).collect();
    ids.sort_unstable();
    if ids.windows(2).any(|w| w[0] == w[1]) {
        return Err(SplitError::DuplicateMember);
    }
    let weight_sum: i128 = weights.iter().map(|(_, w)| i128::from(*w)).sum();
    if weight_sum == 0 {
        return Err(SplitError::NoParticipants);
    }

    let wide: Vec<(MemberId, i128)> = weights
        .iter()
        .map(|(id, w)| (*id, i128::from(*w)))
        .collect();
    Ok(weights
        .iter()
        .zip(largest_remainder(total, &wide, weight_sum))
        .map(|((id, _), part)| (*id, part))
        .collect())
}

/// Allocates `total` across existing amounts in proportion to them (largest
/// remainder, ties broken by position). Used to spread a converted total over
/// the items it came from, so the converted items sum exactly to it.
///
/// # Errors
/// [`SplitError::NegativeAmount`] if `total` or any amount is negative;
/// [`SplitError::NoParticipants`] if all amounts are zero but `total` is not.
pub fn allocate(total: Minor, amounts: &[Minor]) -> Result<Vec<Minor>, SplitError> {
    if total < 0 || amounts.iter().any(|a| *a < 0) {
        return Err(SplitError::NegativeAmount);
    }
    let sum: i128 = amounts.iter().map(|a| i128::from(*a)).sum();
    if sum == 0 {
        return if total == 0 {
            Ok(vec![0; amounts.len()])
        } else {
            Err(SplitError::NoParticipants)
        };
    }
    let wide: Vec<(usize, i128)> = amounts.iter().map(|a| i128::from(*a)).enumerate().collect();
    Ok(largest_remainder(total, &wide, sum))
}

/// Largest-remainder apportionment of a non-negative `total` over
/// non-negative weights summing to `weight_sum > 0`. Leftover units go to the
/// largest remainders, ties to the smallest key. Output keeps input order.
fn largest_remainder<K: Ord + Copy>(
    total: Minor,
    weights: &[(K, i128)],
    weight_sum: i128,
) -> Vec<Minor> {
    let total_wide = i128::from(total);
    let mut parts: Vec<(K, i128, i128)> = weights
        .iter()
        .map(|(k, w)| {
            let num = total_wide * w;
            (*k, num / weight_sum, num % weight_sum)
        })
        .collect();

    let assigned: i128 = parts.iter().map(|(_, q, _)| q).sum();
    // Fewer leftover units than participants, so this always fits in usize.
    let leftover = usize::try_from(total_wide - assigned).unwrap_or(0);

    let mut order: Vec<usize> = (0..parts.len()).collect();
    order.sort_unstable_by(|&a, &b| {
        parts[b]
            .2
            .cmp(&parts[a].2)
            .then(parts[a].0.cmp(&parts[b].0))
    });
    for &i in order.iter().take(leftover) {
        parts[i].1 += 1;
    }

    parts
        .into_iter()
        // Each part is <= total, which is an i64.
        .map(|(_, q, _)| Minor::try_from(q).unwrap_or(Minor::MAX))
        .collect()
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
    fn ten_euros_three_ways() {
        let r = split_weighted(1000, &[(m(1), 1), (m(2), 1), (m(3), 1)]).unwrap();
        assert_eq!(r, vec![(m(1), 334), (m(2), 333), (m(3), 333)]);
    }

    #[test]
    fn weighted() {
        let r = split_weighted(1000, &[(m(1), 2), (m(2), 1), (m(3), 1)]).unwrap();
        assert_eq!(r, vec![(m(1), 500), (m(2), 250), (m(3), 250)]);
    }

    #[test]
    fn zero_weight_member_gets_nothing() {
        let r = split_weighted(101, &[(m(1), 1), (m(2), 0), (m(3), 1)]).unwrap();
        assert_eq!(r, vec![(m(1), 51), (m(2), 0), (m(3), 50)]);
    }

    #[test]
    fn errors() {
        assert_eq!(
            split_weighted(-1, &[(m(1), 1)]),
            Err(SplitError::NegativeAmount)
        );
        assert_eq!(
            split_weighted(10, &[(m(1), 0)]),
            Err(SplitError::NoParticipants)
        );
        assert_eq!(
            split_weighted(10, &[(m(1), 1), (m(1), 1)]),
            Err(SplitError::DuplicateMember)
        );
    }

    #[test]
    fn exact_amounts() {
        let r = split_item(
            1000,
            &[ItemShare::amount(m(1), 700), ItemShare::amount(m(2), 300)],
        );
        assert_eq!(r, Ok(vec![(m(1), 700), (m(2), 300)]));
        assert_eq!(
            split_item(
                1000,
                &[ItemShare::amount(m(1), 700), ItemShare::amount(m(2), 200)]
            ),
            Err(SplitError::AmountsDontAddUp)
        );
        assert_eq!(
            split_item(
                1000,
                &[ItemShare::amount(m(1), 1001), ItemShare::parts(m(2), 1)]
            ),
            Err(SplitError::AmountsExceedTotal)
        );
        assert_eq!(
            split_item(
                1000,
                &[ItemShare::amount(m(1), -1), ItemShare::parts(m(2), 1)]
            ),
            Err(SplitError::NegativeAmount)
        );
        assert_eq!(
            split_item(10, &[ItemShare::amount(m(1), 5), ItemShare::parts(m(1), 1)]),
            Err(SplitError::DuplicateMember)
        );
    }

    #[test]
    fn mixed_split() {
        // 50.00: A had a 20.00 steak, the remaining 30.00 is split 2:1 between B and C.
        let r = split_item(
            5000,
            &[
                ItemShare::parts(m(2), 2),
                ItemShare::amount(m(1), 2000),
                ItemShare::parts(m(3), 1),
            ],
        );
        assert_eq!(r, Ok(vec![(m(2), 2000), (m(1), 2000), (m(3), 1000)]));
        // Amounts cover everything: parts members get 0.
        let r = split_item(
            100,
            &[ItemShare::amount(m(1), 100), ItemShare::parts(m(2), 1)],
        );
        assert_eq!(r, Ok(vec![(m(1), 100), (m(2), 0)]));
        assert_eq!(
            split_item(100, &[ItemShare::parts(m(1), 0)]),
            Err(SplitError::NoParticipants)
        );
    }

    #[test]
    fn allocate_examples() {
        assert_eq!(
            allocate(2903, &[1200, 850, 1100]),
            Ok(vec![1106, 783, 1014])
        );
        assert_eq!(allocate(0, &[0, 0]), Ok(vec![0, 0]));
        assert_eq!(allocate(5, &[0, 0]), Err(SplitError::NoParticipants));
        assert_eq!(allocate(5, &[1, -1]), Err(SplitError::NegativeAmount));
        // Equal remainders: the earlier position wins.
        assert_eq!(allocate(1, &[1, 1]), Ok(vec![1, 0]));
    }

    proptest! {
        #[test]
        fn mixed_split_sums_to_total(
            total in 0i64..10_000_000,
            shares in proptest::collection::vec((any::<bool>(), 0i64..100_000, 0u32..5), 1..12),
        ) {
            let shares: Vec<ItemShare> = shares
                .iter()
                .enumerate()
                .map(|(i, (exact, a, w))| {
                    if *exact { ItemShare::amount(m(i as u128), *a) } else { ItemShare::parts(m(i as u128), *w) }
                })
                .collect();
            match split_item(total, &shares) {
                Ok(out) => {
                    prop_assert_eq!(out.iter().map(|(_, a)| a).sum::<i64>(), total);
                    for (s, (id, a)) in shares.iter().zip(&out) {
                        prop_assert_eq!(s.member, *id);
                        if let Portion::Amount(x) = s.portion { prop_assert_eq!(*a, x); }
                    }
                }
                Err(e) => prop_assert!(matches!(
                    e,
                    SplitError::AmountsExceedTotal | SplitError::AmountsDontAddUp | SplitError::NoParticipants
                )),
            }
        }

        #[test]
        fn allocation_sums_to_total(
            total in 0i64..10_000_000_000,
            amounts in proptest::collection::vec(0i64..10_000_000, 1..30),
        ) {
            prop_assume!(amounts.iter().any(|a| *a > 0));
            let out = allocate(total, &amounts).unwrap();
            prop_assert_eq!(out.iter().sum::<i64>(), total);
            let sum: i128 = amounts.iter().map(|a| i128::from(*a)).sum();
            for (part, a) in out.iter().zip(&amounts) {
                let floor = i128::from(total) * i128::from(*a) / sum;
                prop_assert!(i128::from(*part) == floor || i128::from(*part) == floor + 1);
            }
        }

        #[test]
        fn parts_sum_to_total_and_are_fair(
            total in 0i64..10_000_000_000,
            ws in proptest::collection::vec(0u32..1000, 1..20),
        ) {
            prop_assume!(ws.iter().any(|w| *w > 0));
            let input: Vec<_> = ws.iter().enumerate().map(|(i, w)| (m(i as u128), *w)).collect();
            let out = split_weighted(total, &input).unwrap();
            prop_assert_eq!(out.iter().map(|(_, a)| a).sum::<i64>(), total);

            let wsum: i128 = ws.iter().map(|w| i128::from(*w)).sum();
            for ((_, part), w) in out.iter().zip(&ws) {
                let exact_floor = i128::from(total) * i128::from(*w) / wsum;
                let p = i128::from(*part);
                prop_assert!(p == exact_floor || p == exact_floor + 1);
            }
        }
    }
}
