# simple-hft-engine

Limit order book and zero-copy market data feed handler in Rust, with
an end-to-end pipeline and a custom RDTSC benchmark harness.

Most HFT showcase projects show numbers. This one shows the numbers,
the tool that produced each one, and what each one excludes.

---

## Headline numbers

| What | Time | Tool |
|---|---|---|
| `best_bid` / `best_ask` read | 1–3 ns (5–8 ns p99.9) | RDTSC, baseline-subtracted |
| `timestamp_now()` | 28 ns | RDTSC, baseline-subtracted |
| Feed message decode | 3.17 ns | criterion `iter()` |
| Order book mutations | 60–73 ns | criterion `iter_batched` (upper bound) |
| End-to-end pipeline | **67M msgs/sec** (37–76M range) | `Instant`, 100k in-memory stream |
| Fuzz testing | 18.5M inputs, 61s, no crashes | `cargo-fuzz` + libFuzzer |

**Why a tool column:** RDTSC gives a direct measurement (needs baseline
subtraction). Criterion `iter_batched` gives an upper bound — its
55–60 ns overhead dominates the operation. Every figure in
[`BENCHMARKS.md`](BENCHMARKS.md) says which kind it is.

---

## What is in here

| Crate | What |
|---|---|
| [`hft-core`](crates/hft-core) | `Price`, `Quantity`, `Timestamp`, `Side`. `no_std`, zero deps. |
| [`hft-orderbook`](crates/hft-orderbook) | 20-level fixed-depth LOB, sorted array, aggregated by price. |
| [`hft-feed`](crates/hft-feed) | Zero-copy ITCH-style decoder, sequence tracking, UDP demo, pipeline binary. |
| [`hft-benchmarks`](crates/hft-benchmarks) | RDTSC timer, TSC calibration, thread pinning, percentile stats. |

**38 tests. Zero warnings under `-D warnings`. Fuzz-tested decoder. CI green.**

---

## Try it

```bash
cargo test --workspace --release
cargo run -p hft-feed --release --bin pipeline
cargo bench -p hft-orderbook --bench book_rdtsc
cargo bench -p hft-feed --bench decode
```

`pipeline` runs 100,000 synthetic messages through decode → apply →
strategy stub and prints end-to-end throughput.

---

## Design

- **No async runtime.** Single-threaded hot path, pinned to one core.
- **No heap allocation** in `hft-core` or `hft-orderbook` (`no_std`).
- **Zero-copy decode** via `zerocopy` — borrows from the input buffer.
  A test asserts the decoded reference points into the source buffer.
- **Sequence-number gap detection** in `udp_demo` — logs missing,
  duplicate, and out-of-order packets.
- **Two measurement tools.** RDTSC for read costs. Criterion for
  mutation regression baselines. The crossover is documented.

---

## Documentation

- [`BENCHMARKS.md`](BENCHMARKS.md) — every number, its tool, its limits
- [`PORTFOLIO.md`](PORTFOLIO.md) — 14 design problems and how they were solved

If you read one section: **[`PORTFOLIO.md § Problem 2`](PORTFOLIO.md)**
— where measuring mutations with RDTSC failed, and why that failure
is documented rather than hidden.

---

## License

MIT — see [`LICENSE`](LICENSE).