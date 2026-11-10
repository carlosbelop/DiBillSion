# ADR 0010: Receipt photos attached to expenses

- Status: accepted
- Date: 2026-11-10

## Context

Testers want to check a scanned receipt's items against the photo, from any phone in the group.
Until now the photo was only sent to the model and then dropped. Expenses sync as ops (ADR 0004,
0005); a photo (~200–400 KB JPEG) is too big for an op value (64 KB) and shouldn't be re-sent
with every edit.

## Decision

- **A new transaction field, `attachment`**, holding the photo's id (a UUID, or `null`).
  Last-writer-wins like any field and independent of `body`, so editing the expense keeps it.
- **The bytes are an attachment**, outside the op log: on the phone in a new `attachments` table
  (store schema v5) with an `uploaded` flag; on the server in `attachments` (members only,
  removed with the group) behind `PUT`/`GET /v1/groups/{id}/attachments/{attachment}` with the
  image as base64 JSON (the transport carries text, ADR 0007), at most `MAX_RECEIPT_BYTES`.
- **Upload on sync, before the ops**: a group's pending photos go up before its ops, so nobody
  receives an expense naming a photo the server doesn't have. A group that isn't shared yet keeps
  them pending; forgetting a shared group marks them pending again.
- **Download on demand**: other phones see the id with the expense and fetch the bytes the first
  time someone opens the photo (`receipt_image`), then keep them. Uploads are idempotent.
- The apps attach the same downscaled JPEG they sent for scanning (1600 px, ~300 KB), right
  after saving the expense.

## Consequences

- Each scanned receipt costs one extra upload; photos stay on the server until the group is
  deleted (deleting one expense doesn't remove its photo yet).
- Apps from before this ADR drop `attachment` ops as unknown fields: the photo just doesn't show.
- The server stores receipt images: personal data to mention in the privacy policy before
  publishing (PLAN, store prerequisites).
