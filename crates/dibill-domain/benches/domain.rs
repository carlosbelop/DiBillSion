//! Benchmarks at the scale the app must stay fast at: 1k members, 100k expenses.
//! Run with `cargo bench -p dibill-domain`.

use std::collections::BTreeMap;
use std::hint::black_box;

use criterion::{Criterion, criterion_group, criterion_main};
use dibill_domain::{Expense, ItemShare, MemberId, Minor, balances, settle_up, split_weighted};
use uuid::Uuid;

const MEMBERS: u64 = 1_000;
const EXPENSES: u64 = 100_000;
const PER_EXPENSE: u64 = 10;

/// Small deterministic generator (splitmix64), so runs are comparable.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
}

fn member(n: u64) -> MemberId {
    MemberId(Uuid::from_u128(u128::from(n)))
}

fn amount(rng: &mut Rng) -> Minor {
    Minor::try_from(rng.below(100_000) + 1).unwrap_or(1)
}

fn expenses(rng: &mut Rng) -> Vec<Expense> {
    (0..EXPENSES)
        .map(|_| {
            let first = rng.below(MEMBERS - PER_EXPENSE);
            let shares = (first..first + PER_EXPENSE)
                .map(|n| ItemShare::parts(member(n), 1))
                .collect();
            Expense::single(member(rng.below(MEMBERS)), amount(rng), shares)
        })
        .collect()
}

fn bench(c: &mut Criterion) {
    let mut rng = Rng(42);

    let everyone: Vec<(MemberId, u32)> = (0..MEMBERS).map(|n| (member(n), 1)).collect();
    c.bench_function("split_weighted/1k_members", |b| {
        b.iter(|| split_weighted(black_box(1_000_003), black_box(&everyone)));
    });

    let es = expenses(&mut rng);
    c.bench_function("balances/100k_expenses_1k_members", |b| {
        b.iter(|| balances(black_box(&es), &[]));
    });

    let mut net: BTreeMap<MemberId, Minor> = (0..MEMBERS)
        .map(|n| (member(n), amount(&mut rng) - 50_000))
        .collect();
    let sum: Minor = net.values().sum();
    *net.entry(member(0)).or_default() -= sum;
    c.bench_function("settle_up/1k_members", |b| {
        b.iter(|| settle_up(black_box(&net)));
    });
}

criterion_group!(benches, bench);
criterion_main!(benches);
