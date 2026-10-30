//! Sync model (ADR 0004): every change is an immutable [`Op`] stamped by a
//! Lamport clock, and the state of a group is a pure function of the set of
//! ops seen. For each `(entity, field)` the greatest stamp wins; a delete wins
//! over every edit of its entity.
//!
//! [`Replica`] is the in-memory reference implementation of that rule. The
//! SQLite store applies the same rule per row and is tested against it.

use std::collections::{BTreeMap, BTreeSet};

use uuid::Uuid;

#[cfg(test)]
mod tests;

/// One install of the app. Made once, on first launch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DeviceId(pub Uuid);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct OpId(pub Uuid);

impl OpId {
    #[must_use]
    pub fn new() -> Self {
        Self(Uuid::now_v7())
    }
}

impl Default for OpId {
    fn default() -> Self {
        Self::new()
    }
}

/// When an op happened, in causal order. Ordered by counter, then device, so
/// two devices never produce equal stamps.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Stamp {
    pub lamport: u64,
    pub device: DeviceId,
}

/// A device's Lamport clock. Persist [`counter`](Self::counter) and restore it
/// with [`resume`](Self::resume) so stamps keep growing across restarts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LamportClock {
    device: DeviceId,
    counter: u64,
}

impl LamportClock {
    #[must_use]
    pub fn new(device: DeviceId) -> Self {
        Self::resume(device, 0)
    }

    #[must_use]
    pub fn resume(device: DeviceId, counter: u64) -> Self {
        Self { device, counter }
    }

    #[must_use]
    pub fn device(&self) -> DeviceId {
        self.device
    }

    #[must_use]
    pub fn counter(&self) -> u64 {
        self.counter
    }

    /// The stamp for the next local op.
    pub fn tick(&mut self) -> Stamp {
        self.counter = self.counter.saturating_add(1);
        Stamp {
            lamport: self.counter,
            device: self.device,
        }
    }

    /// Moves past a stamp seen on another device, so our next op orders after it.
    pub fn observe(&mut self, stamp: Stamp) {
        self.counter = self.counter.max(stamp.lamport);
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Change<F, V> {
    /// Sets one field of the entity to a whole value.
    Set { field: F, value: V },
    /// Deletes the entity for good.
    Delete,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Op<E, F, V> {
    pub id: OpId,
    pub stamp: Stamp,
    pub entity: E,
    pub change: Change<F, V>,
}

impl<E, F, V> Op<E, F, V> {
    /// Total order used to pick winners. The op id only breaks ties between
    /// ops that reuse a stamp, which an honest device never does.
    fn key(&self) -> (Stamp, OpId) {
        (self.stamp, self.id)
    }
}

/// The merged state of one entity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntityState<F, V> {
    fields: BTreeMap<F, ((Stamp, OpId), V)>,
    deleted: Option<(Stamp, OpId)>,
}

impl<F, V> Default for EntityState<F, V> {
    fn default() -> Self {
        Self {
            fields: BTreeMap::new(),
            deleted: None,
        }
    }
}

impl<F: Ord, V> EntityState<F, V> {
    #[must_use]
    pub fn is_deleted(&self) -> bool {
        self.deleted.is_some()
    }

    /// The winning value of a field, ignoring deletion.
    #[must_use]
    pub fn field(&self, field: &F) -> Option<&V> {
        self.fields.get(field).map(|(_, v)| v)
    }

    /// The stamp of the winning value of a field.
    #[must_use]
    pub fn field_stamp(&self, field: &F) -> Option<Stamp> {
        self.fields.get(field).map(|((s, _), _)| *s)
    }
}

/// An in-memory replica of a set of entities.
#[derive(Debug, Clone)]
pub struct Replica<E, F, V> {
    clock: LamportClock,
    seen: BTreeSet<OpId>,
    entities: BTreeMap<E, EntityState<F, V>>,
}

impl<E, F, V> Replica<E, F, V>
where
    E: Ord + Clone,
    F: Ord + Clone,
    V: Clone,
{
    #[must_use]
    pub fn new(device: DeviceId) -> Self {
        Self {
            clock: LamportClock::new(device),
            seen: BTreeSet::new(),
            entities: BTreeMap::new(),
        }
    }

    #[must_use]
    pub fn clock(&self) -> &LamportClock {
        &self.clock
    }

    /// Records a local change and returns the op to send to other devices.
    pub fn local(&mut self, entity: E, change: Change<F, V>) -> Op<E, F, V> {
        let op = Op {
            id: OpId::new(),
            stamp: self.clock.tick(),
            entity,
            change,
        };
        self.apply(&op);
        op
    }

    /// Merges an op. Returns `false` if it had been applied before.
    pub fn apply(&mut self, op: &Op<E, F, V>) -> bool {
        if !self.seen.insert(op.id) {
            return false;
        }
        self.clock.observe(op.stamp);
        let state = self.entities.entry(op.entity.clone()).or_default();
        let key = op.key();
        match &op.change {
            Change::Set { field, value } => match state.fields.get_mut(field) {
                Some(current) if current.0 >= key => {}
                Some(current) => *current = (key, value.clone()),
                None => {
                    state.fields.insert(field.clone(), (key, value.clone()));
                }
            },
            Change::Delete => state.deleted = state.deleted.max(Some(key)),
        }
        true
    }

    /// The merged state of an entity, deleted or not.
    #[must_use]
    pub fn entity(&self, entity: &E) -> Option<&EntityState<F, V>> {
        self.entities.get(entity)
    }

    /// The winning value of a field of a live entity.
    #[must_use]
    pub fn get(&self, entity: &E, field: &F) -> Option<&V> {
        self.entity(entity)
            .filter(|s| !s.is_deleted())
            .and_then(|s| s.field(field))
    }

    /// Entities that are not deleted, in id order.
    pub fn live(&self) -> impl Iterator<Item = (&E, &EntityState<F, V>)> {
        self.entities.iter().filter(|(_, s)| !s.is_deleted())
    }

    /// The merged state of every entity. Two replicas that have seen the same
    /// ops have equal states, whatever the order they saw them in.
    #[must_use]
    pub fn state(&self) -> &BTreeMap<E, EntityState<F, V>> {
        &self.entities
    }
}
