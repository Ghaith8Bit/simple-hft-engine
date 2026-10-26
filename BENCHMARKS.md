# Benchmarks

Numbers with methodology. Every figure says what it measures and what
it excludes. For the design reasoning, see [`PORTFOLIO.md`](PORTFOLIO.md).

---

## Environment

| Property | Value |
| --- | --- |
| Machine | ASUS ROG laptop, Linux, 12 cores |
| TSC frequency | 2591 MHz (wall-clock calibrated) |
| Power governor | firmware-controlled (`intel_pstate` rejected the CPU) |
| CPU isolation | none |
| Build | `--release` |

Absolute numbers drift ~30% between runs because firmware controls
CPU frequency. Baseline subtraction cancels this for the numbers
quoted; raw figures do not.

---

## Methodology

- **RDTSC** — `rdtsc` + `lfence`, sub-nanosecond resolution. The read
  itself costs ~26 ns, so baseline subtraction is mandatory: measure
  `rdtsc; empty; rdtsc` and `rdtsc; work; rdtsc`, subtract per-
  percentile.
- **Criterion `iter()`** — no setup/drop overhead. Comparable to RDTSC.
- **Criterion `iter_batched`** — ~55–60 ns overhead. Numbers are
  upper bounds, useful for regression detection, not precise latency.
- **Pipeline** — `cargo run -p hft-feed --release --bin pipeline --
  --runs 5 --quiet`. Warmup run of 100k discarded, then median of 5
  timed runs. Progress logging suppressed during measurement.

Reproduce:

```bash
cargo test -p hft-benchmarks --release -- --nocapture --test-threads=1
cargo bench -p hft-orderbook --bench book
cargo bench -p hft-orderbook --bench book_rdtsc
cargo bench -p hft-feed --bench decode
cargo run -p hft-feed --release --bin pipeline -- --runs 5 --quiet
```

---

## Timing harness — `timestamp_now()`

RDTSC, baseline-subtracted, 100k samples, pinned to core 2.

| Path | p50 | p99 | p99.9 |
| --- | --- | --- | --- |
| baseline | 26 ns | 30 ns | 35 ns |
| full | 54 ns | 58 ns | 64 ns |
| **true cost** | **28 ns** | **28 ns** | **29 ns** |

p50 = p99 = p99.9 within 1 ns means the function is deterministic.
28 ns × 2.591 GHz ≈ 73 cycles, dominated by `lfence` under Spectre
mitigations.

---

## Order book — reads (RDTSC)

| Operation | p50 | p99 | p99.9 |
|---|---|---|---|
| `best_bid_read` | 1 ns | 2 ns | 6 ns |
| `best_ask_read` | 1 ns | 2 ns | 5 ns |

**Range across runs:** p50 = 1–3 ns, p99.9 = 5–8 ns. Low end is RDTSC
pipeline overlap (see PORTFOLIO § Problem 11); high end is an L1/L2
boundary hit. 3 ns is the conservative p50 to quote.

Both sides cost the same — the ask path uses no `.reverse()` in the
binary search.

---

## Order book — mutations (criterion `iter_batched`)

Upper bounds only. Criterion overhead ~55–60 ns dominates the
operation.

| Operation | Time (upper bound) |
| --- | --- |
| `add_same_price` (index 0, no shift) | 61.5 ns |
| `add_exist_price_update` (index 2, no shift) | 61.1 ns |
| `add_new_price_best` (insert at 0, 10-level shift) | 72.9 ns |
| `add_new_price_mid` (insert at 3, 7-level shift) | 66.3 ns |
| `add_new_price_worst` (append, no shift) | 66.1 ns |
| `cancel_partial` (reduce qty, no shift) | 61.4 ns |
| `cancel_full` (remove level, 9-level shift) | 65.9 ns |

**Within-run deltas** (both rows share the same overhead):

- `cancel_full` − `cancel_partial` = 4.5 ns for 9 shifts → **~0.5 ns per level**
- `add_new_price_best` − `add_new_price_worst` = 6.8 ns for 10 shifts → **~0.7 ns per level**

Worst case at `DEPTH = 20`: **~10–14 ns of shifting** on top of the
base operation. Depth is a real tuning knob.

**All mutation numbers are upper bounds.** The operation is smaller
— bounded below by shifting cost, above by criterion's overhead. The
honest figure needs working RDTSC batch mode, attempted and abandoned
(PORTFOLIO § Problem 2).

---

## Feed handler — decoder (criterion `iter()`)

| Operation | Time |
| --- | --- |
| `ref_from_bytes` floor (no dispatch) | 0.97 ns |
| `decode_cancel` | 2.35 ns |
| `decode_add` | 3.17 ns |
| `decode_stream_1000` (per message) | 2.50 ns |

Floor-to-full gap (1.4–2.2 ns) is the `match bytes[0]` dispatch plus
`side > 1` validation. No allocation, no parsing — decode borrows
from the input buffer.

---

## Feed handler — end-to-end pipeline

100,000 messages (2.4 MB in-memory stream) through decode → apply →
strategy. Method: `cargo run -p hft-feed --release --bin pipeline --
--runs 5 --quiet`. Warmup run of 100k discarded; progress logging
suppressed during timed runs.

| Metric | Value |
| --- | --- |
| Median | **65.0 M msgs/sec** |
| Range | 62.8 – 66.4 M |
| Run-to-run spread | 5.5% |

At the median, one message takes **~15 ns end-to-end** (decoder
~3 ns; the rest is book update, strategy check, and loop overhead).

**A single fresh-process run measures lower (~55 M msgs/sec).** The
first run in a process catches the CPU mid-boost; by run 3–5 the
frequency has stabilized at the boosted state. The 65 M figure is
the steady-state number; the ~55 M figure is what a reviewer sees if
they run `cargo run --bin pipeline` once. Both are real — they
measure different CPU states.

**This is cache-warm throughput, not network throughput.** The 2.4 MB
stream is L3-resident; the kernel UDP receive path is not measured
here — see `udp_demo`.

---

## Fuzz testing — decoder

`cargo-fuzz` + libFuzzer on `decode()`, arbitrary `&[u8]` input.

| Metric | Value |
| --- | --- |
| Duration | 61 s |
| Inputs | 18.5 M |
| Executions/sec | ~303 K |
| Crashes | 0 |
| Panics | 0 |

Target: `decode()` must not panic, must not read out of bounds, must
not loop forever on any input. Empty, 1, 23, 24, larger byte slices
all covered.

```bash
cd crates/hft-feed
cargo +nightly fuzz run decode
```

Fuzz crate is excluded from the workspace (nightly-only, `no_main`).

---

## What is not measured

- **Mutations with RDTSC.** Attempted; abandoned (PORTFOLIO § Problem 2).
- **Network receive path.** `udp_demo` exists as a demo; its latency
  is not benchmarked. Kernel UDP round-trip is ~tens of µs.
- **`isolcpus` / `SCHED_FIFO`.** Not applied (GRUB edit + reboot risk).
- **Huge pages, NUMA.** Not applicable on this hardware.
- **Strategy alpha.** The strategy is a stub.
- **Multi-instrument scaling.** Single instrument only.
