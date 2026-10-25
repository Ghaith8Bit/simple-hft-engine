//! RDTSC order-book read benchmark. Measures `best_bid` and `best_ask`
//! load costs. Both are single-load reads with no setup, no drop, no
//! state change — so RDTSC + baseline subtraction gives a meaningful
//! per-op number.
//!
//! Mutation benchmarking with RDTSC was attempted and abandoned: the
//! compiler consistently found a difference between baseline and full
//! loop setup costs that dominated the operation. Criterion's
//! `iter_batched` numbers (upper bounds) are used for mutations
//! instead. See BENCHMARKS.md.
//!
//! Run with: cargo bench -p hft-orderbook --bench book_rdtsc

use hft_benchmarks::affinity::pin_to_core;
use hft_benchmarks::stats::Stats;
use hft_benchmarks::timing::{calibrate_tsc_frequency, cycles_to_ns, rdtsc};
use hft_core::{Price, Quantity, Side};
use hft_orderbook::OrderBook;
use std::hint::black_box;

const N: usize = 100_000;

/// Measure one read operation against a fixed book. Baseline is
/// `rdtsc; rdtsc` (no work). Full is `rdtsc; read; rdtsc`. The
/// per-percentile difference is the honest cost of `read`.
fn measure_read<F>(book: &OrderBook, mut read: F, freq: u64, label: &str)
where
    F: FnMut(&OrderBook),
{
    let mut baseline = Vec::with_capacity(N);
    let mut full = Vec::with_capacity(N);

    for _ in 0..N {
        let s = rdtsc();
        let e = rdtsc();
        baseline.push(e - s);

        let s = rdtsc();
        read(book);
        let e = rdtsc();
        full.push(e - s);
    }

    baseline.sort_unstable();
    full.sort_unstable();
    let b = Stats::from_sorted(&baseline);
    let f = Stats::from_sorted(&full);

    let p50 = cycles_to_ns(f.p50.saturating_sub(b.p50), freq);
    let p99 = cycles_to_ns(f.p99.saturating_sub(b.p99), freq);
    let p999 = cycles_to_ns(f.p999.saturating_sub(b.p999), freq);

    println!("{} (RDTSC, baseline-subtracted):", label);
    println!("  p50   = {} ns", p50);
    println!("  p99   = {} ns", p99);
    println!("  p99.9 = {} ns", p999);
}

fn main() {
    let freq = calibrate_tsc_frequency();
    if freq == 0 {
        eprintln!("TSC calibration failed");
        std::process::exit(1);
    }
    if let Err(e) = pin_to_core(2) {
        eprintln!("pin_to_core(2) failed: {} — continuing unpinned", e);
    }

    println!("TSC: {} MHz", freq / 1_000_000);
    println!();

    // Two-sided book: bids at 100, 98, ..., 82; asks at 102, 104, ..., 120.
    // Ensures best_bid() and best_ask() both return Some.
    let mut book = OrderBook::new();
    for i in 0..10i64 {
        book.add_order(Side::Bid, Price(100 - i * 2), Quantity(10));
        book.add_order(Side::Ask, Price(102 + i * 2), Quantity(10));
    }
    let book = black_box(book);

    measure_read(
        &book,
        |b| {
            black_box(b.best_bid());
        },
        freq,
        "best_bid_read",
    );
    println!();
    measure_read(
        &book,
        |b| {
            black_box(b.best_ask());
        },
        freq,
        "best_ask_read",
    );
}
