# ADR 0007: Sync client with platform HTTP

- Status: accepted
- Date: 2026-10-30

## Context

The apps have to talk to the server (ADR 0006). The protocol logic (registering, pushing pending
ops in batches, paging pulls, keeping cursors, marking ops pushed, mapping errors) must be the same
on Android and iOS, so it belongs in the Rust core. Doing HTTPS from Rust on phones means
shipping a TLS stack and root certificates in the native library: more size and build complexity
for the NDK and Xcode, and certificate bundles whose licences (e.g. CDLA for `webpki-roots`) are
outside our allow-list. The platforms already have well-tested HTTP clients that use the system
trust store, proxies and network policies.

## Decision

- The core defines a foreign trait, `HttpTransport`, with one method:
  `request({method, path, token, body}) -> {status, body}` or `TransportError::Offline`. The app
  implements it (`HttpURLConnection`/OkHttp on Android, `URLSession` on iOS) and prefixes its
  configured server URL. It is called on the background thread that called the core.
- Everything else is Rust (`dibill-core::sync`): `share_group`, `invite_code`, `turn_off_invite`,
  `join_group`, `sync` and `is_shared`, built on `dibill-proto` types.
- Client state lives in the store (schema v4, device-local, never synced): the device's server
  token (in `sync_device`) and, per shared group, the pull cursor (`group_sync`).
- The store lock is released while a request is in flight; ops are marked pushed by id after the
  server accepts them, so local edits made meanwhile stay pending.
- New errors: `CoreError::Offline` (no HTTP answer; changes stay queued) and
  `CoreError::Server { code, reason }` carrying the server's stable error code.

## Consequences

- No TLS or certificate code in the native library; TLS behaviour follows each OS.
- Each app writes ~30 lines of HTTP glue. The protocol is tested once in Rust against an
  in-memory fake server (and the real server's own tests cover the server side).
- A lost token (app data cleared) means the device id can't be registered again; the user rejoins
  shared groups with an invite. Accounts (later) will fix this.
