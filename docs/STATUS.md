# Project status

> Living document. Every session reads it first and updates it in the same PR as the work it describes.
> Milestones are defined in [PLAN.md §4](PLAN.md).

Last updated: 2026-10-31 (session 4)

## Where we are

**M0 (tooling): done. M1 (functional spec): done, SPEC v2 approved. M2 (domain): done.** **M3 (local Android app): in progress** — store, UniFFI facade, SDK/NDK and Compose shell done; first on-device run done (Pixel 9a); only branding (owner) is left. **M5 (backend + sync): in progress** — sync model (ADR 0004), store op log (ADR 0005) and sync API (ADR 0006), server, invites, core sync client and Android share/join done; only a two-real-device check is left.

### Done
- WSL2 Ubuntu 24.04 environment (`scripts/setup-ubuntu.sh`): Rust stable + Android targets, cargo-nextest/deny/llvm-cov/ndk/bloat, Node 24 (nvm), OpenJDK 21, adb, gh.
- Repo scaffold: Cargo workspace, strict lints, CI (fmt, clippy, nextest + coverage, cargo-deny, no-binaries/secrets guard), Dependabot, ADR 0001.
- `dibill-domain`: `split_weighted` (largest remainder, deterministic), `balances`, `settle_up` (greedy), with unit and property tests.
- `dibill-domain`: ISO 4217 `Currency` + decimal FX (`convert`, `convert_items`, ADR 0002), line-item expenses, transfers in balances, criterion baseline ([BENCHMARKS.md](BENCHMARKS.md)).

## Next sessions (from 2026-10-31), in order — see PLAN §4.2

1. ~~**P0 public-repo prep**~~ done (current files cleaned, history kept; issue/PR texts edited). The owner switches the repo to public.
2. **M4 iOS → TestFlight** (several PRs): macOS CI job (XCFramework + UniFFI Swift bindings), SwiftUI app for the flat use case, signing/upload with an App Store Connect API key from GitHub secrets, internal testers.
3. **B1 Pi** (weekend, when the owner turns the Pi on): 64-bit OS + Docker, `deploy/deploy.sh`, Tailscale Funnel URL → release `SYNC_URL`; then B2 Android beta build.
4. M7a receipt evaluation.

## Backlog (in order)

Mark items `[x]` when merged and link the PR.

### M1: Functional spec
- [x] **Write `docs/SPEC.md`**: features, flows, data model, calculation rules with worked examples. v2 approved by the owner on 2026-10-25 (M1 exit criterion met).

### M2: Domain (`dibill-domain`)
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
- [x] UI redesign with our own visual identity (teal/amber palette, avatars, grouped expense list, balance bars, full-screen editors, live exact shares from the core via `preview_split`). Designed from the SPEC flows. ([#50](https://github.com/carlosbelop/DiBillSion/pull/50))
- [x] Owner feedback on the redesign: "who am I" per group (device-local, schema v3 `group_prefs`, not synced), my balance and my payments at the top of Balances and on the group cards, payer defaults to me; swipeable tabs, pull-to-refresh, list and screen animations, an elastic bottom edge that works on short lists; centered title; a tint per group. ([#52](https://github.com/carlosbelop/DiBillSion/pull/52))

### M5: Backend + sync + invite links
Exit criterion: two devices converge after offline edits. Design: [ADR 0004](adr/0004-sync-model.md), [ADR 0005](adr/0005-store-op-log.md), [ADR 0006](adr/0006-sync-api.md).
- [x] Sync model: `dibill-sync` (op ids, Lamport clock, LWW registers per field, permanent tombstones, in-memory reference replica, convergence property tests) + ADR 0004. ([#46](https://github.com/carlosbelop/DiBillSion/pull/46))
- [x] Op log in `dibill-store` (schema v2, [ADR 0005](adr/0005-store-op-log.md)): every write records ops and rows are rebuilt from them; remote ops in any order; pending/pushed bookkeeping; backfill of existing installs. Property test: three stores converge and match the reference replica. ([#48](https://github.com/carlosbelop/DiBillSion/pull/48))
- [x] Sync API and wire format ([ADR 0006](adr/0006-sync-api.md)): JSON over HTTPS under `/v1`, anonymous device tokens, per-group sequence cursor for pull, envelope-only checks on the server, invite codes. Wire types in `dibill-proto`, conversions from the store's `SyncOp`. ([#54](https://github.com/carlosbelop/DiBillSion/pull/54))
- [x] `dibill-server` skeleton: axum + PostgreSQL (`tokio-postgres` + `deadpool`, embedded migrations; `sqlx` clashes with `rusqlite`'s SQLite, see ADR 0006 addendum), `/health`, op push/pull relay with per-group sequence, JSON errors; integration tests against a PostgreSQL 17 service in CI (skipped locally without `DATABASE_URL`). ([#56](https://github.com/carlosbelop/DiBillSion/pull/56))
- [x] Auth: anonymous device tokens (`POST /v1/devices`, SHA-256 hash stored, bearer auth), group membership checks (same PR). Sign-in comes later.
- [x] Invite links on the server: owner gets/turns off/regenerates the group's code, `POST /v1/invites/{code}/join`; claiming a member is the device-local "who am I". End-to-end test: two real stores share a group through the server, edit offline on both sides and converge. ([#58](https://github.com/carlosbelop/DiBillSion/pull/58))
- [x] Sync client in `dibill-core` ([ADR 0007](adr/0007-sync-client-transport.md)): protocol in Rust, raw HTTP through an `HttpTransport` callback the app implements (system TLS); `share_group`, `invite_code`, `turn_off_invite`, `join_group`, `sync`; store schema v4 (token, per-group cursor); `Offline`/`Server` errors. Tested against an in-memory fake server. ([#60](https://github.com/carlosbelop/DiBillSion/pull/60))
- [x] Android: `HttpURLConnection` transport, server URL per build type (debug `http://127.0.0.1:8080` via `adb reverse`, cleartext only to localhost in debug; release empty until hosting), "Invitar" (shares the group, then the share sheet with the link and code), turn the link off, "Unirse" with a pasted link/code or a `dibillsion://join/<code>` link, sync on open, pull-to-refresh and after every edit (background, quiet when offline), "Compartido" on cards and header. Checked on the Pixel against a local server, with a second device simulated over HTTP. ([#62](https://github.com/carlosbelop/DiBillSion/pull/62))
- [ ] Two real devices converging (Pixel + emulator or a second phone) against the local server. The phone ↔ server ↔ second device path already works with the second device simulated over HTTP (PR #62), and two real stores converge through the real server in CI (PR #58).

### Friends beta (PLAN §4.1: B1, B2)
- [x] Deploy kit ([deploy/](../deploy/README.md)): cross-compiled amd64/arm64 image (distroless, non-root), `compose.yaml` with PostgreSQL 17 + nightly `pg_dump` (14 days), `deploy.sh` (build here, ship over SSH, wait for `/health`), HTTPS via Tailscale Funnel or Cloudflare Tunnel, restore guide; server stops cleanly on SIGTERM; CI builds and boots the image. ([#66](https://github.com/carlosbelop/DiBillSion/pull/66))
- [ ] Server hardening for the public internet: rate limits (device registration per IP, pushes per device), request logging without bodies, CORS off, metrics/health.
- [ ] Deploy to the beta host (owner: Raspberry Pi set up or VPS account) and point a release build at it.
- [ ] Android beta build: release signing with an owner-held key (never in git), `SYNC_URL` = beta server, versioning, distribution (owner: APK sideload or Play internal testing).

### M7a: VLM receipt evaluation (starts in parallel)
- [ ] ADR 0008: structured output approach (provider JSON-Schema modes, schema from Rust types, semantic validation and one repair retry in Rust).
- [ ] Receipt schema (`dibill-receipt`): merchant, date, currency, items (label, qty, unit, line total, discounts), tax, tip, total; JSON Schema generated from the Rust types; validation (items + adjustments = total, to the cent).
- [ ] Evaluation set: ~100 receipts. CORD v2 test split (CC BY 4.0) plus Spanish receipts from the owner and friends (personal data removed). Ground truth written by us, checked by the owner on a sample. Images stay out of git; ground truth and a download script are versioned.
- [ ] Rust harness: OpenAI-compatible client (OpenRouter for breadth) + native Gemini; per-model accuracy (total exact, items exact, schema-valid rate), cost per receipt, latency p50/p95; markdown report.
- [ ] Run candidates (owner: API keys / a few euros of credit) and pick the cheapest model over the bar.

### M7b: scanning in the app
- [ ] Server: `POST /v1/receipts` (downscaled image → model → validated items), provider key only on the server, per-device credits and daily limits, global monthly budget cap.
- [ ] Android: camera/gallery, downscale (~1600 px JPEG), progress, editable item list, assign people per item, save as one multi-item expense.

### M4: iOS (no Mac needed for building)
- [x] XCFramework + Swift bindings (`scripts/build-xcframework.sh`), SwiftUI app for the flat use case (XcodeGen, iOS 17+): groups, people, add expense with an equal split and live exact shares, balances with "you owe / you are owed" and settle, invite (share sheet) and join (link, code or `dibillsion://join/<code>`), who am I, pull-to-refresh and background sync over `URLSession`, English + Spanish. macOS CI workflow (`ios.yml`): build + unit tests on the simulator; while the repo is private it only runs with the `macos` PR label or manual dispatch. ([#72](https://github.com/carlosbelop/DiBillSion/pull/72))
- [ ] Simulator screenshots as CI artifacts; remaining parity with Android (edit expenses, income, unequal splits, foreign-currency polish).
- [ ] TestFlight (owner: Apple Developer Program, $99/year).

### Later
M6 polish · M8 store release. See PLAN §4.

## Dev server (local)

```bash
docker run -d --name dibillsion-pg -e POSTGRES_PASSWORD=dibill -p 127.0.0.1:55432:5432 postgres:17
docker exec dibillsion-pg psql -U postgres -c "CREATE DATABASE dibill"
DATABASE_URL=postgres://postgres:dibill@127.0.0.1:55432/dibill cargo run -p dibill-server
adb reverse tcp:8080 tcp:8080   # the debug app talks to http://127.0.0.1:8080
```
Server tests locally: `DATABASE_URL=postgres://postgres:dibill@127.0.0.1:55432/postgres cargo nextest run -p dibill-server`.
The dev password is local-only; never reuse it for a hosted database.

## Known issues and tech debt
- Remote Control (and mobile push) is disabled by the owner's organization policy, so escalations go in the session's final message and in this file.
- WSL appends the Windows `PATH`, which includes Windows Node. Always make sure nvm's Node comes first (`source ~/.nvm/nvm.sh`).
- `main` isn't protected by GitHub (free plan, private repo). The PR workflow is self-enforced, and `.claude/settings.json` denies direct pushes to `main`.

## Needs the owner (open requests)
- **Apple Developer Program**: enrolling 2026-10-31 (individual). Then create an App Store Connect API key (Issuer ID, Key ID, `.p8`) with App Manager access and add it as GitHub Actions secrets; send the flatmates' Apple ID emails for internal TestFlight testing.
- **Make the repo public** after the P0 cleanup PR is merged (Settings → General → Danger zone → Change visibility).
- **Raspberry Pi** this weekend: say when it's on; 64-bit Raspberry Pi OS, SSH key, Docker, Tailscale.
- **Beta hosting**: decided 2026-10-30, Raspberry Pi 4B with Tailscale Funnel (an SSD instead of the SD card if possible). A VPS (Hetzner CAX11 ≈€4.5–5/month) stays the fallback.
- **Domain / name**: autosplit.com is registered (since 2003, renewed to 2027; no website). Free alternatives checked on 2026-10-30: autosplit.app, getautosplit.com, autosplitapp.com, dibillsion.com, dibillsion.app. Using "AutoSplit" would also be a rename (branding), and the name is used by an existing open-source speedrunning tool: worth a trademark check.
- **VLM API access** (blocks M7a runs, not the harness): keys for OpenRouter and/or Google AI Studio / OpenAI with a few euros of credit; Spanish receipt photos for the evaluation set.
- **Android beta distribution**: APK sideload (free) or Play internal testing ($25 one-time account); a release signing key kept by you.
- **App name, `applicationId` and icon** (needed before M8): keeping DiBillSion and the current icon for now (owner, 2026-10-30); tell me when you have the final ones.
- **UI direction:** the redesign is original (hard rule 1). Point out on the phone what you'd like different and I'll iterate.
- **Second device (optional)**: an Android emulator (Android Studio on Windows) or a second phone with USB/wireless debugging, for the two-real-device check.

