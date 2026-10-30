# ADR 0004: Sync model

- Status: accepted
- Date: 2026-10-30

## Context

The apps are offline-first (SPEC §1.1): any device can edit a shared group without a connection,
and every device must end up with the same group once it has seen the same changes (M5 exit
criterion: "two devices converge after offline edits"). SPEC §6 fixes the user-visible rule:
last writer wins per field, deletes are tombstones. PLAN §3.5 chose an op-log with Lamport clocks
over a general CRDT library.

Every transaction must stay valid as a whole: its items, shares, payer, currency and rate are
checked together by `dibill-domain` (ADR 0003). Merging two concurrent edits field by field
inside one transaction could produce a split that no device ever validated.

## Decision

- **Ops.** Every local change is recorded as an immutable op: `(op id, group, stamp, entity,
  change)`. Op ids are UUID v7 made on the device. A change either sets one field of an entity to
  a whole value, or deletes the entity.
- **Clock.** Each device keeps a Lamport counter. It ticks once per local op and jumps past every
  remote stamp it observes. A stamp is `(lamport, device id)`; stamps are totally ordered by
  counter, then device id, so there are no ties between devices. Wall-clock time is never used to
  order ops (device clocks drift and can be set by hand).
- **Registers.** For each `(entity, field)` the value with the greatest stamp wins. The fields are:
  - group: name, emoji, kind, archived. The base currency is set once, by the group's creation,
    because stored exchange rates depend on it.
  - member: display name, status, linked user.
  - transaction: one field, `body`, holding the whole validated transaction (kind, title, payer,
    currency, rate, date, items, shares). Concurrent edits of the same transaction resolve to one
    of the edited versions, never a mix.
- **Tombstones.** Deleting an entity is permanent: a delete wins over every edit of that entity,
  earlier, concurrent or later. Edits after a delete are kept in the log but have no effect.
  Synced members are never deleted, only deactivated (they may be referenced by transactions on
  other devices that we have not seen yet).
- **Merge is a pure function** of the set of ops seen: applying the same ops in any order, any
  number of times, gives the same state. `dibill-sync` holds the clock, the ordering and an
  in-memory reference replica with property tests for this; the SQLite store (next step) applies
  the same rule per row and is tested against the reference replica.
- **Server role.** The server authenticates, stores and relays ops. It assigns each accepted op a
  per-group sequence number used only as a pull cursor ("give me everything after N"); it does
  not change which value wins. The wire format and API are a separate ADR.

## Consequences

- No coordination is needed to edit offline, and convergence follows from the merge rule alone.
- A delete cannot be undone by a concurrent edit. "Undo delete" in the UI has to create a new
  entity (a copy with a new id).
- An entity can arrive before the entity it references (a transaction before its payer) when ops
  are delivered out of order. The store must keep such ops and resolve references when reading,
  not reject them.
- The op log grows forever. Compaction (snapshot + truncation once every member device has
  acknowledged a cursor) is deferred until real groups need it.
