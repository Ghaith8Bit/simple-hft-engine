//! End-to-end pipeline: generate → decode → apply → strategy stub.
//!
//! Measures full-system throughput on a synthetic 100k-message stream.
//! Unlike the decoder-only benchmark (3 ns/msg), this number is what a
//! reviewer actually cares about — it includes decode, sequence
//! tracking, book update, and the strategy check.
//!
//! Run: cargo run -p hft-feed --release --bin pipeline

use hft_feed::apply::apply;
use hft_feed::decode::decode;
use hft_feed::replay::generate_stream;
use hft_feed::sequence::{SequenceEvent, SequenceTracker};
use hft_feed::wire::MSG_SIZE;
use hft_orderbook::OrderBook;
use std::time::Instant;

const N: usize = 100_000;
const SPREAD_THRESHOLD: i64 = 1;

fn main() {
    let stream = generate_stream(N);

    let mut book = OrderBook::new();
    let mut tracker = SequenceTracker::new();
    let mut decoded: u64 = 0;
    let mut applied: u64 = 0;
    let mut rejected: u64 = 0;
    let mut signals: u64 = 0;

    let start = Instant::now();

    let mut offset = 0;
    while offset + MSG_SIZE <= stream.len() {
        let slice = &stream[offset..offset + MSG_SIZE];
        offset += MSG_SIZE;

        let Ok(msg) = decode(slice) else {
            rejected += 1;
            continue;
        };
        decoded += 1;

        match tracker.observe(msg.seq()) {
            SequenceEvent::Normal | SequenceEvent::First => {}
            SequenceEvent::Gap(g) => {
                eprintln!(
                    "gap: expected seq {}, got {} — {} lost",
                    g.expected, g.actual, g.lost
                );
            }
            SequenceEvent::Duplicate(s) => eprintln!("duplicate seq {}", s),
            SequenceEvent::OutOfOrder { previous, actual } => {
                eprintln!("out-of-order: previous {}, got {}", previous, actual);
            }
        }

        apply(msg, &mut book);
        applied += 1;

        if let (Some(bid), Some(ask)) = (book.best_bid(), book.best_ask()) {
            if ask.price.0 - bid.price.0 > SPREAD_THRESHOLD {
                signals += 1;
            }
        }

        if applied.is_multiple_of(10_000) {
            let s = tracker.stats();
            eprintln!(
                "progress: {applied} msgs | bid={} ask={} | gaps={} lost={}",
                book.best_bid().map(|l| l.price.0).unwrap_or(0),
                book.best_ask().map(|l| l.price.0).unwrap_or(0),
                s.gaps,
                s.total_lost,
            );
        }
    }

    let elapsed = start.elapsed();
    let stats = tracker.stats();

    println!("pipeline: decode -> apply -> strategy");
    println!("decoded: {decoded}  applied: {applied}  rejected: {rejected}");
    println!("signals: {signals}  (spread > {SPREAD_THRESHOLD})");
    if let (Some(b), Some(a)) = (book.best_bid(), book.best_ask()) {
        println!(
            "book: bid={} ask={} spread={} depth={}x{}",
            b.price.0,
            a.price.0,
            a.price.0 - b.price.0,
            book.bid_depth(),
            book.ask_depth()
        );
    }
    println!(
        "sequence: gaps={} lost={} dup={} ooo={}",
        stats.gaps, stats.total_lost, stats.duplicates, stats.out_of_order
    );
    println!(
        "elapsed: {:.2?}  ({:.1}M msgs/sec)",
        elapsed,
        N as f64 / elapsed.as_secs_f64() / 1e6
    );
}
