# Functional specification

> Written in our own words from studying how group-expense apps behave (milestone M1).
> This is the only study output allowed in the repo: no code, strings, assets or API details
> from third-party apps. It describes **DiBillSion**, including where we deliberately differ.
>
> Status: draft v2 (2026-10-25), black-box observation done. Items marked **(open)** are product
> decisions still to make.

## 1. Features

### 1.1 v1 (MVP, milestones M3–M6)
- **Groups.** Create, rename, archive, restore and delete a group. A group has a name, an
  optional emoji, a kind (trip, home, couple, event, project, other) and one base currency.
- **Members.** Named participants inside a group. A member may later be claimed by a real
  user account; until then it is a placeholder anyone in the group can act for. On each
  device the user picks "who I am" in the group, and can change it.
- **Transactions**, three kinds:
  - *Expense*: one member paid for something shared by some members.
  - *Income*: one member received money that belongs to some members (a refund, a deposit
    returned, selling shared stuff).
  - *Transfer*: one member paid another back (a reimbursement).
- **Split modes** for expenses and incomes: equally, by parts (integer weights) and by exact
  amounts. Each member can be included or excluded per transaction. A mixed mode (some exact
  amounts, the rest by parts) is supported by the data model; the UI may come later.
- **Line items.** Every expense is a list of items, each split on its own. A manual expense
  is a single item. Receipt scanning (M7) only has to fill the items.
- **Multiple currencies.** Each transaction has its own currency and a stored exchange rate
  to the group's base currency, pre-filled and editable. The editor remembers the last
  currency used in the group.
- **Balances** per member, and **suggested reimbursements** that settle the group. Marking a
  suggestion as paid records a transfer.
- **Categories.** A small built-in set (accommodation, groceries, restaurants, transport,
  entertainment, shopping, health, rent and bills, other) plus custom categories (name + emoji).
- **Statistics.** Spending by category, for "me" or for the whole group, by month, year or all
  time.
- **Search** within a group's transactions; **duplicate** a transaction.
- **Offline first.** Everything except sharing, joining and uploading attachments works with
  no connection; changes sync when the device is online (M5).
- **Invites (M5).** A share link and its QR code. Opening the link installs or opens the app,
  then the user claims an existing placeholder member or joins as a new one. The group owner
  can turn the link off or regenerate it.
- **Attachments (M6).** Photos or PDFs on a transaction (camera or gallery).
- **Export (M6).** CSV of all transactions, and a PDF summary with balances. (Our addition:
  the studied app offers none.)
- **Accounts.** Optional. Without an account the user keeps groups on one device and can
  still join shared groups. Signing in (email link, Sign in with Apple, Google) syncs groups
  across devices. Account and data deletion inside the app.
- **Languages:** English and Spanish at launch.

### 1.2 Exact to the cent (all milestones)
Every amount, share and balance is a whole number of the currency's smallest unit. Shares of an item
always add up to the item, balances always add up to exactly zero, and once everyone pays the
suggested reimbursements every balance is exactly zero. A split never leaves a stray cent that nobody
owes, and a list never shows shares that add up to more or less than the total. (The studied app keeps
fractional shares and rounds each balance on its own, which leaves such cents behind.)

### 1.3 Differentiator (M7)
- **Receipt scanning.** Take a photo of a receipt → the server's VLM returns line items
  (label, quantity, unit price, line total, discounts, tax, tip) → the user edits them and
  assigns each item to members ("everyone" by default) → one expense with many items.

## 2. User flows

1. **First run.** Create a group: name, base currency (defaults to the device locale's
   currency), member names including "me". Optionally invite now (link, QR) or later.
2. **Add an expense.** Amount → title → paid by (defaults to "me") → date (today) → split
   (equally among everyone by default) → optional category, currency, attachment. Save works
   offline. Leaving with unsaved changes asks for confirmation.
3. **Foreign currency.** Picking a currency other than the base one shows the rate
   (pre-filled, editable) and the converted amount live.
4. **Check balances.** The balances tab lists each member's net amount and a status card
   ("you owe", "you are owed", "settled"). Suggested reimbursements are listed below.
5. **Settle.** Tapping a suggestion offers "mark as paid", which records a transfer for that
   amount. The amount can be edited (partial payment).
6. **Join a group.** Open an invite link → pick "who I am" among placeholders, or join as a
   new member.
7. **Remove a member.** Only possible when their balance is zero and they are not in any
   transaction; otherwise the member can be *deactivated*: hidden from new splits, kept in
   history and balances. (Deliberately different from silently redistributing their share.)
8. **Scan a receipt (M7).** Camera → progress → editable item list → assign members per item
   → save as one expense.

## 3. Data model

Base: [PLAN §3.4](PLAN.md). Refinements from the study:

```
Group(id, name, emoji?, kind, base_currency, created_at, archived_at?, invite_token?, invite_enabled)
Member(id, group_id, display_name, user_id?, status: active|inactive, created_at)
Transaction(id, group_id, kind: expense|income|transfer, title, payer_id, currency,
            fx_rate_to_base?, date, category_id?, created_by?, receipt_id?, created_at, updated_at)
  -- fx_rate_to_base is null when currency == base currency
  -- transfer: payer_id = sender, exactly one share with the receiver
ExpenseItem(id, transaction_id, position, label?, qty?, unit_minor?, total_minor)
ItemShare(item_id, member_id, mode: parts|amount, weight?, amount_minor?)
Category(id, group_id?, name, emoji, builtin)
Attachment(id, transaction_id, mime, uri, uploaded_at?)
Receipt(id, image_ref, status, vlm_raw_json, parsed_at)
Op(id, group_id, device_id, lamport, kind, payload)      -- sync op-log (M5)
```
- IDs are UUID v7, created offline.
- Money is `i64` minor units of the stated currency; rates are decimals (never floats).
- One payer per transaction. Multiple payers are modelled as several transactions.

## 4. Calculation rules (with worked examples)

All rules are deterministic: same inputs give the same output on every device, regardless of
language, locale or member names. Ties are broken by member id.

- **Split.** Integer cents, largest-remainder method. Leftover cents go to the members with the
  largest fractional parts, ties to the lowest member id.
  10.00 € split equally among A, B, C → A 3.34, B 3.33, C 3.33.
  10.00 € split 2:1:1 → 5.00, 2.50, 2.50.
- **Items.** Each item is split independently among its own members.
  A pays a 31.50 € receipt: pizza 12.00 (A, B), salad 8.50 (C), drinks 11.00 (A, B, C) →
  A owes 6.00 + 3.67, B owes 6.00 + 3.67, C owes 8.50 + 3.66; A's balance is +21.83.
- **Currency conversion:** each foreign amount is converted once into the group's base currency with the
  rate stored on the expense, then rounded to the nearest minor unit, halves away from zero (ADR 0002).
  10.00 USD at 0.9215 → 9.215 € → 9.22 €. A refund of −10.00 USD converts to exactly −9.22 €.
  1,234 JPY at 0.0061 → 7.5274 € → 7.53 €. Every currency uses its own ISO 4217 minor unit
  (JPY 0 decimals, KWD 3), never a fixed 2.
- **Multi-item expenses in a foreign currency:** the receipt total is converted once, then spread over the
  items in proportion to their original amounts (largest remainder, ties to the earlier item), so the
  converted items always add up to the converted total.
  Receipt 12.00 + 8.50 + 11.00 = 31.50 USD at 0.9215 → 29.03 € → items 11.06 + 7.83 + 10.14 €.
- **Balances.** `paid − owed` per member in base currency. For an income the signs flip: the
  receiver's balance goes down by the total and each share's balance goes up. A transfer
  raises the sender and lowers the receiver. Balances always sum to exactly zero.
- **Settle-up.** Greedy: repeatedly match the largest debtor with the largest creditor (ties
  by member id). At most `n − 1` transfers. Applying all suggestions zeroes every balance.
  Balances A +18.00, B −6.00, C −12.00 → C pays A 12.00, B pays A 6.00.
  Balances A +6.00, B +4.00, C −4.00, D −6.00 → D pays A 6.00, C pays B 4.00 (2 transfers; paying
  creditors one after another in list order would need 3).
  The result never depends on names, language or the order members were listed in.
- **Exact-amount splits** must add up to the item total; the editor shows the difference and
  blocks saving until it is zero.
- **Mixed splits:** fixed amounts first; the remainder (never negative) is split by parts.
- **Display.** A transaction in a foreign currency shows both the original amount and the converted
  amount, in lists and in details. Amount fields accept only as many decimals as the currency has.

## 5. Edge cases

- **Zero-amount expense:** allowed (placeholder), affects nothing.
- **Nobody included:** cannot save. Every item needs at least one member with a non-zero weight.
- **Payer not included:** allowed (paid for others only).
- **Rate:** must be positive. Changing the group's base currency does **not** rewrite old
  rates silently; the user is asked to confirm a re-conversion.
- **Rate entry:** the rate field shows a suggested market rate (needs the backend, M5) that the user
  can accept or overwrite; offline, the last rate used for that currency pair is suggested.
- **Overflow:** amounts beyond `i64` minor units are rejected, never wrapped.
- **Deactivated member:** keeps their balance; can still settle; cannot be picked for new
  splits.
- **Concurrent offline edits (M5):** last writer wins per field (Lamport clock), deletes are
  tombstones. (Not observable on one device; covered by the sync design in M5.)
- **Destructive or money-moving actions** (delete a transaction or person, mark a reimbursement as
  paid) ask for confirmation first.
- **Removing a member** keeps their past shares and their balance as they are; nothing is spread over
  the others (see flow 7).
- **Invite link turned off:** opening it shows "link no longer valid".
- **Receipt items with discounts (M7):** negative lines are folded into the item they apply to
  before splitting; a receipt never yields a negative item.

## 6. Out of scope (v1)

- Banking features: payment requests, payment links, card transactions, open banking, IBANs.
- Shared photo albums per group.
- Importing from other apps (may come after v1).
- Multiple payers in one transaction (use several transactions).
- Ads, in-app purchases and social logins beyond Google and Apple (to be decided before M8).
- Recurring expenses (open: check whether users expect them).
