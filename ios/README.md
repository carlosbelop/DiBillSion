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

The Swift files that don't use SwiftUI (`Money`, `SyncTransport`, `AppModel`) and the tests
also compile on Linux with the swift.org toolchain, linked against `target/debug/libdibill_core.so`
and the bindings from `scripts/gen-bindings.sh "" swift`. Handy for quick checks; the CI run on
macOS is the reference.

## CI

While the repository is private, macOS minutes are expensive, so the workflow runs only when a
PR has the `macos` label or on manual dispatch. Once the repository is public it runs on every
PR and push that touches iOS or the core.
