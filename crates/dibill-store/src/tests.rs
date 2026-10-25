use super::*;

fn eur() -> Currency {
    Currency::from_code("EUR").unwrap()
}

fn usd() -> Currency {
    Currency::from_code("USD").unwrap()
}

struct Trip {
    store: Store,
    group: GroupId,
    a: MemberId,
    b: MemberId,
    c: MemberId,
}

fn trip() -> Trip {
    let store = Store::open_in_memory().unwrap();
    let group = store
        .create_group("Lisbon", Some("🏖"), GroupKind::Trip, eur())
        .unwrap();
    let a = store.add_member(group, "Ana").unwrap();
    let b = store.add_member(group, "Bea").unwrap();
    let c = store.add_member(group, "Carlos").unwrap();
    Trip {
        store,
        group,
        a,
        b,
        c,
    }
}

fn expense(group: GroupId, payer: MemberId, total: Minor, among: &[MemberId]) -> Transaction {
    Transaction {
        id: TransactionId::new(),
        group,
        kind: TransactionKind::Expense,
        title: "Dinner".into(),
        payer,
        currency: Currency::from_code("EUR").unwrap(),
        fx_rate: None,
        date_days: 20_000,
        items: vec![Item {
            label: None,
            total,
            shares: among.iter().map(|m| ItemShare::parts(*m, 1)).collect(),
        }],
    }
}

#[test]
fn migrations_are_idempotent_and_versioned() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("dibill.db");
    let group = {
        let s = Store::open(&path).unwrap();
        s.create_group("Home", None, GroupKind::Home, eur())
            .unwrap()
    };
    let s = Store::open(&path).unwrap();
    let version: u32 = s
        .conn
        .pragma_query_value(None, "user_version", |r| r.get(0))
        .unwrap();
    assert_eq!(version as usize, schema::LATEST);
    let mode: String = s
        .conn
        .pragma_query_value(None, "journal_mode", |r| r.get(0))
        .unwrap();
    assert_eq!(mode, "wal");
    assert_eq!(s.group(group).unwrap().name, "Home");
}

#[test]
fn groups_and_members_round_trip() {
    let t = trip();
    let g = t.store.group(t.group).unwrap();
    assert_eq!(
        (g.name.as_str(), g.emoji.as_deref(), g.kind),
        ("Lisbon", Some("🏖"), GroupKind::Trip)
    );
    assert_eq!(g.base_currency, eur());
    assert!(!g.archived);
    t.store.set_archived(t.group, true).unwrap();
    assert!(t.store.groups().unwrap()[0].archived);

    let names: Vec<_> = t
        .store
        .members(t.group)
        .unwrap()
        .into_iter()
        .map(|m| m.display_name)
        .collect();
    assert_eq!(names, ["Ana", "Bea", "Carlos"]);
    assert!(matches!(
        t.store.group(GroupId(Uuid::nil())),
        Err(StoreError::NotFound)
    ));
}

#[test]
fn transaction_round_trip_and_tombstone() {
    let mut t = trip();
    let mut e = expense(t.group, t.a, 3000, &[t.a, t.b, t.c]);
    e.items.push(Item {
        label: Some("Wine".into()),
        total: 1200,
        shares: vec![ItemShare::amount(t.a, 200), ItemShare::parts(t.b, 1)],
    });
    t.store.save_transaction(&e).unwrap();
    assert_eq!(t.store.transactions(t.group).unwrap(), vec![e.clone()]);

    e.title = "Dinner and wine".into();
    t.store.save_transaction(&e).unwrap();
    assert_eq!(
        t.store.transactions(t.group).unwrap()[0].title,
        "Dinner and wine"
    );

    t.store.delete_transaction(e.id).unwrap();
    assert_eq!(t.store.transactions(t.group).unwrap(), []);
    assert!(matches!(
        t.store.delete_transaction(e.id),
        Err(StoreError::NotFound)
    ));
}

#[test]
fn balances_with_transfers_and_settle_up() {
    let mut t = trip();
    t.store
        .save_transaction(&expense(t.group, t.a, 3000, &[t.a, t.b, t.c]))
        .unwrap();
    t.store
        .save_transaction(&expense(t.group, t.b, 600, &[t.a, t.b, t.c]))
        .unwrap();
    let gb = t.store.group_balances(t.group).unwrap();
    assert_eq!(gb.balances[&t.a], 1800);
    assert_eq!(gb.balances[&t.b], -600);
    assert_eq!(gb.balances[&t.c], -1200);

    for s in gb.suggestions {
        t.store
            .save_transaction(&Transaction::transfer(
                t.group,
                s.from,
                s.to,
                s.amount,
                eur(),
                20_001,
            ))
            .unwrap();
    }
    let after = t.store.group_balances(t.group).unwrap();
    assert!(after.balances.values().all(|v| *v == 0));
    assert_eq!(after.suggestions, []);
}

#[test]
fn foreign_currency_converts_once_per_transaction() {
    let mut t = trip();
    let mut e = expense(t.group, t.a, 1200, &[t.a, t.b]);
    e.currency = usd();
    e.fx_rate = Some("0.9215".parse().unwrap());
    e.items.push(Item {
        label: None,
        total: 850,
        shares: vec![ItemShare::parts(t.c, 1)],
    });
    e.items.push(Item {
        label: None,
        total: 1100,
        shares: vec![
            ItemShare::parts(t.a, 1),
            ItemShare::parts(t.b, 1),
            ItemShare::parts(t.c, 1),
        ],
    });
    t.store.save_transaction(&e).unwrap();
    // 31.50 USD -> 29.03 EUR, items 11.06 + 7.83 + 10.14.
    let b = t.store.group_balances(t.group).unwrap().balances;
    assert_eq!(b[&t.a], 2903 - 553 - 338);
    assert_eq!(b[&t.b], -553 - 338);
    assert_eq!(b[&t.c], -783 - 338);
    assert_eq!(b.values().sum::<Minor>(), 0);
}

#[test]
fn exact_amounts_are_rescaled_after_conversion() {
    let mut t = trip();
    let mut e = expense(t.group, t.a, 1000, &[]);
    e.currency = usd();
    e.fx_rate = Some("0.5".parse().unwrap());
    e.items[0].shares = vec![ItemShare::amount(t.b, 700), ItemShare::amount(t.c, 300)];
    t.store.save_transaction(&e).unwrap();
    let b = t.store.group_balances(t.group).unwrap().balances;
    assert_eq!((b[&t.a], b[&t.b], b[&t.c]), (500, -350, -150));
}

#[test]
fn validation() {
    let mut t = trip();
    let mut e = expense(t.group, t.a, 1000, &[t.a]);
    e.currency = usd();
    assert!(matches!(
        t.store.save_transaction(&e),
        Err(StoreError::MissingRate)
    ));
    e.currency = eur();
    e.fx_rate = Some("1.1".parse().unwrap());
    assert!(matches!(
        t.store.save_transaction(&e),
        Err(StoreError::UnexpectedRate)
    ));

    let nobody = expense(t.group, t.a, 1000, &[]);
    assert!(matches!(
        t.store.save_transaction(&nobody),
        Err(StoreError::Split(SplitError::NoParticipants))
    ));

    t.store
        .set_member_status(t.c, MemberStatus::Inactive)
        .unwrap();
    let with_c = expense(t.group, t.a, 1000, &[t.a, t.c]);
    assert!(matches!(
        t.store.save_transaction(&with_c),
        Err(StoreError::Member(MemberError::Inactive(_)))
    ));
    // An inactive member can still settle up.
    t.store
        .save_transaction(&Transaction::transfer(t.group, t.c, t.a, 100, eur(), 1))
        .unwrap();

    let mut bad = Transaction::transfer(t.group, t.a, t.b, 100, eur(), 1);
    bad.items[0].shares[0] = ItemShare::amount(t.b, 50);
    assert!(matches!(
        t.store.save_transaction(&bad),
        Err(StoreError::MalformedTransfer)
    ));
    let to_self = Transaction::transfer(t.group, t.a, t.a, 100, eur(), 1);
    assert!(matches!(
        t.store.save_transaction(&to_self),
        Err(StoreError::Split(SplitError::InvalidTransfer))
    ));

    let other = t
        .store
        .create_group("Other", None, GroupKind::Other, eur())
        .unwrap();
    let stranger = t.store.add_member(other, "Zoe").unwrap();
    let foreign = expense(t.group, t.a, 1000, &[stranger]);
    assert!(matches!(
        t.store.save_transaction(&foreign),
        Err(StoreError::Member(MemberError::Unknown(_)))
    ));
}

#[test]
fn members_with_history_cannot_be_deleted() {
    let mut t = trip();
    let e = expense(t.group, t.a, 1000, &[t.a, t.b]);
    t.store.save_transaction(&e).unwrap();
    t.store.delete_transaction(e.id).unwrap();
    assert!(matches!(
        t.store.delete_member(t.b),
        Err(StoreError::MemberHasHistory)
    ));
    t.store.delete_member(t.c).unwrap();
    assert_eq!(t.store.members(t.group).unwrap().len(), 2);
}

#[test]
fn deleting_a_group_removes_everything() {
    let mut t = trip();
    t.store
        .save_transaction(&expense(t.group, t.a, 1000, &[t.a, t.b]))
        .unwrap();
    t.store.delete_group(t.group).unwrap();
    assert_eq!(t.store.groups().unwrap(), []);
    for table in ["members", "transactions", "items", "item_shares"] {
        let n: i64 = t
            .store
            .conn
            .query_row(&format!("SELECT count(*) FROM {table}"), [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 0, "{table}");
    }
}

#[test]
fn a_transaction_id_cannot_move_between_groups() {
    let mut t = trip();
    let e = expense(t.group, t.a, 1000, &[t.a]);
    t.store.save_transaction(&e).unwrap();
    let other = t
        .store
        .create_group("Other", None, GroupKind::Other, eur())
        .unwrap();
    let z = t.store.add_member(other, "Zoe").unwrap();
    let moved = Transaction {
        group: other,
        ..expense(other, z, 1000, &[z])
    };
    let moved = Transaction { id: e.id, ..moved };
    assert!(matches!(
        t.store.save_transaction(&moved),
        Err(StoreError::NotFound)
    ));
    assert_eq!(t.store.transactions(t.group).unwrap(), vec![e]);
}
