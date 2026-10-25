# CLAUDE.md

Guidance for Claude Code (and any AI assistant) working in this repository.

## Project

DiBillSion: group expense splitting for Android and iOS, with receipt scanning (photo → line items → per-item assignment).
Shared Rust core + native UI (Compose / SwiftUI) via UniFFI, Rust backend (axum + PostgreSQL).
The full plan is in `docs/PLAN.md`, decisions in `docs/adr/`, the functional spec in `docs/SPEC.md`.

## Hard rules

1. **Clean room.** We study an existing app's *behavior* only.
   - Decompiler output, APKs, screenshots and notes live in `_study/` (git-ignored, CI-guarded). Never commit them.
   - Never copy code, smali, class names, strings, icons, images, layouts or private API details from third-party apps into the repo.
   - The only study output allowed in the repo is `docs/SPEC.md`, written in our own words.
2. **Money is `i64` minor units** (cents). No floats anywhere in money paths (`clippy::float_arithmetic` is denied). FX uses `rust_decimal`.
3. **Determinism.** Split, balance and settle results must be identical on every device. Break ties by `MemberId`.
4. **No secrets in git.** No keystores, API keys or `.env` files.
5. **Git attribution.** Never add `Co-Authored-By: Claude`, "Generated with Claude Code" or any AI attribution to commits or PRs.

## Workflow

- `main` is protected. Never commit to it directly. Every change follows **issue → linked branch → PR**:
  1. Create (or reuse) an issue, **assigned to `carlosbelop`**:
     `gh issue create --assignee carlosbelop --title "…" --body "…"`
  2. Create the branch from the issue, so it's linked under the issue's *Development* section:
     `gh issue develop <n> --name <type>/<n>-<slug> --base main --checkout` (type: `feat`, `fix`, `chore`, `docs`, `refactor`, `test`)
  3. Open the PR, **assigned to `carlosbelop`**, with `Closes #<n>` in the body:
     `gh pr create --assignee carlosbelop --base main`
  4. Squash-merge. The issue closes automatically and the remote branch is deleted (repo setting). Then locally:
     `git switch main && git pull --ff-only && git branch -D <branch> && git remote prune origin`
- Always assign `carlosbelop` to issues and PRs.
- Conventional Commits (`feat: …`, `fix: …`). Commits are SSH-signed with the repo-local identity.
- Before committing, everything must pass:
  ```bash
  cargo fmt --all
  cargo clippy --workspace --all-targets -- -D warnings
  cargo nextest run --workspace
  cargo deny check
  ```
- Add tests with every logic change. Prefer `proptest` for money invariants (parts sum to total, balances sum to zero).
- Record architecture changes as a new ADR in `docs/adr/`.

## Environment

- WSL2 Ubuntu 24.04 (or native Ubuntu). The repo lives at `~/proyectos/dibillsion`, never under `/mnt/c`.
- Tools are installed by `scripts/setup-ubuntu.sh` (single source of truth for versions).
- REA (reverse-engineering MCP) uses `REA_JADX_MCP_JAR` and `REA_APKTOOL_COMMAND` from `~/.bashrc`.

## Layout

| Path | Purpose |
|---|---|
| `crates/dibill-domain` | Pure money logic, no I/O |
| `crates/dibill-core` | Facade for mobile apps (UniFFI) |
| `crates/dibill-server` | Backend |
| `android/`, `ios/` | Native UI shells |
| `_study/` | Local-only research (ignored) |
