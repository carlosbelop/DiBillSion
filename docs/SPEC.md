# Functional specification

> Written in our own words from studying how group-expense apps behave (milestone M1).
> This is the only study output allowed in the repo: no code, strings, assets or API details
> from third-party apps.

## 1. Features
_TBD_

## 2. User flows
_TBD_

## 3. Data model
_TBD (starting point: docs/PLAN.md §3.4)_

## 4. Calculation rules (with worked examples)
- Split: largest-remainder method over integer cents, ties broken by member id.
  10.00 € split equally among A, B, C → A 3.34, B 3.33, C 3.33.
- Currency conversion: each foreign amount is converted once into the group's base currency with the
  rate stored on the expense, then rounded to the nearest minor unit, halves away from zero (ADR 0002).
  10.00 USD at 0.9215 → 9.215 € → 9.22 €. A refund of −10.00 USD converts to exactly −9.22 €.
  1,234 JPY at 0.0061 → 7.5274 € → 7.53 €.
- _TBD: refunds/negative expenses in splits_

## 5. Edge cases
_TBD_

## 6. Out of scope (v1)
_TBD_
