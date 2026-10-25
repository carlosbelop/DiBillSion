use crate::{MemberId, Minor};

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SplitError {
    #[error("amount must not be negative")]
    NegativeAmount,
    #[error("at least one participant with a non-zero weight is required")]
    NoParticipants,
    #[error("member listed more than once")]
    DuplicateMember,
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

    let total_wide = i128::from(total);
    let mut parts: Vec<(MemberId, i128, i128)> = weights
        .iter()
        .map(|(id, w)| {
            let num = total_wide * i128::from(*w);
            (*id, num / weight_sum, num % weight_sum)
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

    Ok(parts
        .into_iter()
        // Each part is <= total, which is an i64.
        .map(|(id, q, _)| (id, Minor::try_from(q).unwrap_or(Minor::MAX)))
        .collect())
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

    proptest! {
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
