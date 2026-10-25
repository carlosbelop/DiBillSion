use std::collections::BTreeMap;

use crate::{Expense, MemberId, Portion, Transfer};

/// Members with history are never deleted, only deactivated: they keep their
/// balance and can still settle, but are not offered for new expenses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MemberStatus {
    #[default]
    Active,
    Inactive,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum MemberError {
    #[error("member {0:?} is not in the group")]
    Unknown(MemberId),
    #[error("member {0:?} is inactive")]
    Inactive(MemberId),
}

/// Checks a new or edited expense against the group's members: the payer and
/// every member with a non-zero portion must exist and be active. Inactive
/// members may appear with a zero portion (excluded).
///
/// # Errors
/// The first offending member, payer first, then shares in order.
pub fn check_expense_members(
    expense: &Expense,
    members: &BTreeMap<MemberId, MemberStatus>,
) -> Result<(), MemberError> {
    let active = |id: MemberId| match members.get(&id) {
        None => Err(MemberError::Unknown(id)),
        Some(MemberStatus::Inactive) => Err(MemberError::Inactive(id)),
        Some(MemberStatus::Active) => Ok(()),
    };
    active(expense.payer)?;
    for share in expense.items.iter().flat_map(|i| &i.shares) {
        match share.portion {
            Portion::Parts(0) | Portion::Amount(0) => {
                if !members.contains_key(&share.member) {
                    return Err(MemberError::Unknown(share.member));
                }
            }
            _ => active(share.member)?,
        }
    }
    Ok(())
}

/// Checks a new transfer: both members must exist. Inactive members may still
/// settle their balance.
///
/// # Errors
/// [`MemberError::Unknown`] for the first unknown member.
pub fn check_transfer_members(
    transfer: &Transfer,
    members: &BTreeMap<MemberId, MemberStatus>,
) -> Result<(), MemberError> {
    for id in [transfer.from, transfer.to] {
        if !members.contains_key(&id) {
            return Err(MemberError::Unknown(id));
        }
    }
    Ok(())
}

/// A member can be deleted only if no expense or transfer mentions them;
/// otherwise deactivate them.
#[must_use]
pub fn can_delete(member: MemberId, expenses: &[Expense], transfers: &[Transfer]) -> bool {
    let in_expense = |e: &Expense| {
        e.payer == member
            || e.items
                .iter()
                .flat_map(|i| &i.shares)
                .any(|s| s.member == member)
    };
    !expenses.iter().any(in_expense)
        && !transfers.iter().any(|t| t.from == member || t.to == member)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ItemShare;
    use uuid::Uuid;

    fn m(n: u128) -> MemberId {
        MemberId(Uuid::from_u128(n))
    }

    fn group() -> BTreeMap<MemberId, MemberStatus> {
        BTreeMap::from([
            (m(1), MemberStatus::Active),
            (m(2), MemberStatus::Active),
            (m(3), MemberStatus::Inactive),
        ])
    }

    #[test]
    fn inactive_members_cannot_join_new_expenses() {
        let ok = Expense::single(
            m(1),
            1000,
            vec![
                ItemShare::parts(m(1), 1),
                ItemShare::parts(m(2), 1),
                ItemShare::parts(m(3), 0),
            ],
        );
        assert_eq!(check_expense_members(&ok, &group()), Ok(()));

        let with_inactive = Expense::single(m(1), 1000, vec![ItemShare::parts(m(3), 1)]);
        assert_eq!(
            check_expense_members(&with_inactive, &group()),
            Err(MemberError::Inactive(m(3)))
        );
        let inactive_payer = Expense::single(m(3), 1000, vec![ItemShare::parts(m(1), 1)]);
        assert_eq!(
            check_expense_members(&inactive_payer, &group()),
            Err(MemberError::Inactive(m(3)))
        );
        let stranger = Expense::single(m(1), 1000, vec![ItemShare::amount(m(9), 0)]);
        assert_eq!(
            check_expense_members(&stranger, &group()),
            Err(MemberError::Unknown(m(9)))
        );
    }

    #[test]
    fn inactive_members_can_still_settle() {
        let t = Transfer {
            from: m(3),
            to: m(1),
            amount: 500,
        };
        assert_eq!(check_transfer_members(&t, &group()), Ok(()));
        let t = Transfer { to: m(9), ..t };
        assert_eq!(
            check_transfer_members(&t, &group()),
            Err(MemberError::Unknown(m(9)))
        );
    }

    #[test]
    fn only_members_without_history_can_be_deleted() {
        let es = [Expense::single(
            m(1),
            1000,
            vec![ItemShare::parts(m(1), 1), ItemShare::parts(m(2), 0)],
        )];
        let ts = [Transfer {
            from: m(3),
            to: m(1),
            amount: 1,
        }];
        assert!(!can_delete(m(1), &es, &ts));
        assert!(!can_delete(m(2), &es, &ts));
        assert!(!can_delete(m(3), &es, &ts));
        assert!(can_delete(m(4), &es, &ts));
    }
}
