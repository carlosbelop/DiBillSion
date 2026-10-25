# Benchmarks

Run with `cargo bench -p dibill-domain` (criterion, `crates/dibill-domain/benches/domain.rs`).
Inputs are generated with a fixed seed, so numbers are comparable between runs on the same machine.

## Baseline (2026-10-25)

Intel Core i7-9750H, WSL2 Ubuntu 24.04, release profile.

| Benchmark | Input | Median |
|---|---|---|
| `split_weighted/1k_members` | 10,000.03 split equally among 1,000 members | 28 µs |
| `balances/100k_expenses_1k_members` | 100k single-item expenses, 10 members each | 151 ms |
| `settle_up/1k_members` | 1,000 random non-zero balances | 65 µs |

`balances` is about 1.5 µs per expense. Fine for real groups (hundreds to a few thousand expenses);
revisit if the store starts recomputing it on every keystroke.
