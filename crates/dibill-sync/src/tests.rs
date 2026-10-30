use proptest::prelude::*;
use uuid::Uuid;

use crate::{Change, DeviceId, LamportClock, Op, Replica, Stamp};

type R = Replica<u8, u8, u8>;
type TestOp = Op<u8, u8, u8>;

fn device(n: u8) -> DeviceId {
    DeviceId(Uuid::from_u128(u128::from(n) + 1))
}

fn set(field: u8, value: u8) -> Change<u8, u8> {
    Change::Set { field, value }
}

#[test]
fn clock_ticks_and_jumps_past_remote_stamps() {
    let mut clock = LamportClock::new(device(0));
    assert_eq!(clock.tick().lamport, 1);
    assert_eq!(clock.tick().lamport, 2);
    clock.observe(Stamp {
        lamport: 10,
        device: device(1),
    });
    assert_eq!(clock.tick().lamport, 11);
    // An older stamp does not move the clock back.
    clock.observe(Stamp {
        lamport: 3,
        device: device(1),
    });
    assert_eq!(clock.tick().lamport, 12);
    assert_eq!(LamportClock::resume(device(0), 12).tick().lamport, 13);
}

#[test]
fn stamps_order_by_counter_then_device() {
    let s = |lamport, d| Stamp {
        lamport,
        device: device(d),
    };
    assert!(s(1, 9) < s(2, 0));
    assert!(s(2, 0) < s(2, 1));
}

#[test]
fn later_edit_wins_in_any_delivery_order() {
    let mut a = R::new(device(0));
    let mut b = R::new(device(1));
    let first = a.local(1, set(0, 10));
    b.apply(&first);
    let second = b.local(1, set(0, 20)); // b has seen `first`, so this is later.

    let mut forward = R::new(device(2));
    forward.apply(&first);
    forward.apply(&second);
    let mut backward = R::new(device(3));
    backward.apply(&second);
    backward.apply(&first);
    assert_eq!(forward.get(&1, &0), Some(&20));
    assert_eq!(backward.get(&1, &0), Some(&20));
}

#[test]
fn concurrent_edits_resolve_by_device_on_equal_counters() {
    let mut a = R::new(device(0));
    let mut b = R::new(device(1));
    let from_a = a.local(1, set(0, 10));
    let from_b = b.local(1, set(0, 20));
    a.apply(&from_b);
    b.apply(&from_a);
    assert_eq!(a.get(&1, &0), Some(&20));
    assert_eq!(a.state(), b.state());
}

#[test]
fn fields_merge_independently() {
    let mut a = R::new(device(0));
    let mut b = R::new(device(1));
    let name = a.local(1, set(0, 10));
    let status = b.local(1, set(1, 20));
    a.apply(&status);
    b.apply(&name);
    assert_eq!(a.get(&1, &0), Some(&10));
    assert_eq!(a.get(&1, &1), Some(&20));
    assert_eq!(a.state(), b.state());
}

#[test]
fn delete_wins_over_later_edits() {
    let mut a = R::new(device(0));
    let mut b = R::new(device(1));
    let delete = a.local(1, Change::Delete);
    b.apply(&delete);
    let edit = b.local(1, set(0, 10));
    assert!(edit.stamp > delete.stamp);
    assert_eq!(b.get(&1, &0), None);
    assert!(b.entity(&1).is_some_and(crate::EntityState::is_deleted));
    assert_eq!(b.live().count(), 0);
}

#[test]
fn applying_an_op_twice_is_a_no_op() {
    let mut a = R::new(device(0));
    let op = a.local(1, set(0, 10));
    let mut b = R::new(device(1));
    assert!(b.apply(&op));
    let before = b.state().clone();
    assert!(!b.apply(&op));
    assert_eq!(b.state(), &before);
}

#[derive(Debug, Clone)]
enum Action {
    Set {
        device: u8,
        entity: u8,
        field: u8,
        value: u8,
    },
    Delete {
        device: u8,
        entity: u8,
    },
    /// `to` receives every op `from` has produced so far.
    Pull {
        from: u8,
        to: u8,
    },
}

const DEVICES: u8 = 3;

fn action() -> impl Strategy<Value = Action> {
    prop_oneof![
        4 => (0..DEVICES, 0..4u8, 0..3u8, any::<u8>()).prop_map(|(device, entity, field, value)| {
            Action::Set { device, entity, field, value }
        }),
        1 => (0..DEVICES, 0..4u8).prop_map(|(device, entity)| Action::Delete { device, entity }),
        2 => (0..DEVICES, 0..DEVICES).prop_map(|(from, to)| Action::Pull { from, to }),
    ]
}

/// Runs the actions on one replica per device and returns them with every op made.
fn simulate(actions: &[Action]) -> (Vec<R>, Vec<TestOp>) {
    let mut replicas: Vec<R> = (0..DEVICES).map(|d| R::new(device(d))).collect();
    let mut logs: Vec<Vec<TestOp>> = vec![Vec::new(); DEVICES.into()];
    for action in actions {
        match *action {
            Action::Set {
                device,
                entity,
                field,
                value,
            } => {
                let op = replicas[usize::from(device)].local(entity, set(field, value));
                logs[usize::from(device)].push(op);
            }
            Action::Delete { device, entity } => {
                let op = replicas[usize::from(device)].local(entity, Change::Delete);
                logs[usize::from(device)].push(op);
            }
            Action::Pull { from, to } => {
                for op in &logs[usize::from(from)] {
                    replicas[usize::from(to)].apply(op);
                }
            }
        }
    }
    (replicas, logs.concat())
}

proptest! {
    /// After every device has seen every op, all devices hold the same state.
    #[test]
    fn devices_converge(actions in prop::collection::vec(action(), 0..60)) {
        let (mut replicas, ops) = simulate(&actions);
        for replica in &mut replicas {
            for op in &ops {
                replica.apply(op);
            }
        }
        for replica in &replicas[1..] {
            prop_assert_eq!(replica.state(), replicas[0].state());
        }
    }

    /// The state depends only on the set of ops: any order, with duplicates,
    /// gives the same result.
    #[test]
    fn delivery_order_and_duplicates_do_not_matter(
        actions in prop::collection::vec(action(), 0..60),
        picks in prop::collection::vec(any::<prop::sample::Index>(), 0..120),
    ) {
        let (_, ops) = simulate(&actions);
        let mut in_order = R::new(device(9));
        for op in &ops {
            in_order.apply(op);
        }
        let mut shuffled = R::new(device(8));
        if !ops.is_empty() {
            for pick in &picks {
                shuffled.apply(&ops[pick.index(ops.len())]);
            }
        }
        for op in ops.iter().rev() {
            shuffled.apply(op);
        }
        prop_assert_eq!(shuffled.state(), in_order.state());
    }

    /// A local op always orders after every op the device has seen.
    #[test]
    fn local_ops_follow_everything_seen(actions in prop::collection::vec(action(), 0..60)) {
        let (mut replicas, ops) = simulate(&actions);
        let replica = &mut replicas[0];
        for op in &ops {
            replica.apply(op);
        }
        let next = replica.local(0, set(0, 0));
        prop_assert!(ops.iter().all(|op| op.stamp < next.stamp));
    }
}
