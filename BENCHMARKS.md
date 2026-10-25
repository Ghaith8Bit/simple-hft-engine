# Benchmarks

Numbers with methodology. Every number says what it measures and what
it excludes. For the reasoning behind the design choices, see
[`PORTFOLIO.md`](PORTFOLIO.md).

---

## Environment

| Property | Value |
|---|---|
| Machine | ASUS ROG laptop, Linux, 12 cores |
| TSC frequency | 2591 MHz (measured via wall-clock calibration) |
| Power governor | firmware-controlled (no `cpufreq` driver) |
| CPU isolation | none |
| Build profile | `--release` |

**Governor note:** `dmesg` shows `intel_pstate: CPU model not
supported`. Frequency is controlled by the ROG firmware, not Linux.
Absolute numbers drift ~30% between runs. The baseline subtraction
cancels this for the numbers we quote.

---

## Methodology

Three measurement tools, three purposes:

- **RDTSC** (`rdtsc` + `lfence`) — sub-nanosecond, cycle-level. Needs
  baseline subtraction because the read itself costs ~26 ns.
- **Baseline subtraction** — measure `rdtsc; empty; rdtsc` (floor),
  then `rdtsc; work; rdtsc` (full). Subtract. The difference is the
  honest cost of `work`.
- **Criterion** — statistical tool for regression detection. Its
  `iter_batched` overhead is ~55–60 ns, so numbers are upper bounds,
  not precise latencies. Good for catching slowdowns, not for
  claiming nanosecond figures.

Reproduce:

```bash
cargo test -p hft-benchmarks --release -- --nocapture --test-threads=1
cargo bench -p hft-orderbook --bench book
cargo bench -p hft-orderbook --bench book_rdtsc
cargo bench -p hft-feed --bench decode
```

---

## Results

### Timing harness — `timestamp_now()`

RDTSC, baseline-subtracted, 100k samples, pinned to core 2.

| Path | p50 | p99 | p99.9 |
|---|---|---|---|
| baseline (`rdtsc; rdtsc`) | 26 ns | 30 ns | 35 ns |
| full (`rdtsc; now; rdtsc`) | 54 ns | 58 ns | 64 ns |
| **true cost** | **28 ns** | **28 ns** | **29 ns** |

The tightness (28/28/29) is the proof: the function is deterministic,
no hidden branches. 28 ns × 2.591 GHz ≈ 73 cycles — dominated by
`lfence` cost under Spectre mitigations.

**This 28 ns is a fixed tax on every measurement below.** All further
benchmarks subtract it.

### Order book — read

RDTSC, baseline-subtracted, 100k samples.

| Operation | p50 | p99 | p99.9 |
|---|---|---|---|
| `best_bid_read` | 3 ns | 3 ns | 8 ns |

3 ns ≈ one 128-bit load. The 8 ns tail is the load occasionally
missing L1. **Honest number.**

### Order book — mutations

Criterion. Upper bounds only — `iter_batched` overhead dominates the
operation. Useful for regression detection, not precise latency.

| Operation | Time (upper bound) |
|---|---|
| `add_same_price` | 78.7 ns |
| `add_new_price_best` | 75.8 ns |
| `add_new_price_mid` | 74.2 ns |
| `add_new_price_worst` | 69.0 ns |
| `cancel_partial` | 63.0 ns |
| `cancel_full` | 70.7 ns |

**Within-run deltas** (criterion subtraction, legitimate because both
numbers share the same overhead):

- `cancel_full` − `cancel_partial` = 7.7 ns for 9 shifted levels → **~0.85 ns per shift**
- `add_new_price_best` − `add_new_price_worst` = 6.8 ns for 10 shifts → **~0.68 ns per shift**

Worst case (DEPTH=20): 20 × ~0.7 ns = **~14 ns** of shifting.
**Inferred, not directly measured.**

### Feed handler — decode

Criterion `iter()`, no setup overhead. 24-byte zero-copy messages via
`zerocopy::ref_from_prefix`.

| Operation | Time |
|---|---|
| `ref_from_bytes` floor (no dispatch) | 0.97 ns |
| `decode_cancel` | 2.35 ns |
| `decode_add` | 3.17 ns |
| `decode_stream_1000` (per message) | 2.50 ns |

Floor-to-full gap (1.4–2.2 ns) = type dispatch + side validation.
At 3 ns/msg → **1M msgs/sec uses 0.3% of one CPU core.**

---

## What is not measured

- **Mutations with RDTSC** — attempted and abandoned. The compiler
  defeated setup-subtraction (`cancel_full` reported faster than
  `cancel_partial`, impossible). Criterion upper bounds are the
  honest alternative. See [Roadmap](#roadmap).
- **Kernel bypass** — no `io_uring`, `AF_XDP`, `DPDK`.
- **`isolcpus` / `SCHED_FIFO`** — not applied (dev laptop, GRUB risk).
- **Huge pages, NUMA** — not applicable on this hardware.
- **End-to-end tick-to-trade** — not built yet (strategy stub).
- **Multi-instrument scaling** — single instrument only.

---

## Roadmap

- [x] RDTSC harness + TSC calibration + baseline subtraction
- [x] Thread pinning via `sched_setaffinity`
- [x] Order book (fixed-depth 20-level, sorted array, aggregated)
- [x] `order_count` field removed after semantic analysis (measured)
- [x] `best_bid_read` RDTSC-measured at 3 ns
- [x] Feed handler (zero-copy decode, 3 ns/msg)
- [x] UDP demo (sender + receiver, realistic drops)
- [ ] `isolcpus=2,3 nohz_full=2,3 rcu_nocbs=2,3` — production tuning, documented only
- [ ] `SCHED_FIFO` on measurement thread
- [ ] Per-order tracking (slab + intrusive list) — for real order identity
- [ ] End-to-end tick-to-trade pipeline
- [ ] Fuzz the decode path

---

## Change log

- **2026-10-24** — Timing harness: TSC 2591 MHz; `timestamp_now()` at 28 ns (p50 = p99 = p99.9) via baseline subtraction.
- **2026-10-24** — Order book: 20-level LOB; `order_count` removed (24→16 byte `PriceLevel`); mutations 63–79 ns (criterion upper bounds); `best_bid_read` at 3 ns (RDTSC); per-shift cost inferred ~0.7 ns.
- **2026-10-25** — Feed handler: zero-copy decode via `zerocopy`; `decode_add` 3.17 ns, `decode_cancel` 2.35 ns, stream 2.50 ns/msg; floor 0.97 ns; UDP demo added.