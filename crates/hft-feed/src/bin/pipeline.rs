//! End-to-end pipeline: generate → decode → apply → strategy stub.
//!
//! Usage:
//!   pipeline                 # 1 timed run, progress logging
//!   pipeline --quiet         # 1 timed run, no progress logging
//!   pipeline --runs 5 --quiet # 5 timed runs, report median + range
//!   pipeline --no-warmup     # skip the warmup run (for debugging)
//!
//! A warmup run of the full N iterations is performed before the timed
//! runs unless `--no-warmup` is passed.

use hft_feed::apply::apply;
use hft_feed::decode::decode;
use hft_feed::replay::generate_stream;
use hft_feed::sequence::{SequenceEvent, SequenceTracker};
use hft_feed::wire::MSG_SIZE;
use hft_orderbook::OrderBook;
use std::time::{Duration, Instant};

const N: usize = 100_000;
const SPREAD_THRESHOLD: i64 = 1;

struct RunResult {
    decoded: u64,
    applied: u64,
    rejected: u64,
    signals: u64,
    elapsed: Duration,
    gaps: u64,
    lost: u64,
    duplicates: u64,
    out_of_order: u64,
    final_bid: i64,
    final_ask: i64,
    final_bid_depth: usize,
    final_ask_depth: usize,
}

fn run_pipeline(n: usize, verbose: bool) -> RunResult {
    let stream = generate_stream(n);
    let mut book = OrderBook::new();
    let mut tracker = SequenceTracker::new();
    let mut decoded = 0u64;
    let mut applied = 0u64;
    let mut rejected = 0u64;
    let mut signals = 0u64;

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
                if verbose {
                    eprintln!(
                        "gap: expected seq {}, got {} — {} lost",
                        g.expected, g.actual, g.lost
                    );
                }
            }
            SequenceEvent::Duplicate(s) => {
                if verbose {
                    eprintln!("duplicate seq {}", s);
                }
            }
            SequenceEvent::OutOfOrder { previous, actual } => {
                if verbose {
                    eprintln!("out-of-order: previous {}, got {}", previous, actual);
                }
            }
        }

        apply(msg, &mut book);
        applied += 1;

        if let (Some(bid), Some(ask)) = (book.best_bid(), book.best_ask()) {
            if ask.price.0 - bid.price.0 > SPREAD_THRESHOLD {
                signals += 1;
            }
        }

        if verbose && applied.is_multiple_of(10_000) {
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
    let s = tracker.stats();

    RunResult {
        decoded,
        applied,
        rejected,
        signals,
        elapsed,
        gaps: s.gaps,
        lost: s.total_lost,
        duplicates: s.duplicates,
        out_of_order: s.out_of_order,
        final_bid: book.best_bid().map(|l| l.price.0).unwrap_or(0),
        final_ask: book.best_ask().map(|l| l.price.0).unwrap_or(0),
        final_bid_depth: book.bid_depth(),
        final_ask_depth: book.ask_depth(),
    }
}

fn parse_flag_value(args: &[String], flag: &str) -> Option<String> {
    args.iter()
        .position(|a| a == flag)
        .and_then(|i| args.get(i + 1).cloned())
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let quiet = args.iter().any(|a| a == "--quiet");
    let no_warmup = args.iter().any(|a| a == "--no-warmup");
    let runs: usize = parse_flag_value(&args, "--runs")
        .and_then(|s| s.parse().ok())
        .unwrap_or(1);

    if !no_warmup {
        if !quiet {
            eprintln!("warmup: {} msgs (discarded)", N);
        }
        let _ = run_pipeline(N, false);
    }

    let mut speeds: Vec<f64> = Vec::with_capacity(runs);
    let mut last: Option<RunResult> = None;
    for i in 0..runs {
        let r = run_pipeline(N, !quiet);
        let speed = N as f64 / r.elapsed.as_secs_f64() / 1e6;
        speeds.push(speed);
        if !quiet && runs > 1 {
            eprintln!("run {}: {:.1}M msgs/sec", i + 1, speed);
        }
        last = Some(r);
    }

    let r = last.expect("at least one run");
    println!("pipeline: decode -> apply -> strategy");
    println!(
        "decoded: {}  applied: {}  rejected: {}",
        r.decoded, r.applied, r.rejected
    );
    println!("signals: {}  (spread > {})", r.signals, SPREAD_THRESHOLD);
    println!(
        "book: bid={} ask={} spread={} depth={}x{}",
        r.final_bid,
        r.final_ask,
        r.final_ask - r.final_bid,
        r.final_bid_depth,
        r.final_ask_depth
    );
    println!(
        "sequence: gaps={} lost={} dup={} ooo={}",
        r.gaps, r.lost, r.duplicates, r.out_of_order
    );

    speeds.sort_by(|a, b| a.partial_cmp(b).unwrap());
    if runs == 1 {
        println!("elapsed: {:.2?}  ({:.1}M msgs/sec)", r.elapsed, speeds[0]);
    } else {
        let median = speeds[runs / 2];
        println!(
            "{} runs: median {:.1}M msgs/sec, range [{:.1}M, {:.1}M]",
            runs,
            median,
            speeds[0],
            speeds[runs - 1]
        );
    }
}
