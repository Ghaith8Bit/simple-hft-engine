# simple-hft-engine

A limit order book and market data feed handler in Rust, built to be
**measured and reproducible**.

Most portfolio HFT projects show impressive numbers. This one shows
the numbers, the tool that produced each one, and what each one
does not include. That separation is the point.

---

## The numbers

| What | Time | How measured |
|---|---|---|
| `best_bid_read` | **3 ns** | RDTSC, baseline-subtracted |
| `timestamp_now()` | **28 ns** | RDTSC, baseline-subtracted |
| Feed message decode | **3 ns** | criterion `iter()` |
| Order book mutations | 63–79 ns | criterion `iter_batched` (upper bound) |

**Why the last column exists:** 3 ns from RDTSC baseline subtraction
is a measurement. 63 ns from criterion's `iter_batched` is a ceiling
— the real operation is smaller, but the tool cannot see below its
own overhead. Every number in [`BENCHMARKS.md`](BENCHMARKS.md) says
which kind it is and why. That is what separates a benchmark from a
claim.

---

## What is in here

- **`hft-core`** — types, `no_std`, zero dependencies
- **`hft-orderbook`** — 20-level fixed-depth LOB, sorted array, no allocation
- **`hft-feed`** — zero-copy `zerocopy` decode + UDP sender/receiver
- **`hft-benchmarks`** — RDTSC timer, TSC calibration, percentile stats

**33 tests. Zero warnings under `-D warnings`. ~1,500 lines of hot-path code.**

---

## What this project demonstrates

- **Measurement discipline** — I built the stopwatch before the car, and I know what the stopwatch costs (28 ns per read).
- **Documented failure** — I tried to measure mutations with RDTSC. It produced physically impossible results (an operation with more work reported faster than one with less). I stopped, switched to criterion, and labeled those numbers as upper bounds. The failure is in the repo.
- **Trade-off reasoning** — every design choice (fixed-depth array over `BTreeMap`, aggregated over per-order, `no_std` over `std`) has a written answer in [`PORTFOLIO.md`](PORTFOLIO.md).
- **Zero-copy discipline** — the decode path does not copy, parse, or allocate. A test asserts the decoded reference points into the original buffer. If the pointer moved, the test fails.
- **Layering the compiler enforces** — `hft-core` cannot allocate because it is `no_std`. Not "should not" — cannot. The compiler refuses.

---

## Try it

```bash
cargo test --workspace --release
cargo bench -p hft-orderbook --bench book_rdtsc
cargo bench -p hft-feed --bench decode
```

The order book bench prints `best_bid_read: p50 = 3 ns` on my machine.
On yours it will print a number close to that (firmware frequency
varies) — the methodology, not the specific figure, is what travels.

---

## Where to look next

- [`BENCHMARKS.md`](BENCHMARKS.md) — every number, its tool, its limits
- [`PORTFOLIO.md`](PORTFOLIO.md) — 11 design problems and how I solved them

If you read one section: **[`PORTFOLIO.md § Problem 2`](PORTFOLIO.md)** — where I tried to measure mutations with RDTSC, it failed, and I explain why that failure is in the repo.

---

## License

MIT — see [`LICENSE`](LICENSE).