# CLAUDE.md

Guidance for Claude Code (and any AI assistant) working in this repository.

## Project

DiBillSion: group expense splitting for Android and iOS, with receipt scanning (photo → line items → per-item assignment).
Shared Rust core + native UI (Compose / SwiftUI) via UniFFI, Rust backend (axum + PostgreSQL).
The full plan is in `docs/PLAN.md`, decisions in `docs/adr/`, the functional spec in `docs/SPEC.md`.
**Current state and backlog: `docs/STATUS.md`. Read it at the start of every session and keep it updated.**

## Autonomy

The owner (`carlosbelop`) wants sessions to **implement and develop autonomously, without asking for permission**.

- Pick the next item from `docs/STATUS.md` (or the plan's milestones), then run the full cycle on your own: issue → branch → code + tests → PR → wait for CI → squash-merge when green → update `docs/STATUS.md` → next item.
- Make reasonable technical decisions yourself. Record significant ones as ADRs instead of asking.
- If CI fails, fix it. Don't merge red PRs, and don't disable checks or lints to get green.
- Keep PRs small and focused (one issue each) so the history stays reviewable.
- **Only escalate crucial things.** The owner approves them through Remote Control or a push notification:
  - Anything needing `sudo`, passwords, tokens or other credentials
  - Physical actions: the phone (pairing, installing, unlocking), USB, hardware
  - Spending money or creating accounts (stores, cloud, VLM provider, domains)
  - Publishing anything publicly: store submission, making the repo public, posting
  - Irreversible or destructive operations: deleting data or branches with unmerged work, rewriting published history
  - Product or legal decisions that change scope: features beyond the plan, licensing, branding
- When blocked on one of those, send a notification explaining exactly what's needed, then continue with other backlog items that aren't blocked.

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

- `main` is **not** protected by GitHub (rulesets aren't enforced on private repos on the free plan), so this workflow is self-enforced. Never commit or push to `main` directly. Every change follows **issue → linked branch → PR**:
  1. Create (or reuse) an issue, **assigned to `carlosbelop`**:
     `gh issue create --assignee carlosbelop --title "…" --body "…"`
  2. Create the branch from the issue, so it's linked under the issue's *Development* section:
     `gh issue develop <n> --name <type>/<n>-<slug> --base main --checkout` (type: `feat`, `fix`, `chore`, `docs`, `refactor`, `test`)
  3. Open the PR, **assigned to `carlosbelop`**, with `Closes #<n>` in the body:
     `gh pr create --assignee carlosbelop --base main`
  4. Wait for CI (`gh pr checks <n> --watch`), then squash-merge when green: `gh pr merge <n> --squash --delete-branch`. The issue closes automatically and the remote branch is deleted. Then locally:
     `git switch main && git pull --ff-only && git branch -D <branch> && git remote prune origin`
- Always assign `carlosbelop` to issues and PRs.
- Conventional Commits (`feat: …`, `fix: …`). Don't put `(#n)` in commit messages: the squash merge adds the PR number, and the PR body links the issue. Commits are SSH-signed with the repo-local identity.
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
- REA (reverse-engineering MCP server `rea`) is registered for Claude Code. Its variables (`REA_JADX_MCP_JAR`, `REA_APKTOOL_COMMAND`, timeouts, heap) are set in `~/.claude/settings.json` (`env`) and `~/.bashrc`.
- The phone (Pixel 9a) connects through adb wireless debugging. If `adb devices` is empty, ask the owner to re-pair; that's a physical action.
- Permissions: `.claude/settings.json` allows the dev toolchain and denies `sudo`, force pushes, pushes to `main` and reading secrets.

## Layout

| Path | Purpose |
|---|---|
| `crates/dibill-domain` | Pure money logic, no I/O |
| `crates/dibill-store` | On-device SQLite store |
| `crates/dibill-sync` | Sync model: op-log, clocks, merge (no I/O) |
| `crates/dibill-proto` | Sync API wire types (app core + server) |
| `crates/dibill-core` | Facade for mobile apps (UniFFI) |
| `crates/dibill-server` | Backend |
| `android/`, `ios/` | Native UI shells |
| `_study/` | Local-only research (ignored) |
