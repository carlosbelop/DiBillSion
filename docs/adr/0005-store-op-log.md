# ADR 0005: Op log in the on-device store

- Status: accepted
- Date: 2026-10-30

## Context

ADR 0004 defines the sync model: ops, Lamport stamps, last writer wins per field, permanent
deletes. ADR 0003 defines the SQLite store, whose tables the apps read. The store has to record
local changes as ops, apply remote ops, and give the same rows on every device that has seen the
same ops, including when ops arrive out of order.

## Decision

- **The op log is the source of truth.** Schema v2 adds `ops` (one row per op, value as JSON) and
  `sync_device` (this install's id and Lamport counter). Every write, local or remote, inserts
  ops and then rebuilds the affected rows from the merged state of the entity. Local and remote
  changes share one code path; the merge is the `dibill-sync` replica, run per entity.
- **Values are JSON.** Names, kinds, currencies and statuses are strings, `archived` is a boolean,
  and a transaction's `body` is the whole transaction. The server relays values without reading
  them. Ops whose value doesn't fit their field are dropped on arrival, so every stored value parses.
- **Rows are a pure function of the ops.** A group appears once its creation fields have arrived.
  A member appears once its group exists. A transaction appears while its winning body is valid
  for the group *as it is now*: the payer and every share member exist (in any status, even
  deleted) and every item splits cleanly. Otherwise its rows are removed until the missing ops
  arrive; each remote batch rebuilds the whole group, so these transactions show up later.
  Member status is not checked when rebuilding, so a concurrent deactivation can't hide an expense.
- **Creation order comes from UUID v7 ids**, not from the local clock, so lists are ordered the
  same on every device.
- **Deletes are final.** Saving a deleted transaction fails with `NotFound`; "undo" has to create
  a new one. A deleted group removes its rows; later ops for it are kept but never shown.
- **Members.** Deleting a member is still allowed only without history and writes a tombstone. If
  another device concurrently used that member in a transaction, the member is shown again as
  inactive so balances stay complete and match on every device.
- **Existing installs** get their data backfilled as ops once, when the device id is first created.
- **Push bookkeeping.** Local ops start as pending; `mark_pushed` acknowledges them. Ops received
  from the server are not pending.

## Consequences

- Correctness rests on two property tests: stores that edit offline and sync at random converge,
  and their rows equal what the reference replica says wins.
- A remote batch rebuilds its whole group: O(ops in the group). Fine for real groups; switch to
  rebuilding only touched entities plus pending ones if profiling shows a problem.
- Editing a transaction replaces the whole body, so two people editing different items of the same
  receipt at the same time keep only one version. Per-item registers can come later if receipts
  (M7) need them.
