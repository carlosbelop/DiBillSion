---
name: add-feature
description: The path a DiBillSion feature or behaviour change takes through the layers (domain, store and sync, UniFFI core, Android Compose, iOS SwiftUI, strings, tests, docs) and the traps at each one. Use before changing app behaviour, adding a screen, a split mode, a synced field or a core call, and when asked to modify the app.
---

# Adding or changing a feature

Read `docs/STATUS.md` first (CLAUDE.md says so) and check the relevant part of `docs/SPEC.md`.
If the change alters behaviour described in the spec, the spec changes in the same PR (bump its
version line). Anything beyond the plan's scope is the owner's call (CLAUDE.md, "Autonomy").

## 1. Decide where the logic lives

Rule of thumb: **anything that computes money, decides who owes what, or must give the same
result on every device belongs in Rust**, not in Kotlin or Swift (hard rule 3). The apps only
parse input, show results and call the core.

Known exceptions to keep in mind (don't add more without a reason):
- `Split.kt` / `Split.swift` turn what the user typed (percentages, amounts) into `ShareDto`
  lists; the actual split is `preview_split` / the store, in Rust.
- `ReceiptDraft.kt` / `ReceiptDraft.swift` hold the receipt review state on each platform,
  with mirrored tests. A change to one needs the same change and test in the other.

Prefer reusing an existing representation over adding a stored one. Example: percentages are
stored as **parts in hundredths of a percent**, reduced by their GCD (50/30/20 % → 5/3/2), so
the percentage split (#123, #130, #131) needed no new field, no sync change and no migration.

## 2. The layers, bottom up

| Layer | Where | What to do | Tests |
|---|---|---|---|
| Domain | `crates/dibill-domain` | Pure money logic, `i64` minor units, ties by `MemberId`. No floats (`clippy::float_arithmetic` is denied); FX with `rust_decimal`. | Unit tests + `proptest` for invariants (parts sum to total, balances sum to zero). |
| Store | `crates/dibill-store` (`lib.rs`, `schema.rs`, `sync.rs`) | Every write goes through `self.write(group, Entity, vec![set(Field, json(..))])` so it lands in the op log (ADR 0005). Tables are a projection of the ops. | `src/tests.rs`; sync round-trips through `WireOp`. |
| Core facade | `crates/dibill-core/src/lib.rs` (+ `receipt.rs`, `sync.rs`) | Methods on `Core` inside the `#[uniffi::export] impl Core` block; DTOs are `uniffi::Record`; errors map to `CoreError`. Ids cross the boundary as `String`. | `mod tests` at the bottom of `lib.rs`. |
| Bindings | `scripts/gen-bindings.sh` | Never edit or commit them; Gradle and `build-xcframework.sh` regenerate them. | CI job "Kotlin bindings (UniFFI)". |
| Android | `android/app/src/main/kotlin/com/dibillsion/app/` | `AppViewModel` calls the core on `Dispatchers.IO` via `launchCore { }` / `inGroup { }` (reloads detail, groups and syncs). Screens in `ui/`, shared pieces in `ui/Common.kt`, `ui/SplitControls.kt`, colours in `ui/Theme.kt`. | `android/app/src/test/kotlin/…` (JVM unit tests, run against the real core). |
| iOS | `ios/DiBillSion/` | `AppModel` calls the core off the main actor (`inGroup`); views in `Views/`, shared ones in `Views/Common.swift`, `SplitViews.swift`, `SharedByRow.swift`. Mirror the Android behaviour (parity is a goal, see STATUS). | `ios/DiBillSionTests/` (XCTest, macOS CI). |

### Adding a synced field (only if really needed)

- Add a `Field` variant in `crates/dibill-store/src/sync.rs`, with its `name()` and `parse()`
  strings, and its projection. The server stores ops opaquely: no server change.
- **Older apps drop ops whose field they don't know** (`TryFrom<WireOp>` fails, "callers drop
  such ops"). Testers update the APK by hand, so mixed versions are normal. Check what an old
  app shows when it misses the new op, and whether it ever re-reads it after updating.
- New keys in the transaction `Body` JSON need `#[serde(default)]` so old bodies still parse.
- A new SQLite column means appending to `MIGRATIONS` in `schema.rs` (never edit an existing
  entry); the server has its own `crates/dibill-server/migrations/`.
- This is an architecture change: write an ADR (`docs/adr/`, next number, add it to the index).

## 3. Strings, in both languages

- Android: `res/values/strings.xml` (English) **and** `res/values-es/strings.xml` (Spanish).
  Use `stringResource(R.string.…)` in Compose; never hard-code user-visible text.
- iOS: `ios/DiBillSion/Localizable.xcstrings` (English source, Spanish translation). SwiftUI
  `Text("…")` literals are keys; add the `es` translation for every new one.
- Spanish is the owner's and testers' language: write it naturally (Spain), not word for word.
- Original wording only (hard rule 1): don't borrow text, names or layouts from other apps.

## 4. Check it

Before committing (same as CLAUDE.md):

```bash
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo nextest run --workspace
cargo deny check
```

Platform checks:
- Android: `cd android && ./gradlew :app:testDebugUnitTest` (also builds the core and bindings).
- iOS without a Mac: the Linux type-check in `ios/README.md` ("Without a Mac") catches most
  mistakes in the non-UI Swift files; SwiftUI views only build in the macOS CI (`ios.yml` runs
  on every PR touching `ios/` or `crates/`, but it is not a required check, so look at it).
- UI changes: see the `device-check` skill to try it on the owner's Pixel.

## 5. Ship it

Follow the CLAUDE.md workflow (issue → linked branch → PR → CI → squash-merge). Keep one
platform per PR when the change is big (Android first, then iOS parity, as #123 → #131 did),
and in the same PR:
- tick or add the item in `docs/STATUS.md` with the PR link,
- update `docs/SPEC.md` if behaviour changed, `ios/README.md` / `android/README.md` if how to
  build or run changed, an ADR if the architecture changed.
