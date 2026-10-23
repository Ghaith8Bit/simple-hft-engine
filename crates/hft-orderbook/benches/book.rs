use criterion::{black_box, criterion_group, criterion_main, BatchSize, Criterion};
use hft_core::{Price, Quantity, Side};
use hft_orderbook::OrderBook;

/// Pre-populate a book with `n` bid levels starting at `base_price`,
/// descending by 2 per level (leaving room for mid-inserts at
/// odd-tick prices). Returned by value so `iter_batched` can consume
/// it per measurement iteration.
///
/// With `populated_book(10, 100)` the prices are:
/// `100, 98, 96, 94, 92, 90, 88, 86, 84, 82`
fn populated_book(n: i64, base_price: i64) -> OrderBook {
    let mut book = OrderBook::new();
    for i in 0..n {
        book.add_order(Side::Bid, Price(base_price - i * 2), Quantity(10));
    }
    book
}

fn bench_best_bid(c: &mut Criterion) {
    let book = populated_book(10, 100);
    c.bench_function("best_bid_read", |b| {
        b.iter(|| black_box(book.best_bid()));
    });
}

fn bench_add_same_price(c: &mut Criterion) {
    // Fast path: price 100 is at index 0. Binary search hits Ok(0),
    // no array shift, only qty/order_count updates.
    c.bench_function("add_same_price", |b| {
        b.iter_batched(
            || populated_book(10, 100),
            |mut book| {
                book.add_order(Side::Bid, Price(100), Quantity(1));
                black_box(book);
            },
            BatchSize::SmallInput,
        );
    });
}

fn bench_add_new_price_best(c: &mut Criterion) {
    // Worst-case shift: price 200 is better than all existing levels,
    // so it inserts at index 0. All 10 existing levels shift right.
    c.bench_function("add_new_price_best", |b| {
        b.iter_batched(
            || populated_book(10, 100),
            |mut book| {
                book.add_order(Side::Bid, Price(200), Quantity(1));
                black_box(book);
            },
            BatchSize::SmallInput,
        );
    });
}

fn bench_add_new_price_mid(c: &mut Criterion) {
    // Typical case: price 95 falls between 96 (index 2) and 94
    // (index 3), so it inserts at index 3. 7 existing levels shift
    // right; binary search runs full depth.
    c.bench_function("add_new_price_mid", |b| {
        b.iter_batched(
            || populated_book(10, 100),
            |mut book| {
                book.add_order(Side::Bid, Price(95), Quantity(1));
                black_box(book);
            },
            BatchSize::SmallInput,
        );
    });
}

fn bench_add_new_price_worst(c: &mut Criterion) {
    // New worst price into a not-full book: price 50 is worse than
    // 82 (index 9), so it writes at index 10 with no shift. Only
    // binary search + length bump.
    c.bench_function("add_new_price_worst", |b| {
        b.iter_batched(
            || populated_book(10, 100),
            |mut book| {
                book.add_order(Side::Bid, Price(50), Quantity(1));
                black_box(book);
            },
            BatchSize::SmallInput,
        );
    });
}

fn bench_cancel_partial(c: &mut Criterion) {
    // Level exists (price 100 at index 0) with qty 10. Cancel 5 leaves
    // qty 5; no shift, no removal.
    c.bench_function("cancel_partial", |b| {
        b.iter_batched(
            || populated_book(10, 100),
            |mut book| {
                let _ = book.cancel_order(Side::Bid, Price(100), Quantity(5));
                black_box(book);
            },
            BatchSize::SmallInput,
        );
    });
}

fn bench_cancel_full(c: &mut Criterion) {
    // Level exists (price 100 at index 0) with qty 10. Cancel 10 drops
    // qty to 0, removes the level, and shifts the remaining 9 levels
    // left.
    c.bench_function("cancel_full", |b| {
        b.iter_batched(
            || populated_book(10, 100),
            |mut book| {
                let _ = book.cancel_order(Side::Bid, Price(100), Quantity(10));
                black_box(book);
            },
            BatchSize::SmallInput,
        );
    });
}

criterion_group!(
    benches,
    bench_best_bid,
    bench_add_same_price,
    bench_add_new_price_best,
    bench_add_new_price_mid,
    bench_add_new_price_worst,
    bench_cancel_partial,
    bench_cancel_full,
);
criterion_main!(benches);
