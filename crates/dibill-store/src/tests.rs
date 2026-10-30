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

#[test]
fn who_am_i_is_per_group_and_local() {
    let t = trip();
    assert_eq!(t.store.me(t.group).unwrap(), None);
    t.store.set_me(t.group, Some(t.b)).unwrap();
    assert_eq!(t.store.me(t.group).unwrap(), Some(t.b));

    let other = t
        .store
        .create_group("Other", None, GroupKind::Other, eur())
        .unwrap();
    let z = t.store.add_member(other, "Zoe").unwrap();
    assert!(matches!(
        t.store.set_me(t.group, Some(z)),
        Err(StoreError::NotFound)
    ));
    assert_eq!(t.store.me(other).unwrap(), None);

    // Not part of what is synced.
    let ops_before = t.store.group_ops(t.group).unwrap().len();
    t.store.set_me(t.group, Some(t.a)).unwrap();
    assert_eq!(t.store.group_ops(t.group).unwrap().len(), ops_before);

    // A removed member is no longer "me".
    t.store.delete_member(t.a).unwrap();
    assert_eq!(t.store.me(t.group).unwrap(), None);
    t.store.set_me(t.group, None).unwrap();
}

mod sync {
    use proptest::prelude::*;

    use super::*;

    type Snapshot = Vec<(Group, Vec<Member>, Vec<Transaction>, GroupBalances)>;

    fn snapshot(store: &Store) -> Snapshot {
        store
            .groups()
            .unwrap()
            .into_iter()
            .map(|g| {
                let members = store.members(g.id).unwrap();
                let transactions = store.transactions(g.id).unwrap();
                let balances = store.group_balances(g.id).unwrap();
                (g, members, transactions, balances)
            })
            .collect()
    }

    /// A second device that has received everything in `t`'s group.
    fn replica_of(t: &Trip) -> Store {
        let mut other = Store::open_in_memory().unwrap();
        other
            .apply_remote(&t.store.group_ops(t.group).unwrap())
            .unwrap();
        other
    }

    fn exchange(a: &mut Store, b: &mut Store, group: GroupId) {
        let from_a = a.group_ops(group).unwrap();
        let from_b = b.group_ops(group).unwrap();
        a.apply_remote(&from_b).unwrap();
        b.apply_remote(&from_a).unwrap();
    }

    #[test]
    fn writes_are_recorded_as_pending_ops() {
        let mut t = trip();
        let pending = t.store.pending_ops(1000).unwrap();
        // Group: 4 fields; 3 members: name + status each.
        assert_eq!(pending.len(), 10);
        let device = t.store.device_id().unwrap();
        assert!(pending.iter().all(|op| op.stamp.device == device));
        assert!(pending.windows(2).all(|w| w[0].stamp < w[1].stamp));
        let ids: Vec<OpId> = pending.iter().map(|op| op.id).collect();
        t.store.mark_pushed(&ids[..4]).unwrap();
        assert_eq!(t.store.pending_ops(1000).unwrap(), pending[4..]);
    }

    #[test]
    fn a_new_device_rebuilds_the_group_from_ops() {
        let mut t = trip();
        let mut e = expense(t.group, t.a, 3000, &[t.a, t.b, t.c]);
        e.currency = usd();
        e.fx_rate = Some("0.9".parse().unwrap());
        t.store.save_transaction(&e).unwrap();
        let gone = expense(t.group, t.b, 500, &[t.a]);
        t.store.save_transaction(&gone).unwrap();
        t.store.delete_transaction(gone.id).unwrap();
        t.store
            .set_member_status(t.c, MemberStatus::Inactive)
            .unwrap();
        t.store.set_archived(t.group, true).unwrap();

        let other = replica_of(&t);
        assert_eq!(snapshot(&other), snapshot(&t.store));
        // Received ops are not pending on the new device.
        assert_eq!(other.pending_ops(1000).unwrap(), []);
    }

    #[test]
    fn ops_can_arrive_in_any_order() {
        let mut t = trip();
        t.store
            .save_transaction(&expense(t.group, t.a, 1000, &[t.a, t.b]))
            .unwrap();
        let ops = t.store.group_ops(t.group).unwrap();
        let mut other = Store::open_in_memory().unwrap();
        // The transaction arrives first, then the members, then the group.
        for op in ops.iter().rev() {
            other.apply_remote(std::slice::from_ref(op)).unwrap();
        }
        assert_eq!(snapshot(&other), snapshot(&t.store));
        assert_eq!(other.apply_remote(&ops).unwrap(), 0, "all known");
    }

    #[test]
    fn concurrent_edits_of_a_transaction_resolve_to_one_version() {
        let mut t = trip();
        let e = expense(t.group, t.a, 1000, &[t.a, t.b]);
        t.store.save_transaction(&e).unwrap();
        let mut other = replica_of(&t);

        let mine = Transaction {
            title: "Mine".into(),
            ..expense(t.group, t.a, 2000, &[t.a])
        };
        let theirs = Transaction {
            title: "Theirs".into(),
            ..expense(t.group, t.b, 3000, &[t.b, t.c])
        };
        t.store
            .save_transaction(&Transaction {
                id: e.id,
                ..mine.clone()
            })
            .unwrap();
        other
            .save_transaction(&Transaction {
                id: e.id,
                ..theirs.clone()
            })
            .unwrap();
        exchange(&mut t.store, &mut other, t.group);

        let merged = snapshot(&t.store);
        assert_eq!(snapshot(&other), merged);
        let title = &merged[0].2[0].title;
        assert!(title == "Mine" || title == "Theirs");
    }

    #[test]
    fn a_delete_wins_over_a_concurrent_edit() {
        let mut t = trip();
        let e = expense(t.group, t.a, 1000, &[t.a, t.b]);
        t.store.save_transaction(&e).unwrap();
        let mut other = replica_of(&t);
        t.store.delete_transaction(e.id).unwrap();
        other
            .save_transaction(&Transaction {
                title: "Edited later".into(),
                ..e.clone()
            })
            .unwrap();
        exchange(&mut t.store, &mut other, t.group);
        assert_eq!(t.store.transactions(t.group).unwrap(), []);
        assert_eq!(snapshot(&other), snapshot(&t.store));
        // And it stays deleted locally too.
        assert!(matches!(
            other.save_transaction(&e),
            Err(StoreError::NotFound)
        ));
    }

    #[test]
    fn a_deleted_member_used_concurrently_shows_as_inactive() {
        let mut t = trip();
        let mut other = replica_of(&t);
        t.store.delete_member(t.c).unwrap();
        other
            .save_transaction(&expense(t.group, t.a, 900, &[t.a, t.c]))
            .unwrap();
        exchange(&mut t.store, &mut other, t.group);
        let members = t.store.members(t.group).unwrap();
        let c = members.iter().find(|m| m.id == t.c).unwrap();
        assert_eq!(c.status, MemberStatus::Inactive);
        assert_eq!(
            t.store.group_balances(t.group).unwrap().balances[&t.c],
            -450
        );
        assert_eq!(snapshot(&other), snapshot(&t.store));
    }

    #[test]
    fn deleting_a_group_reaches_other_devices() {
        let mut t = trip();
        let mut other = replica_of(&t);
        t.store.delete_group(t.group).unwrap();
        other.add_member(t.group, "Late").unwrap();
        exchange(&mut t.store, &mut other, t.group);
        assert_eq!(t.store.groups().unwrap(), []);
        assert_eq!(other.groups().unwrap(), []);
    }

    #[test]
    fn ops_survive_the_wire() {
        let mut t = trip();
        let e = expense(t.group, t.a, 1000, &[t.a, t.b]);
        t.store.save_transaction(&e).unwrap();
        t.store.delete_transaction(e.id).unwrap();
        let ops = t.store.group_ops(t.group).unwrap();
        let json: Vec<String> = ops
            .iter()
            .map(|op| serde_json::to_string(&dibill_proto::WireOp::from(op)).unwrap())
            .collect();
        let back: Vec<SyncOp> = json
            .iter()
            .map(|j| {
                let wire: dibill_proto::WireOp = serde_json::from_str(j).unwrap();
                assert_eq!(wire.check(), Ok(()));
                SyncOp::try_from(wire).unwrap()
            })
            .collect();
        assert_eq!(back, ops);

        let mut other = Store::open_in_memory().unwrap();
        other.apply_remote(&back).unwrap();
        assert_eq!(snapshot(&other), snapshot(&t.store));
    }

    #[test]
    fn unknown_wire_fields_are_refused() {
        let t = trip();
        let mut wire = dibill_proto::WireOp::from(&t.store.group_ops(t.group).unwrap()[0]);
        wire.field = Some("colour".into());
        assert!(SyncOp::try_from(wire).is_err());
    }

    #[test]
    fn malformed_ops_are_dropped() {
        let t = trip();
        let mut ops = t.store.group_ops(t.group).unwrap();
        for op in &mut ops {
            if let Change::Set {
                field: Field::Name,
                value,
            } = &mut op.change
            {
                *value = "not json".into();
            }
        }
        let mut other = Store::open_in_memory().unwrap();
        // Only the 3 member statuses and 3 group fields other than the name survive.
        assert_eq!(other.apply_remote(&ops).unwrap(), 6);
        assert_eq!(other.groups().unwrap(), []);
    }

    #[test]
    fn data_from_before_the_op_log_is_backfilled() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("v1.db");
        let (group, a, b) = (Uuid::now_v7(), Uuid::now_v7(), Uuid::now_v7());
        {
            let mut conn = Connection::open(&path).unwrap();
            schema::migrate_to(&mut conn, 1).unwrap();
            conn.execute_batch(&format!(
                "INSERT INTO groups VALUES (x'{g}', 'Old', NULL, 'home', 'EUR', 1, 2);
                 INSERT INTO members VALUES (x'{a}', x'{g}', 'Ana', NULL, 'active', 1);
                 INSERT INTO members VALUES (x'{b}', x'{g}', 'Bea', NULL, 'inactive', 2);
                 INSERT INTO transactions VALUES (x'{t}', x'{g}', 'expense', 'Rent', x'{a}', 'EUR', NULL, 1, 1, 1, NULL);
                 INSERT INTO items VALUES (x'{t}', 0, NULL, 1000);
                 INSERT INTO item_shares VALUES (x'{t}', 0, x'{a}', 1, NULL);
                 INSERT INTO item_shares VALUES (x'{t}', 0, x'{b}', 1, NULL);",
                g = group.simple(),
                a = a.simple(),
                b = b.simple(),
                t = Uuid::now_v7().simple(),
            ))
            .unwrap();
        }
        let old = Store::open(&path).unwrap();
        let ops = old.group_ops(GroupId(group)).unwrap();
        assert_eq!(ops.len(), 5 + 2 * 2 + 1);
        let mut other = Store::open_in_memory().unwrap();
        other.apply_remote(&ops).unwrap();
        assert_eq!(snapshot(&other), snapshot(&old));
        // Reopening doesn't backfill twice.
        drop(old);
        let old = Store::open(&path).unwrap();
        assert_eq!(old.group_ops(GroupId(group)).unwrap().len(), ops.len());
    }

    #[derive(Debug, Clone)]
    enum Action {
        AddMember(usize),
        SetActive(usize, usize, bool),
        DeleteMember(usize, usize),
        Save {
            device: usize,
            payer: usize,
            among: u8,
            total: Minor,
        },
        Edit(usize, usize, Minor),
        Delete(usize, usize),
        Archive(usize, bool),
        Sync(usize, usize),
    }

    const DEVICES: usize = 3;

    fn action() -> impl Strategy<Value = Action> {
        let d = 0..DEVICES;
        prop_oneof![
            1 => d.clone().prop_map(Action::AddMember),
            1 => (d.clone(), 0..6usize, any::<bool>()).prop_map(|(d, m, a)| Action::SetActive(d, m, a)),
            1 => (d.clone(), 0..6usize).prop_map(|(d, m)| Action::DeleteMember(d, m)),
            4 => (d.clone(), 0..6usize, 1..64u8, 1..10_000i64)
                .prop_map(|(device, payer, among, total)| Action::Save { device, payer, among, total }),
            2 => (d.clone(), 0..8usize, 1..10_000i64).prop_map(|(d, t, n)| Action::Edit(d, t, n)),
            1 => (d.clone(), 0..8usize).prop_map(|(d, t)| Action::Delete(d, t)),
            1 => (d.clone(), any::<bool>()).prop_map(|(d, a)| Action::Archive(d, a)),
            3 => (d.clone(), d).prop_map(|(f, t)| Action::Sync(f, t)),
        ]
    }

    /// Runs one action; validation errors (an inactive payer, say) are expected.
    fn run(stores: &mut [Store], group: GroupId, action: &Action) {
        let members = |s: &Store| s.members(group).unwrap();
        let _ = match *action {
            Action::AddMember(d) => stores[d].add_member(group, "New").map(drop),
            Action::SetActive(d, m, active) => {
                let Some(m) = members(&stores[d]).get(m).map(|m| m.id) else {
                    return;
                };
                let status = if active {
                    MemberStatus::Active
                } else {
                    MemberStatus::Inactive
                };
                stores[d].set_member_status(m, status)
            }
            Action::DeleteMember(d, m) => {
                let Some(m) = members(&stores[d]).get(m).map(|m| m.id) else {
                    return;
                };
                stores[d].delete_member(m)
            }
            Action::Save {
                device,
                payer,
                among,
                total,
            } => {
                let all = members(&stores[device]);
                let Some(payer) = all.get(payer).map(|m| m.id) else {
                    return;
                };
                let among: Vec<MemberId> = (0..6)
                    .filter(|i| among & (1 << i) != 0)
                    .filter_map(|i| all.get(i).map(|m| m.id))
                    .collect();
                stores[device].save_transaction(&expense(group, payer, total, &among))
            }
            Action::Edit(d, t, total) => {
                let Some(mut e) = stores[d].transactions(group).unwrap().get(t).cloned() else {
                    return;
                };
                e.items[0].total = total;
                stores[d].save_transaction(&e)
            }
            Action::Delete(d, t) => {
                let Some(id) = stores[d].transactions(group).unwrap().get(t).map(|t| t.id) else {
                    return;
                };
                stores[d].delete_transaction(id)
            }
            Action::Archive(d, archived) => stores[d].set_archived(group, archived),
            Action::Sync(from, to) => {
                let ops = stores[from].group_ops(group).unwrap();
                stores[to].apply_remote(&ops).map(drop)
            }
        };
    }

    /// Converging is not enough: the rows must also be what the reference merge
    /// in `dibill-sync` says wins.
    fn check_against_reference(
        store: &Store,
        group: GroupId,
        ops: &[SyncOp],
    ) -> std::result::Result<(), TestCaseError> {
        let mut reference: dibill_sync::Replica<Entity, Field, String> =
            dibill_sync::Replica::new(DeviceId(Uuid::nil()));
        for op in ops {
            reference.apply(&dibill_sync::Op {
                id: op.id,
                stamp: op.stamp,
                entity: op.entity,
                change: op.change.clone(),
            });
        }
        let json = |v: &str| serde_json::from_str::<serde_json::Value>(v).unwrap();
        // Winning values, deleted or not.
        let winner = |e: Entity, f: Field| {
            reference
                .entity(&e)
                .and_then(|s| s.field(&f))
                .map(|v| json(v))
        };

        let g = store.group(group).unwrap();
        prop_assert_eq!(
            winner(Entity::Group, Field::Archived)
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
            g.archived
        );

        let shown = store.transactions(group).unwrap();
        for t in &shown {
            prop_assert!(
                reference
                    .get(&Entity::Transaction(t.id), &Field::Body)
                    .is_some(),
                "deleted transaction shown"
            );
            prop_assert_eq!(
                winner(Entity::Transaction(t.id), Field::Body),
                Some(json(&crate::sync::body(t)))
            );
        }
        let rows: Vec<Uuid> = store
            .conn
            .prepare("SELECT id FROM members WHERE group_id = ?1")
            .unwrap()
            .query_map([group.0], |r| r.get(0))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap();
        let mut live = 0;
        for (entity, state) in reference.live() {
            let Entity::Transaction(id) = *entity else {
                continue;
            };
            let Some(body) = state.field(&Field::Body) else {
                continue;
            };
            let body = json(body);
            let mut refs = vec![body["payer"].as_str().unwrap().to_owned()];
            for item in body["items"].as_array().unwrap() {
                for share in item["shares"].as_array().unwrap() {
                    refs.push(share["member"].as_str().unwrap().to_owned());
                }
            }
            if refs.iter().all(|m| rows.contains(&m.parse().unwrap())) {
                live += 1;
                prop_assert!(shown.iter().any(|t| t.id == id), "live transaction missing");
            }
        }
        prop_assert_eq!(live, shown.len());

        for m in store.members(group).unwrap() {
            let e = Entity::Member(m.id);
            prop_assert_eq!(
                winner(e, Field::Name),
                Some(json(&crate::sync::json(&m.display_name)))
            );
            let deleted = reference
                .entity(&e)
                .is_some_and(dibill_sync::EntityState::is_deleted);
            let status = if deleted {
                "inactive".into()
            } else {
                winner(e, Field::Status).unwrap()
            };
            prop_assert_eq!(status, json(&crate::sync::json(status_str(m.status))));
        }
        Ok(())
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(64))]

        /// Devices editing offline and syncing at random end up identical once
        /// they have exchanged everything, and match a device that only ever
        /// received the ops (in reverse order).
        #[test]
        fn devices_converge(actions in prop::collection::vec(action(), 0..40)) {
            let t = trip();
            let group = t.group;
            let mut stores = vec![t.store];
            for _ in 1..DEVICES {
                let mut s = Store::open_in_memory().unwrap();
                s.apply_remote(&stores[0].group_ops(group).unwrap()).unwrap();
                stores.push(s);
            }
            for action in &actions {
                run(&mut stores, group, action);
            }
            let all: Vec<SyncOp> = stores.iter().flat_map(|s| s.group_ops(group).unwrap()).collect();
            for s in &mut stores {
                s.apply_remote(&all).unwrap();
            }
            let mut observer = Store::open_in_memory().unwrap();
            for op in all.iter().rev() {
                observer.apply_remote(std::slice::from_ref(op)).unwrap();
            }
            let expected = snapshot(&observer);
            for s in &stores {
                prop_assert_eq!(&snapshot(s), &expected);
            }
            check_against_reference(&observer, group, &all)?;
        }
    }
}
