# DiBillSion — Plan: study Tricount with REA, then build a clean Rust-based expense splitter

> Status: M0–M2 done, M3 in progress. Live status and backlog: [STATUS.md](STATUS.md).
> Target: Android + iOS store release, with receipt scanning (VLM) added after the MVP.

---

## 0. Ground rules (read first)

Studying how Tricount works is fine. Copying it is not. Its APK is bunq's copyrighted code, and "Tricount" is their trademark. Under EU law (Directive 2009/24/EC art. 6; in Spain, LPI art. 100), you may decompile only for interoperability. You may **not** use what you learn from decompiling to build a "substantially similar" program. Both Google Play (impersonation/IP policy) and Apple (App Review Guidelines 4.1 *Copycats* and 5.2 *Intellectual Property*) reject clones.

So the project uses a **clean-room** split:

| Track | Who / what | Output | Must NOT carry over |
|---|---|---|---|
| **A. Study** | REA + Claude + you using the app | A *functional spec*: features, flows, rules, edge cases, written in your own words | Code, smali, class names, strings, icons, images, colors, layouts, private API endpoints/keys |
| **B. Build** | Fresh code, written only from the spec | DiBillSion app + your own backend | Nothing from the decompiled APK |

Practical consequences:
- Keep decompiler output in a separate folder (`_study/`, git-ignored). Never paste it into `src/`.
- Don't call Tricount's backend. Build your own.
- Pick your own name, icon, palette, and copy. "DiBillSion" works.
- Get the APK only from your own device (`adb pull`), not from mirror sites. Those are untrusted and may be tampered with.
- This isn't legal advice. Spend an hour with an IP lawyer before you publish.

The reusable parts are generic anyway: group expenses, unequal shares, multiple currencies, debt simplification. These are common ideas, and well-known algorithms exist for them. The new value is your **receipt-to-line-items** feature.

---

## 1. Development environment: WSL2 (Ubuntu 24.04) on Windows 11

Everything except the Android emulator, Android Studio and iOS runs **inside WSL2**. The same setup works unchanged on a native Ubuntu machine and in GitHub Actions (Linux runners).

| Where | What |
|---|---|
| **WSL2 Ubuntu** (`~/proyectos/dibillsion`) | Git, Rust, REA + JADX + Apktool, adb, backend (Docker), Claude Code CLI |
| **Windows** | VS Code (with the *WSL* extension), Android Studio + emulator, Docker Desktop (WSL backend), USB passthrough (`usbipd-win`) |
| **macOS** (Mac or CI runner) | iOS build, signing, TestFlight |

Rules:
- Keep the repo in the Linux filesystem (`~/...`), never under `/mnt/c` (much slower, and permission/line-ending problems).
- Line endings are always LF (`.gitattributes`). Don't call Windows `.exe` files from project scripts.

### 1.1 One-time setup

```bash
# Windows (PowerShell, once): keep WSL up to date
wsl --update

# Ubuntu: clone (or cd) and run the reproducible setup script
cd ~/proyectos/dibillsion
./scripts/setup-ubuntu.sh          # asks for sudo once (apt)
source ~/.bashrc
npx rea-agents setup               # registers REA as an MCP server for Claude Code
```

`scripts/setup-ubuntu.sh` is the single source of truth for versions. It's idempotent and works on WSL, native Ubuntu and CI.

### 1.2 Study track tools (installed by the script)

| Tool | Version | Why | Location / variable |
|---|---|---|---|
| OpenJDK (full JDK, `javac`) | 21 (apt `openjdk-21-jdk-headless`) | JADX bridge needs `jdk.compiler`. A JRE alone is not enough | system |
| Node.js | 24.x (via nvm) | Runs REA (CLI + MCP) and its SQLite support | `~/.nvm` |
| REA (`rea-agents`) | npm 4.1.0+ | MCP server + `rea` CLI | `npx rea-agents setup` |
| jadx-headless-mcp | **0.7.1**, SHA-256 verified | Engine REA wraps for decompiling | `REA_JADX_MCP_JAR` |
| Apktool | **3.0.3** (avoid < 2.9.3 and 3.0.0–3.0.1, known CVEs) | Resources + string tables | `REA_APKTOOL_COMMAND` (`~/.local/bin/apktool`) |
| Android platform-tools | latest | `adb`: pull APK, logcat, dumpsys | `~/.local/bin/adb` |
| *(optional)* JADX GUI | 1.5.x | Browse code by hand (runs via WSLg) | manual |

REA tuning (set in `~/.bashrc` by the script): `REA_JADX_OPERATION_TIMEOUT_MS=600000`, `REA_JADX_HEAP_MIB=4096`.
Raise the WSL memory limit if JADX runs out of heap: `%UserProfile%\.wslconfig` → `[wsl2]` `memory=12GB`.

Not needed: Ghidra, IDA, Hopper, pwntools, Rizin, tshark (they're for native binaries). Tricount is very likely a native Kotlin app. Step 2.1 confirms it.

### 1.3 Connecting the phone (USB) to WSL

WSL doesn't see USB devices by default. Pick one:

- **A. usbipd-win (recommended).**
  - Windows, once: `winget install usbipd`.
  - Each session, from an **administrator** PowerShell: `usbipd list`, then `usbipd bind --busid <id>` (only the first time), then `usbipd attach --wsl --busid <id>`.
  - Then `adb devices` works inside Ubuntu.
- **B. Wireless debugging.** Android 11+: *Developer options → Wireless debugging*. Then in Ubuntu run `adb pair <ip>:<port>` followed by `adb connect <ip>:<port>`. No USB needed.

### 1.4 Build track tools

| Area | Tool | Where | Notes |
|---|---|---|---|
| Rust | rustup stable + rustfmt + clippy, Android targets | WSL (script) | Version pinned by `rust-toolchain.toml` |
| Cargo tools | cargo-nextest, cargo-deny, cargo-llvm-cov, cargo-ndk, cargo-bloat | WSL (script, via cargo-binstall) | |
| Bindings | UniFFI (`uniffi` crate + `uniffi-bindgen`) | WSL | Added in M3 |
| Android SDK + **NDK r27+** | `sdkmanager` (command-line tools) in WSL for `cargo ndk`. Android Studio on Windows for UI and emulator | both | `ANDROID_NDK_HOME` in WSL (M3) |
| Android UI | Android Studio (Windows) opening `\\wsl.localhost\Ubuntu\home\<user>\proyectos\dibillsion\android` | Windows | Or build with `./gradlew` inside WSL and install with `adb` |
| Backend | Docker Desktop with the WSL2 backend, PostgreSQL 17 container, `sqlx-cli` | WSL | Enable *Settings → Resources → WSL integration → Ubuntu* |
| iOS | Xcode 16+ on macOS, targets `aarch64-apple-ios aarch64-apple-ios-sim x86_64-apple-ios`, XCFramework | Mac / CI | Can't be done on Windows or Linux |
| CI | GitHub Actions: Linux for Rust/Android, macOS for iOS | GitHub | `.github/workflows/ci.yml` |

### 1.5 Editor and GitHub

- **VS Code:** install the *WSL* extension on Windows (and *rust-analyzer*, *Even Better TOML* inside WSL). Open the project with `code ~/proyectos/dibillsion` from Ubuntu.
- **GitHub (personal account):** SSH key generated **inside WSL** (`ssh-keygen -t ed25519`), added to the personal GitHub account. Commit email = the account's `noreply` address. Repo is **private**. GitHub does not enforce branch protection on private repos on the free plan, so the PR + green CI workflow is self-enforced (see CLAUDE.md).

---

## 2. Study track: steps with REA

Working folder: `_study/` (git-ignored). All of it is notes for the spec, not source code.

### 2.0 Get the APK from your own phone
In Ubuntu (WSL), with the phone connected as in 1.3:
```bash
adb devices
adb shell pm list packages | grep -i tricount        # confirmed: com.tribab.tricount.android (v14.5.0, single APK)
adb shell pm path com.tribab.tricount.android                # may list base.apk + split_config.*.apk
adb pull <path-to-base.apk> _study/apk/tricount-base.apk
```
REA doesn't handle split APKs or AABs, so analyze `base.apk`. Pull the `split_config.<lang>.apk` / density splits only if resources seem missing.

### 2.1 Triage: what is it built with?
```
rea inspect-android-package _study/apk/tricount-base.apk        # manifest, permissions, SDK, components, counts
rea identify-flutter-build  _study/apk/tricount-base.apk        # rules Flutter in or out (libapp.so/libflutter.so)
```
Also check for `assets/index.android.bundle` / `libhermes.so` (React Native) and `kotlin/` metadata (Kotlin).
Record: minSdk/targetSdk, permissions (camera? contacts?), exported activities, **deep-link intent filters** (how group invite links work), whether the code is R8-obfuscated, and third-party SDKs (analytics, crash reporting, billing).

### 2.2 Resources and strings: the feature inventory
```
rea inspect-apktool-client
rea decode-android-resources _study/apk/tricount-base.apk --locales en --locales es --string-tables-only
```
String tables list nearly every screen, label, error, and setting, so they make the best feature checklist. Turn them into a feature list **in your own words**. Don't copy the strings.

### 2.3 Behavior: how each feature works
Ask Claude (with the REA MCP loaded) targeted questions, for example:
```
rea search-android-classes  _study/apk/tricount-base.apk  Balance
rea search-android-classes  _study/apk/tricount-base.apk  Expense
rea inspect-android-class   _study/apk/tricount-base.apk  <class>
rea inspect-android-method  _study/apk/tricount-base.apk  <class> <method>
rea trace-android-references _study/apk/tricount-base.apk <class> --method-name <m>
rea project-android-application-graph '{"inventory_evidence":[...]}'
```
Questions to answer (behavior only):
1. **Data model.** Which entities exist (group, member, expense, share, transfer/reimbursement, currency, attachment, category)? Which fields are optional?
2. **Split modes.** Equal, by amount, by shares/weights, by percentage? Can a member be excluded per expense?
3. **Rounding.** Where does the leftover cent go when 10.00 is split 3 ways?
4. **Multi-currency.** Is the rate fixed per expense or live? Where do rates come from? What is the group's base currency?
5. **Balances and settle-up.** How are suggested reimbursements computed (min-transactions heuristic?), and is it deterministic?
6. **Offline and sync.** Local DB (Room?), conflict handling, and how invite links / joining without an account work.
7. **Export and extras.** CSV/PDF export, photo attachments, categories, statistics, notifications.

### 2.4 Black-box observation (runtime, your own device)
- Use the app through every flow and screenshot each one. This becomes the UX reference for **your own** design, not a template.
- `adb logcat`, `adb shell dumpsys activity activities` show which screens exist and how navigation works.
- Try edge cases: 0.01 amounts, 1-person groups, deleting a member who has expenses, mixed currencies, editing an old expense, and making changes on two phones at once.
- Don't attack pinning or intercept traffic on their backend. You don't need it, because you'll design your own API.

### 2.5 Deliverable: `docs/SPEC.md` (the only thing that crosses to Track B)
Features, user stories, data model (your own names), calculation rules with worked examples, edge cases, and non-goals. From this point on, the build team (you + Claude) works only from SPEC.md.

---

## 3. Build track: architecture

### 3.1 Why Rust, and where

Rust is a top-tier choice for speed, memory, and correctness. In an expense app, though, what users notice as speed comes from the **UI layer, startup time, and the local database**, not CPU math. So use Rust where it helps most:

**Recommended: shared Rust core + native UI on each platform**
```
┌──────────── Android ─────────────┐   ┌───────────── iOS ──────────────┐
│ Jetpack Compose (Kotlin)          │   │ SwiftUI                        │
│   └─ UniFFI Kotlin bindings       │   │   └─ UniFFI Swift bindings     │
└───────────────┬───────────────────┘   └───────────────┬────────────────┘
                └──────────── dibill-core (Rust) ────────┘
     domain model · split engine · balances/settle-up · FX · SQLite store
     sync engine (op-log) · validation · receipt-item allocation
                              │  HTTPS (protobuf/JSON)
                    dibill-server (Rust: axum + PostgreSQL)
                    auth · group sync · invite links · receipt→VLM proxy
```
- One codebase for all the logic: money math, sync, and storage are written and tested once.
- Native UI gives the best startup time, scrolling, accessibility, and camera integration.
- The trade-off is two thin UI layers. If you'd rather have one UI codebase, swap the shells for **Flutter + `flutter_rust_bridge`** and keep the same core. Pure-Rust mobile UI (Dioxus, Slint, Tauri mobile) isn't mature enough yet for a polished store app.

### 3.2 Cargo workspace
```
dibillsion/
├─ Cargo.toml                 (workspace)
├─ crates/
│  ├─ dibill-domain/          pure types + split/settle algorithms (no I/O, no deps beyond serde)
│  ├─ dibill-store/           SQLite persistence (rusqlite, migrations)
│  ├─ dibill-sync/            op-log, conflict resolution, HTTP client
│  ├─ dibill-core/            façade exposed via UniFFI to Kotlin/Swift
│  ├─ dibill-proto/           wire types (prost) shared by app + server
│  └─ dibill-server/          axum backend
├─ android/                   Compose app (Gradle), loads libdibill_core.so
├─ ios/                       SwiftUI app (Xcode), links DibillCore.xcframework
├─ docs/SPEC.md
└─ _study/                    (git-ignored) REA outputs, screenshots, notes
```

### 3.3 Crates
| Crate | Use |
|---|---|
| `serde`, `serde_json` | serialization |
| `uuid` (v7) | time-ordered IDs created offline, no clashes during sync |
| `rust_decimal` | FX rates and conversions. **Never `f64` for money**. Amounts are stored as `i64` minor units + ISO currency |
| `time` or `chrono` | timestamps |
| `thiserror` | typed errors across the FFI |
| `rusqlite` (feature `bundled`) + `rusqlite_migration` | on-device DB, same SQLite version on both platforms |
| `uniffi` | Kotlin/Swift bindings |
| `prost` | compact protobuf sync payloads |
| `reqwest` (rustls) + `tokio` | sync client |
| Server: `axum`, `tokio`, `tower-http`, `sqlx` (postgres), `tracing`, `jsonwebtoken` | backend |
| Tests: `proptest`, `criterion`, `insta` | invariants, benchmarks, snapshot tests |

### 3.4 Data model (designed for receipt scanning from day one)
```
Group(id, name, base_currency, created_at)
Member(id, group_id, display_name, user_id?)            -- user_id null = guest
Expense(id, group_id, payer_id, title, total_minor, currency, fx_rate_to_base, date, category?, receipt_id?)
ExpenseItem(id, expense_id, label, qty, unit_minor, total_minor)   -- one row for manual expenses, many for receipts
ItemShare(item_id, member_id, weight)                    -- equal / weighted / exact via weights
Transfer(id, group_id, from_id, to_id, amount_minor, currency, date)   -- reimbursements
Receipt(id, image_ref, status, vlm_raw_json, parsed_at)
Op(id, group_id, device_id, lamport, kind, payload)      -- sync op-log
```
Splitting by **items** isn't something you bolt on later. It's the core shape, and a manual expense is just a receipt with one item. When the VLM feature arrives, it fills `ExpenseItem` rows, and the user assigns members per item (or "everyone").

### 3.5 Core algorithms
- **Split.** Work in integer minor units. Distribute the remainder with the largest-remainder method, ordered deterministically by member id, so every device gets the same result.
- **Balances.** `paid − owed` per member, in base currency. Invariant: the sum is always 0 (checked with proptest).
- **Settle-up.** Greedy match of the largest creditor with the largest debtor gives at most n−1 transfers. Truly minimal is NP-hard, and greedy is the industry norm.
- **Sync.** Offline-first append-only op-log per group with Lamport clocks and last-writer-wins per field. Deletes are tombstones. The server orders and relays ops. Simpler than full CRDTs and enough for this domain.

### 3.6 Optimization checklist
- **Rust release profile:** `opt-level = 3` (or `"s"` if APK size matters more), `lto = "fat"`, `codegen-units = 1`, `panic = "abort"`, `strip = true`. Check size with `cargo bloat`.
- **FFI:** coarse-grained calls (return a whole screen's view-model, not 50 small getters), and keep work off the UI thread (UniFFI async).
- **SQLite:** WAL mode, prepared-statement cache, indexes on `(group_id, date)`, and one transaction per batch of ops.
- **Android:** R8 full mode, Baseline Profiles + Startup Profiles for Compose, ship only `arm64-v8a` + `armeabi-v7a` (+ `x86_64` for emulators) through App Bundle splits.
- **iOS:** static XCFramework, `-Osize` for Swift if needed.
- **Network:** protobuf + zstd for sync deltas, ETag / since-cursor pulls, no polling (use push notifications to trigger sync).
- **Receipts (later):** downscale on device to about 1600 px long side as JPEG/WebP (~200–400 KB) before upload. Show a progress state and let the user edit the result.
- **Measure, don't guess:** `criterion` benchmarks for the split engine, Android Macrobenchmark for startup and scroll, Xcode Instruments on iOS.

---

## 4. Milestones

| # | Milestone | Exit criteria |
|---|---|---|
| M0 | Tooling installed | `rea inspect-android-package` works on a test APK, `cargo ndk build` produces a `.so` |
| M1 | Study complete | `docs/SPEC.md` approved. `_study/` stays out of git |
| M2 | `dibill-domain` | Split, balance, and settle algorithms written, proptest invariants pass, benchmarks recorded |
| M3 | Local-only app on Android | Create group, add members and expenses, see balances, all offline |
| M4 | iOS shell | Same features on iOS from the same core |
| M5 | Backend + sync + invite links | Two devices converge after offline edits |
| M6 | Polish | Multi-currency, export CSV/PDF, attachments, accessibility, localization (es/en) |
| M7 | **Receipt scanning** | Camera → upload → VLM → editable item list → per-item assignment |
| M8 | Store release | Play: closed testing track (new personal accounts need 12 testers for 14 days), then production. Apple: TestFlight, then review |

### 4.1 Order from 2026-10-30 (owner: test with friends, then receipts, then iOS)

M3 and M5 are done in code. The owner wants friends testing as soon as possible, then receipt
scanning, then iOS. The order below follows dependencies and what is blocked on whom:

| Phase | What | Depends on | Exit criteria |
|---|---|---|---|
| **B1 Beta server** | Production setup of `dibill-server` (ARM64 container, PostgreSQL, backups, rate limits, logs) on the owner's Raspberry Pi 4B or a small VPS, exposed over HTTPS | Owner: hosting choice, Pi set up | Phones outside the home network sync through it |
| **B2 Android beta** | Release build pointing at the beta server, signed with a beta key kept by the owner, handed to friends (APK or Play internal testing) | B1; owner: distribution choice | Friends install it, share a group, sync |
| **M7a VLM evaluation** | Receipt schema (Rust types → JSON Schema), evaluation set with our own ground truth, a Rust harness that runs many models with structured output and scores accuracy, cost and latency | Owner: API keys/credits; nothing else (starts in parallel with B1) | A report that names the cheapest model meeting the bar |
| **M7b Scan in the app** | Server endpoint (image → VLM → validated items), per-device credits/limits paid by the owner, Android camera + review + per-item assignment | M7a, B1 | A friend scans a real receipt and splits it by item |
| **M4 iOS** | SwiftUI shell on the same core, built and tested on macOS CI runners (no Mac needed); TestFlight for iPhone friends | Owner: Apple Developer Program ($99/yr) for any install on a real iPhone | Same features as Android on TestFlight |
| M6, M8 | Polish, store release | as before | as before |

Why this order: B1/B2 put the finished sync in friends' hands now and give real feedback (and
real receipts for M7). M7a needs no server and runs in parallel; its result decides M7b's
provider. iOS last: it is the most expensive (Apple account, macOS CI minutes, no local
simulator) and only pays off once the product is validated on Android.


- Google Play Console account (one-time US$25) and Apple Developer Program (US$99/year).
- Privacy policy URL, Play *Data safety* form, Apple *privacy nutrition labels*. Receipt images sent to a cloud VLM must be declared.
- If you add Google sign-in, Apple requires Sign in with Apple too (or another privacy-preserving login option).
- GDPR: account and data deletion inside the app (both stores require it), plus a DPA with the VLM provider.

---

## 5. Next actions
1. ~~Scaffold repo in WSL, write `dibill-domain` (split, balance, settle) with tests.~~ Done.
2. Run `./scripts/setup-ubuntu.sh` with sudo (JDK, unzip, adb), then `npx rea-agents setup`.
3. SSH key for the personal GitHub account, private repo, first push, protect `main`.
4. Connect the phone (1.3), pull `base.apk` into `_study/apk/`.
5. Run 2.1 triage and share the output. That confirms the stack and how big the analysis will be.
