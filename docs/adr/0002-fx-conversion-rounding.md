# ADR 0002: FX conversion and rounding

- Status: accepted
- Date: 2026-10-25

## Context

Groups have one base currency, but expenses can be entered in any currency. Balances and settle-up
work on integer minor units of the base currency, so every foreign amount must be converted and rounded
exactly once, identically on every device.

## Decision

- `Currency` is an ISO 4217 code plus its minor-unit exponent (0–4), looked up in a built-in table of
  active codes. Codes without a minor unit (metals, XDR, test codes) are rejected.
- An `FxRate` is a positive `rust_decimal::Decimal` in **major units**: 1 unit of the source currency
  = `rate` units of the target currency. It is stored with the expense, not looked up live.
- `convert` computes `amount × rate × 10^(exp_to − exp_from)` in decimal and rounds once to the
  nearest minor unit of the target, **halves away from zero** (commercial rounding, as on receipts and
  bank statements). This is sign-symmetric, so a refund converts to exactly the negated expense.
- Overflow is an error, never a silent wrap or saturation.

## Consequences

- No floats anywhere in the path. Decimal arithmetic is deterministic across platforms.
- The converted amount is computed once per expense (not per share), then split with the
  largest-remainder method, so shares always sum to the converted total.
- Banker's rounding was rejected: it is less familiar to users and buys nothing when each amount is
  rounded only once.
