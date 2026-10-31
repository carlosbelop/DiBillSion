# Project status

> Living document. Every session reads it first and updates it in the same PR as the work it describes.
> Milestones are defined in [PLAN.md §4](PLAN.md).

Last updated: 2026-10-31 (session 5)

## Where we are

**M0 (tooling): done. M1 (functional spec): done, SPEC v2 approved. M2 (domain): done.** **M3 (local Android app): in progress** — store, UniFFI facade, SDK/NDK and Compose shell done; first on-device run done (Pixel 9a); only branding (owner) is left. **M5 (backend + sync): in progress** — sync model (ADR 0004), store op log (ADR 0005) and sync API (ADR 0006), server, invites, core sync client and Android share/join done; only a two-real-device check is left.

### Done
- WSL2 Ubuntu 24.04 environment (`scripts/setup-ubuntu.sh`): Rust stable + Android targets, cargo-nextest/deny/llvm-cov/ndk/bloat, Node 24 (nvm), OpenJDK 21, adb, gh.
- Repo scaffold: Cargo workspace, strict lints, CI (fmt, clippy, nextest + coverage, cargo-deny, no-binaries/secrets guard), Dependabot, ADR 0001.
- `dibill-domain`: `split_weighted` (largest remainder, deterministic), `balances`, `settle_up` (greedy), with unit and property tests.
- `dibill-domain`: ISO 4217 `Currency` + decimal FX (`convert`, `convert_items`, ADR 0002), line-item expenses, transfers in balances, criterion baseline ([BENCHMARKS.md](BENCHMARKS.md)).

## Next sessions (from 2026-10-31), in order — see PLAN §4.2

1. ~~**P0 public-repo prep**~~ done (current files cleaned, history kept; issue/PR texts edited). The owner switches the repo to public.
2. **M4 iOS → TestFlight**: app and CI done (#72, #74); TestFlight uploads work (#78), testers pending (see *Needs the owner*). Then simulator screenshots in CI and parity items.
3. **B1 Pi** (weekend, when the owner turns the Pi on): 64-bit OS + Docker, `deploy/deploy.sh`, Tailscale Funnel URL → release `SYNC_URL`; then B2 Android beta build.
4. **M7 receipts** (started 2026-10-31 while the Pi waits): eval set + harness (#82), then server endpoint + core, Android and iOS scan screens. Model choice after the eval run (owner: key).

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
- [x] ADR 0008: any OpenAI-compatible provider (base URL + model + key), strict JSON Schema output (or `json_object`/prompt fallback), amounts as decimal strings parsed without floats, one repair retry when the items don't add up. ([#82](https://github.com/carlosbelop/DiBillSion/pull/82))
- [x] `dibill-receipt`: schema (tested against the Rust types), `parse_amount`, `check`, `normalize` (discounts folded into items, never negative, sum preserved: property test), the HTTP client (feature `http`) tested against a fake provider (same PR).
- [x] Evaluation set started: `eval/receipts/` with 5 receipts from the owner (ES supermarkets, ES/FR restaurants) and our ground truth; photos are in the repo (owner's decision, 2026-10-31). A test checks every ground truth parses and adds up. Grows as the owner adds photos (same PR).
- [x] `receipt-eval` harness: many models in one run, parse rate, exact totals, exact item lists, recall/precision, label similarity, merchant/date, retries, latency p50/p95, tokens, cost (reported by the provider or from `--model name@in,out` prices), markdown report (same PR).
- [ ] Run candidates (owner: provider choice and a key) and pick the cheapest model over the bar. More receipts first (aim: 30+).

### M7b: scanning in the app
- [x] Server: `POST /v1/receipts/scan` (base64 image → configured model → validated answer with one repair retry), provider key only on the server (`RECEIPT_*` env, off without it), per-device daily limit and global monthly cap counted in `receipt_scans` (no image stored), new error codes `receipts_disabled`, `limit_reached`, `unreadable`, `provider_failed`. Core `scan_receipt` normalizes into minor units of the group's currency. Deploy kit passes the settings ([deploy/README §4](../deploy/README.md)). ([#84](https://github.com/carlosbelop/DiBillSion/pull/84))
- [x] Android: scan button on the Expenses tab (camera through a `FileProvider` URI, no camera permission; or the photo picker), downscale to 1600 px JPEG on the phone, reading screen, review screen (title from the merchant, payer, date, currency + rate, editable/addable/deletable items with folded discounts shown, people per item with "everyone" by default and one tap to pick a single person, check against the printed total, per-person totals from the core, scans left), saved as one multi-item expense; errors per server code (en/es). Transport timeout now comes from the core per request (15 s sync, 150 s scan). Builds and passes JVM tests (`ReceiptDraftTest`); **not yet tried on the phone** (it was locked). ([#86](https://github.com/carlosbelop/DiBillSion/pull/86))

### M4: iOS (no Mac needed for building)
- [x] XCFramework + Swift bindings (`scripts/build-xcframework.sh`), SwiftUI app for the flat use case (XcodeGen, iOS 17+): groups, people, add expense with an equal split and live exact shares, balances with "you owe / you are owed" and settle, invite (share sheet) and join (link, code or `dibillsion://join/<code>`), who am I, pull-to-refresh and background sync over `URLSession`, English + Spanish. macOS CI workflow (`ios.yml`): build + unit tests on the simulator; while the repo is private it only runs with the `macos` PR label or manual dispatch. ([#72](https://github.com/carlosbelop/DiBillSion/pull/72))
- [ ] Simulator screenshots as CI artifacts; remaining parity with Android (edit expenses, income, unequal splits, foreign-currency polish).
- [x] Signing + TestFlight upload from CI (`testflight.yml`, automatic signing with the App Store Connect API key, internal testing only; unsigned device archive on PRs). ([#74](https://github.com/carlosbelop/DiBillSion/pull/74))
- [x] First TestFlight upload (2026-10-31, build 13; no sync server yet). Signing: each run makes a throwaway Apple Distribution certificate + App Store profile through the API (`scripts/asc.py`), signs manually, uploads, then revokes/deletes them; Xcode 26 runners (`macos-26`), required by App Store Connect. ([#78](https://github.com/carlosbelop/DiBillSion/pull/78))
- [ ] Flatmates installed it from TestFlight (owner: add them as internal testers).

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
Without Docker (WSL without Docker Desktop): PostgreSQL 16 binaries unpacked from the Ubuntu packages
(`apt-get download postgresql-16 postgresql-client-16 libpq5`, `dpkg -x` into
`~/.local/share/dibillsion-tools/pg/root`), cluster in `~/.local/share/dibillsion-tools/pg/data` (same
port and password): `LD_LIBRARY_PATH=$P/root/usr/lib/x86_64-linux-gnu $P/root/usr/lib/postgresql/16/bin/pg_ctl -D $P/data -o "-p 55432 -k /tmp -c listen_addresses=127.0.0.1" -l $P/log start`.
The dev password is local-only; never reuse it for a hosted database.

## Known issues and tech debt
- Remote Control (and mobile push) is disabled by the owner's organization policy, so escalations go in the session's final message and in this file.
- WSL appends the Windows `PATH`, which includes Windows Node. Always make sure nvm's Node comes first (`source ~/.nvm/nvm.sh`).
- Since the repo is public (2026-10-31) the `Protect main` ruleset is enforced: PR only, squash only, required checks (Rust, Dependencies, No binaries / secrets), verified signatures, linear history.

## Needs the owner (open requests)
- **TestFlight testers**: add the flatmates as internal testers ([ios/README.md § TestFlight](../ios/README.md#testflight), step 5). Apple setup and secrets are done; the first build is uploaded.
- **Raspberry Pi** this weekend: say when it's on; 64-bit Raspberry Pi OS, SSH key, Docker, Tailscale.
- **Beta hosting**: decided 2026-10-30, Raspberry Pi 4B with Tailscale Funnel (an SSD instead of the SD card if possible). A VPS (Hetzner CAX11 ≈€4.5–5/month) stays the fallback.
- **Domain / name**: autosplit.com is registered (since 2003, renewed to 2027; no website). Free alternatives checked on 2026-10-30: autosplit.app, getautosplit.com, autosplitapp.com, dibillsion.com, dibillsion.app. Using "AutoSplit" would also be a rename (branding), and the name is used by an existing open-source speedrunning tool: worth a trademark check.
- **VLM provider + key** (blocks the eval run and scanning in production, not the code): recommended an [OpenRouter](https://openrouter.ai) key with ~5 € credit, so one key runs Gemini, GPT, Claude, Qwen and Mistral models in one eval and the winner can stay on OpenRouter or move to its vendor. More receipt photos for `eval/receipts/` (see its README).
- **Android beta distribution**: APK sideload (free) or Play internal testing ($25 one-time account); a release signing key kept by you.
- **App name, `applicationId` and icon** (needed before M8): keeping DiBillSion and the current icon for now (owner, 2026-10-30); tell me when you have the final ones.
- **UI direction:** the redesign is original (hard rule 1). Point out on the phone what you'd like different and I'll iterate.
- **Second device (optional)**: an Android emulator (Android Studio on Windows) or a second phone with USB/wireless debugging, for the two-real-device check.

