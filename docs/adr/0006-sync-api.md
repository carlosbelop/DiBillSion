# ADR 0006: Sync API and wire format

- Status: accepted
- Date: 2026-10-30

## Context

ADR 0004 and ADR 0005 define ops and how a device merges them. Devices now need a server to exchange
ops through. SPEC §1.1 says the app works without an account and that groups are joined through a
share link; PLAN §3.6 suggested protobuf + zstd for sync payloads. This ADR fixes the protocol so
the server (`dibill-server`) and the client (`dibill-core`) can be built against one contract,
shared as the `dibill-proto` crate.

## Decision

### Transport and encoding
- HTTPS, JSON bodies (serde), paths under `/v1`. Responses are gzip-compressed when the client
  accepts it. Request bodies are capped at 1 MiB.
- JSON instead of protobuf for now: op values are already JSON (ADR 0005), an op is ~200 bytes,
  and JSON can be read in logs and with `curl`. Protobuf + zstd is a drop-in change of encoding
  behind `/v2` if measurements ever show the payloads matter.
- Ids are UUID strings, Lamport counters integers (at most `i64::MAX`), op values opaque strings.
- Errors are `{"error": <code>, "message": <English detail>}` with a stable `code`:
  `unauthorized`, `forbidden`, `not_found`, `invalid_request`, `too_large`.

### Devices and auth
- `POST /v1/devices {device_id}` registers this install (the store's sync device id) and returns a
  random 256-bit bearer token. The server keeps only its SHA-256 hash. Registering an id that
  already exists is `forbidden`, so a token can't be obtained for someone else's device.
- Every other call sends `Authorization: Bearer <token>`.
- No accounts yet. Signing in later links several device ids to one user; the protocol for ops
  doesn't change.

### Groups and access
- A group exists on the server once a device shares it: `PUT /v1/groups/{id}` (idempotent). The
  sharing device becomes the group's owner and a member.
- Only member devices can push, pull or create invites; anyone else gets `forbidden`, and an
  unknown group is `not_found`.

### Ops
- **Push** `POST /v1/groups/{id}/ops {ops: [...]}`, at most 1000 ops per call. Every op must belong
  to the group in the path and have been made by the calling device (`op.device` = the token's
  device), so devices can't forge each other's ops. Known op ids are skipped, so retries are
  safe. The server checks only the envelope (ids, entity kind, field and value present together,
  value ≤ 64 KiB), never the values: those are the client's (ADR 0005 drops malformed ones).
- The server gives each new op the next **sequence number of its group**. Answer:
  `{accepted, cursor}`.
- **Pull** `GET /v1/groups/{id}/ops?after=<seq>&limit=<n>` (n ≤ 1000) returns ops with
  `seq > after` in order: `{ops: [{seq, op}], cursor, more}`. The client stores the cursor per
  group and pulls again while `more` is true. Pulls include the caller's own ops, which the store
  then marks as pushed.
- The sequence only orders delivery; which value wins is still decided by Lamport stamps.
- No push notifications in v1: clients pull when a group opens, on pull-to-refresh and after
  pushing. FCM/APNs "something changed" pings can come later without changing this API.

### Invites
- `POST /v1/groups/{id}/invites` returns the group's invite code (created on first call, the same
  code afterwards): 128 random bits, base64url. Only the owner can call it.
- `DELETE /v1/groups/{id}/invites` turns the link off; the next `POST` makes a new code
  ("regenerate").
- `POST /v1/invites/{code}/join` makes the caller a member and returns `{group_id}`. The device then
  pulls the group and picks "who I am" locally (ADR 0005's device-local `me`): claiming a member
  needs no server call.
- Links are `https://<domain>/join/<code>`, opened by the app through Android App Links /
  iOS Universal Links. Until there is a domain (owner decision), the app also accepts
  `dibillsion://join/<code>`.

## Consequences

- The server stays a relay: it never has to understand groups, members or money, so the domain
  rules live in one place (the Rust core on the devices).
- A device that loses its token loses access to shared groups until accounts exist; reinstalling
  creates a new device that has to join again with an invite.
- Someone holding an invite code can join until it is turned off. Members can't be removed from
  the server yet; that comes with accounts.
- The server can show nothing about a group (not even its name) without reading values, so an
  invite preview, if wanted, is computed by the app after joining.

## Addendum (2026-10-30, server skeleton)

- Error code `internal` for unexpected server failures (HTTP 500); clients retry later.
- PostgreSQL is reached through `tokio-postgres` with a `deadpool-postgres` pool instead of the
  `sqlx` named in PLAN §3.3: `sqlx` lists an optional SQLite driver, and Cargo's `links` check
  makes it clash with `rusqlite`'s SQLite in the shared workspace lockfile. Migrations are an
  embedded, append-only list of SQL files applied in order under an advisory lock, recorded in
  `schema_migrations`.
