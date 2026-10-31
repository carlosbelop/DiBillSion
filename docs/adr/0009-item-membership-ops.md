# ADR 0009: Item membership synced per item and person

- Status: accepted
- Date: 2026-10-31

## Context

A scanned receipt is one expense with many items, each split equally among its own people
(SPEC flow 8). The owner wants people to open a saved receipt and add or remove themselves, or
someone else, from each item. With the sync model of ADR 0004/0005 a transaction is one
last-writer-wins `body` field, so two flatmates changing different items at the same time would
lose one of the two changes: whoever synced last would overwrite the other's item list.

## Decision

- A new transaction field, `Share { item, member }`, sent on the wire as
  `share:<item position>:<member id>` with a JSON number of parts as the value (`1` to share the
  item equally, `0` to leave it). Each (item, member) pair is its own last-writer-wins register,
  so changes to different items, or by different people on the same item, all survive.
- Projection: the winning `body` is read first; then every `Share` field **newer than that body**
  is applied, in field order (item, then member). A share older than the body is ignored, so
  saving the whole transaction again (the editor) replaces earlier item changes; the editor
  starts from what is shown, so it keeps them. Shares are skipped for unknown members, unknown
  item positions and items split by exact amounts, and a change that would leave an item with
  nobody is ignored, so a transaction never disappears because of item changes.
- `Store::set_item_share` (core `set_item_member`) checks the same rules up front: the item
  exists and is split by parts, the last person can't leave, inactive members can't be added,
  transfers are refused.
- The server relays the new field like any other (it never reads field names or values).

## Consequences

- People can fix item assignments after saving, from any phone, at the same time.
- A whole-transaction edit made concurrently with item changes (without having seen them) wins
  over them: rare, since receipts are edited item by item, and visible on the screen.
- Apps from before this ADR drop `share:` ops as unknown fields and never see them later (the
  pull cursor has moved on). Everyone in a shared group must update before using item changes;
  the friends beta starts after this, so no installed app is affected.
- The reference-replica property test still covers bodies; item changes have their own tests
  (concurrent changes on one item, a newer body superseding them, wire round trip).
