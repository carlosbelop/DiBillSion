# Project status

> Living document. Every session reads it first and updates it in the same PR as the work it describes.
> Milestones are defined in [PLAN.md §4](PLAN.md).

Last updated: 2026-10-30 (session 3)

## Where we are

**M0 (tooling): done. M1 (study): done, SPEC v2 approved. M2 (domain): done.** **M3 (local Android app): in progress** — store, UniFFI facade, SDK/NDK and Compose shell done; first on-device run done (Pixel 9a); only branding (owner) is left. **M5 (backend + sync): in progress** — sync model (ADR 0004), store op log (ADR 0005) and sync API (ADR 0006) and server relay done; invites next.

### Done
- WSL2 Ubuntu 24.04 environment (`scripts/setup-ubuntu.sh`): Rust stable + Android targets, cargo-nextest/deny/llvm-cov/ndk/bloat, Node 24 (nvm), OpenJDK 21, jadx-headless-mcp 0.7.1, Apktool 3.0.3, adb, gh.
- REA registered as MCP server `rea` for Claude Code (`npx -y rea-agents@6.8.0 mcp`).
- Repo scaffold: Cargo workspace, strict lints, CI (fmt, clippy, nextest + coverage, cargo-deny, study-material guard), Dependabot, ADR 0001.
- `dibill-domain`: `split_weighted` (largest remainder, deterministic), `balances`, `settle_up` (greedy), with unit and property tests.
- Study input: `_study/apk/tricount-base.apk`. Package `com.tribab.tricount.android`, v14.5.0 (versionCode 500004150), minSdk 32, targetSdk 37, single APK, 296 MB, sha256 `19d9dbb08353708d4f698752011c334fbdc669cef2f4b9918e3701ce62336edb`.
- Study notes (local only): `_study/notes/01-triage.md`, `02-features.md`, `03-behavior.md`. REA's resource decoder caps at 2,000 strings, so full string tables were decoded with local Apktool into `_study/decoded/`.
- `dibill-domain`: ISO 4217 `Currency` + decimal FX (`convert`, `convert_items`, ADR 0002), line-item expenses, transfers in balances, criterion baseline ([BENCHMARKS.md](BENCHMARKS.md)).

## Backlog (in order)

Mark items `[x]` when merged and link the PR.

### M1: Study (clean room, notes stay in `_study/`)
- [x] **Triage** (PLAN §2.1): `inspect_android_package`, `identify_flutter_build` on the APK. Record stack (Kotlin/Compose? Flutter? RN?), permissions, deep links / invite links, third-party SDKs, obfuscation level → `_study/notes/01-triage.md`.
- [x] **Resources and strings** (PLAN §2.2): `decode_android_resources` (en, es) → feature inventory in our own words → `_study/notes/02-features.md`.
- [x] **Behavior** (PLAN §2.3): answer the 7 questions (data model, split modes, rounding, multi-currency, settle-up, offline/sync and invites, export) → `_study/notes/03-behavior.md`.
- [x] **Black-box observation** (PLAN §2.4) on the phone. ⚠️ Use only a throwaway test group. Never touch the owner's real groups. If something can't be done through adb, ask the owner.
- [x] **Write `docs/SPEC.md`** (own words; the only study output allowed in the repo). v2 approved by the owner on 2026-10-25 (M1 exit criterion met).
- [x] Fold black-box results into SPEC (draft v2): exact-cent guarantee, settle-up example, display and confirmation rules. Notes in `_study/notes/04-blackbox.md` (throwaway group created and deleted; the owner's groups were not opened).

### M2: Domain (`dibill-domain`, no study needed)
- [x] Currency type (ISO 4217 code + minor-unit exponent) and FX conversion with `rust_decimal`, with explicit rounding rules and tests. ([#11](https://github.com/carlosbelop/DiBillSion/pull/11), ADR 0002)
- [x] Line-item model: `ExpenseItem` + `ItemShare`. A manual expense is one item; receipts are many. Balances computed per item. ([#13](https://github.com/carlosbelop/DiBillSion/pull/13))
- [x] Transfers (reimbursements) included in balances. ([#15](https://github.com/carlosbelop/DiBillSion/pull/15))
- [x] `criterion` benchmarks for split, balances and settle-up (1k members, 100k expenses). ([#17](https://github.com/carlosbelop/DiBillSion/pull/17), baseline in [BENCHMARKS.md](BENCHMARKS.md))
- [x] Convert multi-item expenses once and allocate across items ([#19](https://github.com/carlosbelop/DiBillSion/pull/19)).
- [x] Refine `docs/SPEC.md` calculation rules with worked examples from the tests (in the SPEC PR).
- [x] Transaction kinds from SPEC §1: income (signs flip) alongside expense and transfer. ([#23](https://github.com/carlosbelop/DiBillSion/pull/23))
- [x] Split modes from SPEC §4: exact amounts per member and mixed (fixed amounts + parts for the rest), with validation that amounts add up. ([#25](https://github.com/carlosbelop/DiBillSion/pull/25))
- [x] Member status (active/inactive): inactive members excluded from new splits, kept in balances. ([#27](https://github.com/carlosbelop/DiBillSion/pull/27))

### M3: Local-only Android app
- [x] `dibill-store`: SQLite (rusqlite bundled + migrations), WAL, repository API. ([#29](https://github.com/carlosbelop/DiBillSion/pull/29), ADR 0003)
- [x] `dibill-core`: UniFFI facade, Kotlin bindings generation (`scripts/gen-bindings.sh`). ([#31](https://github.com/carlosbelop/DiBillSion/pull/31)) Synchronous for now; the apps call it off the UI thread. Async can come with sync (M5).
- [x] Android SDK + NDK in WSL via command-line tools (`scripts/setup-ubuntu.sh`: platform 37, build-tools 37, NDK r29). Licenses accepted with the owner's approval (2026-10-25). ([#35](https://github.com/carlosbelop/DiBillSion/pull/35))
- [x] Compose shell: groups, members, expenses, balances, settle-up. Offline only. ([#37](https://github.com/carlosbelop/DiBillSion/pull/37)) Builds and passes JVM tests (including a smoke test through the real core); **not yet run on a device**.
- [x] CI job: `cargo ndk` build + Gradle assemble + unit tests, debug APK uploaded as an artifact (same PR).
- [x] Run the app on the Pixel: all core flows worked; fixes for what showed up in [#39](https://github.com/carlosbelop/DiBillSion/pull/39) (localized errors, confirmations, converted amounts). #39 re-checked on the device: confirmations, Spanish errors and wording all work.
- [ ] Finalize `applicationId` (currently placeholder `com.dibillsion.app`) and app icon before M8 (branding: owner). The owner decided on 2026-10-30 to keep the name DiBillSion and the current icon for now; they will change them later.
- [x] UI redesign with our own visual identity (teal/amber palette, avatars, grouped expense list, balance bars, full-screen editors, live exact shares from the core via `preview_split`). The owner asked for a copy of the studied app's UI; per hard rule 1 (clean room) it was redesigned from SPEC flows instead, without copying its layouts, strings, icons or images. ([#50](https://github.com/carlosbelop/DiBillSion/pull/50))
- [x] Owner feedback on the redesign: "who am I" per group (device-local, schema v3 `group_prefs`, not synced), my balance and my payments at the top of Balances and on the group cards, payer defaults to me; swipeable tabs, pull-to-refresh, list and screen animations, an elastic bottom edge that works on short lists; centered title; a tint per group. ([#52](https://github.com/carlosbelop/DiBillSion/pull/52))

### M5: Backend + sync + invite links
Exit criterion: two devices converge after offline edits. Design: [ADR 0004](adr/0004-sync-model.md), [ADR 0005](adr/0005-store-op-log.md), [ADR 0006](adr/0006-sync-api.md).
- [x] Sync model: `dibill-sync` (op ids, Lamport clock, LWW registers per field, permanent tombstones, in-memory reference replica, convergence property tests) + ADR 0004. ([#46](https://github.com/carlosbelop/DiBillSion/pull/46))
- [x] Op log in `dibill-store` (schema v2, [ADR 0005](adr/0005-store-op-log.md)): every write records ops and rows are rebuilt from them; remote ops in any order; pending/pushed bookkeeping; backfill of existing installs. Property test: three stores converge and match the reference replica. ([#48](https://github.com/carlosbelop/DiBillSion/pull/48))
- [x] Sync API and wire format ([ADR 0006](adr/0006-sync-api.md)): JSON over HTTPS under `/v1`, anonymous device tokens, per-group sequence cursor for pull, envelope-only checks on the server, invite codes. Wire types in `dibill-proto`, conversions from the store's `SyncOp`. ([#54](https://github.com/carlosbelop/DiBillSion/pull/54))
- [x] `dibill-server` skeleton: axum + PostgreSQL (`tokio-postgres` + `deadpool`, embedded migrations; `sqlx` clashes with `rusqlite`'s SQLite, see ADR 0006 addendum), `/health`, op push/pull relay with per-group sequence, JSON errors; integration tests against a PostgreSQL 17 service in CI (skipped locally without `DATABASE_URL`). ([#56](https://github.com/carlosbelop/DiBillSion/pull/56))
- [x] Auth: anonymous device tokens (`POST /v1/devices`, SHA-256 hash stored, bearer auth), group membership checks (same PR). Sign-in comes later.
- [ ] Invite links: create/join a shared group, claim a member.
- [ ] Client sync in `dibill-core` (push pending ops, pull since cursor), exposed to Kotlin; Android UI for sharing/joining.
- [ ] Two-device convergence test (two emulators or emulator + Pixel); needs a reachable server (local first, hosting is an owner decision).

### Later
M4 iOS (needs macOS) · M6 polish · M7 receipt scanning (VLM) · M8 store release. See PLAN §4.

## Known issues and tech debt
- Remote Control (and mobile push) is disabled by the owner's organization policy, so escalations go in the session's final message and in this file.
- WSL appends the Windows `PATH`, which includes Windows Node. Always make sure nvm's Node comes first (`source ~/.nvm/nvm.sh`).
- `main` isn't protected by GitHub (free plan, private repo). The PR workflow is self-enforced, and `.claude/settings.json` denies direct pushes to `main`.

## Needs the owner (open requests)
- **Local PostgreSQL for the server tests** (not blocking; CI runs them): enable *Docker Desktop → Settings → Resources → WSL integration → Ubuntu*, or install PostgreSQL in WSL (`sudo apt install postgresql`). Then `DATABASE_URL=postgres://… cargo nextest run -p dibill-server` runs them here too.
- **App name, `applicationId` and icon** (needed before M8): keeping DiBillSion and the current icon for now (owner, 2026-10-30); tell me when you have the final ones.
- **UI direction:** the redesign is original (clean-room rule). Point out on the phone what you'd like different and I'll iterate.
- **M4 (iOS)** needs a Mac (Xcode); start it when you have one available.
- **Hosting for the M5 server** (later in M5, not blocking yet): local Docker is enough until the two-device test; a public host costs money and needs your account.

