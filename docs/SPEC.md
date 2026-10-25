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
- _TBD: currencies, rounding of FX, refunds/negative expenses_

## 5. Edge cases
_TBD_

## 6. Out of scope (v1)
_TBD_
