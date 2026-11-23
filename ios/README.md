# iOS app

SwiftUI app (iOS 17+) on the same Rust core as Android. No Mac is needed day to day: the
macOS CI workflow (`.github/workflows/ios.yml`) builds and tests it.

| Path | Contents |
|---|---|
| `project.yml` | XcodeGen spec; `xcodegen` writes `DiBillSion.xcodeproj` (not tracked) |
| `DiBillSion/` | App: `AppModel` (core calls off the main actor), `SyncTransport` (`URLSession`), `Money`, `Views/` |
| `DiBillSion/Localizable.xcstrings` | English source strings, Spanish translations |
| `DiBillSionTests/` | XCTest: money parsing/formatting, dates, and a smoke test through the real core |
| `Generated/` | Not tracked. `DibillCore.xcframework` + `dibill_core.swift` from `scripts/build-xcframework.sh` |

## Build on a Mac

```bash
rustup target add aarch64-apple-ios aarch64-apple-ios-sim
brew install xcodegen
scripts/build-xcframework.sh --sim      # or no flag: device + simulator, release profile
cd ios && xcodegen && open DiBillSion.xcodeproj
```

The Debug build syncs with `http://127.0.0.1:8080` (a `dibill-server` on the same Mac; see
`docs/STATUS.md`, "Dev server"). Release builds take the server from the `SYNC_URL` build
setting (empty until the beta server is up, which turns invites off).

## Without a Mac

The Swift files that use neither SwiftUI nor Apple-only Foundation APIs (`Money`, `Split`,
`ReceiptDraft`, `SyncTransport`) type-check on Linux with a swift.org toolchain (Swift 6.2,
unpacked by hand into `~/.local/share/dibillsion-tools/swift`, with the `libncurses6`/`libtinfo6`
packages it needs in `swiftlibs`; `scripts/setup-ubuntu.sh` doesn't install it):

```bash
scripts/gen-bindings.sh target/bindings/swift swift
cp target/bindings/swift/dibill_coreFFI.modulemap target/bindings/swift/module.modulemap
T=~/.local/share/dibillsion-tools
LD_LIBRARY_PATH=$T/swiftlibs/usr/lib/x86_64-linux-gnu $T/swift/usr/bin/swiftc -typecheck \
  -I target/bindings/swift target/bindings/swift/dibill_core.swift \
  ios/DiBillSion/{Money,Split,ReceiptDraft,SyncTransport}.swift
```

`AppModel` uses `String(localized:)`, which the Linux Foundation lacks. Handy for quick checks;
the CI run on macOS is the reference.

## CI

The workflow runs on every PR and every push to `main` that touches `ios/`, `crates/`, the Cargo
files or the binding scripts, and on manual dispatch. It isn't a required check, so a PR that
only changes the core still needs its iOS run to be green before merging.

## TestFlight

`.github/workflows/testflight.yml` archives a signed Release build and uploads it to App Store
Connect: **Actions → TestFlight → Run workflow** (or push a tag `ios-v*`). Each run creates a
throwaway Apple Distribution certificate and App Store profile through the API key
(`scripts/asc.py`), signs with them, uploads, then revokes and deletes them (already uploaded
builds are unaffected), so no signing material is stored anywhere. Xcode 26 (`macos-26`) is
required by App Store Connect. The build number is the workflow run number. On PRs it only makes an unsigned device
archive.

One-time setup (owner, after the Apple Developer Program is approved):

1. **Team ID**: developer.apple.com → Account → Membership details. Save it as the secret
   `APPLE_TEAM_ID`.
2. **App ID**: Certificates, Identifiers & Profiles → Identifiers → + → App IDs → App, explicit
   bundle ID `com.dibillsion.app`.
3. **App record**: App Store Connect → Apps → + New App: iOS, name "DiBillSion", primary language
   Spanish (Spain), bundle ID `com.dibillsion.app`, SKU `dibillsion-ios`.
4. **API key**: App Store Connect → Users and Access → Integrations → App Store Connect API →
   Team Keys → +, access **Admin** (CI creates and revokes its signing certificate itself).
   Download the `.p8` (only possible once) and add the secrets:
   ```bash
   gh secret set ASC_KEY_ID --body "<Key ID>"
   gh secret set ASC_ISSUER_ID --body "<Issuer ID>"
   gh secret set ASC_KEY_P8 < AuthKey_<Key ID>.p8
   gh secret set APPLE_TEAM_ID --body "<Team ID>"
   ```
5. **Testers** (internal testing, no beta review): Users and Access → + → invite each tester
   with the *Customer Support* role; once they accept, TestFlight → Internal Testing → + group →
   add them, with automatic distribution on. They install the TestFlight app and accept the invite.

The beta server URL comes from the repository variable `SYNC_URL`
(`gh variable set SYNC_URL --body https://<host>.<tailnet>.ts.net`).
