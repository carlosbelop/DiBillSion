# DiBillSion

Group expense splitting for Android and iOS, built on a shared Rust core, with receipt scanning:
take a photo of a receipt, and each line item is added to the group expense automatically.

> Status: early development. Live status: [docs/STATUS.md](docs/STATUS.md); plan: [docs/PLAN.md](docs/PLAN.md).

## Architecture

```
Android (Jetpack Compose) ─┐                 ┌─ iOS (SwiftUI)
                           └─ UniFFI ─ dibill-core (Rust) ─┘
                                         │
                                  dibill-server (Rust, axum + PostgreSQL)
```

| Path | Contents |
|---|---|
| `crates/dibill-domain` | Pure money logic: split (largest remainder), balances, settle-up. No I/O |
| `crates/dibill-store` | On-device SQLite persistence with migrations |
| `crates/dibill-core` | Facade for the mobile apps (UniFFI bindings) |
| `crates/dibill-server` | Backend (sync, invite links, receipt→VLM proxy) |
| `android/`, `ios/` | Native UI shells |
| `docs/` | Plan, functional spec, architecture decision records (ADRs) |
| `scripts/` | Environment setup, binding generation |

Decisions are recorded in [docs/adr](docs/adr).

## Getting started (Ubuntu 24.04, native or WSL2)

```bash
git clone <repo-url> && cd dibillsion
./scripts/setup-ubuntu.sh
source ~/.bashrc
cargo nextest run --workspace
cd android && ./gradlew :app:assembleDebug   # Android app (builds the Rust core with cargo-ndk)
```

On WSL2, keep the repo inside the Linux filesystem (`~/...`), not under `/mnt/c`.

## Development workflow

- Every change goes issue → linked branch → pull request → green CI → squash merge (see CLAUDE.md). GitHub does not enforce this on the free plan, so it is self-enforced.
- Branches: `feat/...`, `fix/...`, `chore/...`, `docs/...`
- Commits follow [Conventional Commits](https://www.conventionalcommits.org/) (`feat: add weighted split`).
- Before pushing:
  ```bash
  cargo fmt --all
  cargo clippy --workspace --all-targets -- -D warnings
  cargo nextest run --workspace
  cargo deny check
  ```

## Rules

1. **Money is integer minor units** (`i64` cents). Floats are banned by clippy.
2. **Original work only:** nothing from other apps (code, strings, icons, assets, APIs) goes into this repo.
3. **No secrets in git:** signing keys and API keys go in GitHub Secrets / local `.env`.

## License

Proprietary. All rights reserved. See [LICENSE](LICENSE).
