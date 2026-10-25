# ADR 0001: Shared Rust core with native UI shells

- Status: accepted
- Date: 2026-10-25

## Context

The app must ship on Android and iOS, run fast, work offline, and compute money exactly the same way on
every device. The receipt-scanning feature must plug into the same data model.

## Decision

- All domain logic, storage (SQLite) and sync live in a Rust workspace (`crates/`).
- UI is native: Jetpack Compose on Android, SwiftUI on iOS, calling Rust through UniFFI-generated bindings.
- Money is stored as `i64` minor units. FX conversion uses `rust_decimal`.
- Expenses are modelled as line items from day one (a manual expense = one item), so receipt scanning
  only has to create items.

## Consequences

- One implementation and one test suite for the logic. Native performance and accessibility.
- Two thin UI codebases to maintain. If that becomes a burden, the shells can be replaced with Flutter +
  `flutter_rust_bridge` without touching the core.
- iOS builds require macOS (local Mac or CI macOS runner).
