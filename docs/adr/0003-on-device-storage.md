# ADR 0003: On-device storage

- Status: accepted
- Date: 2026-10-25

## Context

The apps are offline-first (SPEC §1.1). Both platforms need the same data, the same queries and the
same rules, and the store must be ready for the sync op-log (M5) without a rewrite.

## Decision

- `dibill-store` uses SQLite through `rusqlite` with the `bundled` feature, so Android and iOS run the
  same SQLite version. WAL journal, `synchronous = NORMAL`, foreign keys on, `STRICT` tables.
- Migrations are an append-only list of SQL scripts; `PRAGMA user_version` records how many ran. All
  pending migrations run in one transaction at open. No migration framework dependency.
- Money is `INTEGER` minor units in the transaction's own currency; the exchange rate is the decimal
  string of the stored `FxRate`. Base-currency amounts are **derived** at read time with
  `convert_items` (ADR 0002), never stored, so a corrected rate fixes every balance.
- Transfers are transactions of kind `transfer` with one item and one exact-amount share (the
  receiver), matching SPEC §3.
- Deleting a transaction writes a tombstone (`deleted_at`), which sync will need. Members referenced
  by any transaction, including deleted ones, can only be deactivated.
- Every write is validated with `dibill-domain` before it reaches SQL.

## Consequences

- Balances are recomputed from all transactions of a group. The benchmark (BENCHMARKS.md) shows
  ~1.5 µs per expense, fine for real groups; add a cached snapshot table only if profiling asks.
- Exact-amount shares in a foreign currency are re-scaled onto the converted item total so that the
  converted split still sums exactly.
