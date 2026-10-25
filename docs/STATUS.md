# Project status

> Living document. Every session reads it first and updates it in the same PR as the work it describes.
> Milestones are defined in [PLAN.md §4](PLAN.md).

Last updated: 2026-10-25

## Where we are

**M0 (tooling): done.** Starting **M1 (study)** and **M2 (domain)** in parallel. M2 doesn't depend on the study.

### Done
- WSL2 Ubuntu 24.04 environment (`scripts/setup-ubuntu.sh`): Rust stable + Android targets, cargo-nextest/deny/llvm-cov/ndk/bloat, Node 24 (nvm), OpenJDK 21, jadx-headless-mcp 0.7.1, Apktool 3.0.3, adb, gh.
- REA registered as MCP server `rea` for Claude Code (`npx -y rea-agents@6.8.0 mcp`).
- Repo scaffold: Cargo workspace, strict lints, CI (fmt, clippy, nextest + coverage, cargo-deny, study-material guard), Dependabot, ADR 0001.
- `dibill-domain`: `split_weighted` (largest remainder, deterministic), `balances`, `settle_up` (greedy), with unit and property tests.
- Study input: `_study/apk/tricount-base.apk`. Package `com.tribab.tricount.android`, v14.5.0 (versionCode 500004150), minSdk 32, targetSdk 37, single APK, 296 MB, sha256 `19d9dbb08353708d4f698752011c334fbdc669cef2f4b9918e3701ce62336edb`.

## Backlog (in order)

Mark items `[x]` when merged and link the PR.

### M1: Study (clean room, notes stay in `_study/`)
- [ ] **Triage** (PLAN §2.1): `inspect_android_package`, `identify_flutter_build` on the APK. Record stack (Kotlin/Compose? Flutter? RN?), permissions, deep links / invite links, third-party SDKs, obfuscation level → `_study/notes/01-triage.md`.
- [ ] **Resources and strings** (PLAN §2.2): `decode_android_resources` (en, es) → feature inventory in our own words → `_study/notes/02-features.md`.
- [ ] **Behavior** (PLAN §2.3): answer the 7 questions (data model, split modes, rounding, multi-currency, settle-up, offline/sync and invites, export) → `_study/notes/03-behavior.md`.
- [ ] **Black-box observation** (PLAN §2.4) on the phone. ⚠️ Use only a throwaway test group. Never touch the owner's real groups. If something can't be done through adb, ask the owner.
- [ ] **Write `docs/SPEC.md`** (own words; the only study output allowed in the repo). PR it.

### M2: Domain (`dibill-domain`, no study needed)
- [x] Currency type (ISO 4217 code + minor-unit exponent) and FX conversion with `rust_decimal`, with explicit rounding rules and tests. ([#11](https://github.com/carlosbelop/DiBillSion/pull/11), ADR 0002)
- [ ] Line-item model: `ExpenseItem` + `ItemShare`. A manual expense is one item; receipts are many. Balances computed per item.
- [ ] Transfers (reimbursements) included in balances.
- [ ] `criterion` benchmarks for split, balances and settle-up (1k members, 100k expenses).
- [ ] Refine `docs/SPEC.md` calculation rules with worked examples from the tests.

### M3: Local-only Android app
- [ ] `dibill-store`: SQLite (rusqlite bundled + migrations), WAL, repository API.
- [ ] `dibill-core`: UniFFI facade (coarse-grained, async), Kotlin bindings generation.
- [ ] Android SDK + NDK r27+ in WSL via command-line tools. ⚠️ Accepting the Android SDK licenses (`sdkmanager --licenses`) is accepting terms: ask the owner.
- [ ] Compose shell: groups, members, expenses, balances, settle-up. Offline only.
- [ ] CI job: `cargo ndk` build + Gradle assemble.

### Later
M4 iOS (needs macOS) · M5 backend + sync + invites · M6 polish · M7 receipt scanning (VLM) · M8 store release. See PLAN §4.

## Known issues and tech debt
- REA env vars live in `~/.bashrc` (skipped by non-interactive shells) and in `~/.claude/settings.json`. If a non-Claude script needs them, also export them from `~/.profile` and update `scripts/setup-ubuntu.sh`.
- WSL appends the Windows `PATH`, which includes Windows Node. Always make sure nvm's Node comes first (`source ~/.nvm/nvm.sh`).
- `main` isn't protected by GitHub (free plan, private repo). The PR workflow is self-enforced, and `.claude/settings.json` denies direct pushes to `main`.

## Needs the owner (open requests)
_None right now._
