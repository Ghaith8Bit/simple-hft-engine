# Benchmarks

Performance numbers, methodology, and — just as importantly — what
the numbers do **not** include. A number without methodology is a
rumor; a number without caveats is a lie.

For interview narratives that reference these measurements, see
[`PORTFOLIO.md`](PORTFOLIO.md).

---

## What is being measured

Two components exist:

- **`hft-benchmarks`** — RDTSC-based timing harness. Measures
  `timestamp_now()`, the primitive every future benchmark builds on.
- **`hft-orderbook`** — fixed-depth, price-aggregated limit order
  book. Measured by two tools: RDTSC for `best_bid_read` (honest
  per-op cost), criterion for mutations (upper bounds, regression
  baselines). See [Why two tools](#why-two-tools-for-the-order-book).

Everything else (feed handler, strategy, risk, gateway) does not yet
exist.

---

## Environment

| Property | Value |
|---|---|
| Machine | GHAITH-ROG (ASUS ROG laptop) |
| OS | Linux |
| CPU | x86_64, 12 logical cores (0–11), TSC present |
| Measured TSC frequency | **2591 MHz** (via 200 ms wall-clock calibration) |
| Power governor | **firmware-controlled** (no `cpufreq` driver active) |
| CPU isolation | none (`isolcpus` not applied) |
| Rust | stable (`rust-toolchain.toml` pinned) |
| Build profile | `--release` |

**CPU governor: not configurable on this machine.** `dmesg` shows
`intel_pstate: CPU model not supported`; no fallback driver loads.
Frequency is controlled by the ASUS ROG embedded controller, not by
Linux. On a dedicated server the `performance` governor would pin
the CPU to a fixed non-boost frequency and remove this variance. The
baseline subtraction below still isolates the cost of
`timestamp_now()` from the measurement loop, so the **relative**
numbers remain valid even though the **absolute** frequency is not
pinned.

Because the frequency drifts between runs (the firmware raises and
lowers boost based on thermal state), **absolute numbers vary by
~30% between runs with no code change**. The difference row in the
timestamp results is stable across runs because both component rows
drift together. Quote the difference row, not the raw figures.
Observed difference range across runs: **28–40 ns**.

---

## Methodology

### Timestamp overhead (`hft-benchmarks`)

All hot-path timings use `rdtsc` preceded by `lfence` (serialization
against preceding memory operations). Cycle counts convert to
nanoseconds via the calibrated TSC frequency. Percentiles are
computed by sorting all samples and indexing at `n * p / 100`.

**Two loops are measured, not one:**

- `measure_empty_loop` — `rdtsc; rdtsc` back to back. No work between
  them. This is the floor: the cost of the measurement loop itself.
- `measure_full_loop` — `rdtsc; timestamp_now(); rdtsc`. One call
  between the reads.
- The **true cost** of `timestamp_now()` is `full − baseline`. This
  is the number quoted everywhere downstream.

`black_box` in the full loop prevents the compiler from deleting the
inner call as dead code. The empty loop needs no such protection
because both `rdtsc` results are used directly.

Reproduce with:

```bash
cargo test -p hft-benchmarks --release -- --nocapture --test-threads=1
```

`--release`, `--test-threads=1`, `--nocapture` are all mandatory.
Debug builds inflate numbers 10–50×; parallel tests steal CPU from
the measurement thread; without `--nocapture` the test harness
swallows the output.

### Order book (`hft-orderbook`)

Two benchmarks, two tools:

- `benches/book.rs` — **criterion**. `iter()` for `best_bid_read`,
  `iter_batched()` for mutations. Statistical analysis, HTML
  reports, regression detection. ~55–60 ns of `iter_batched`
  overhead per iteration.
- `benches/book_rdtsc.rs` — **RDTSC**. Baseline-subtracted per-op
  timing for `best_bid_read` only. Honest number, no scaffolding.

Reproduce with:

```bash
cargo bench -p hft-orderbook --bench book        # criterion (upper bounds)
cargo bench -p hft-orderbook --bench book_rdtsc  # RDTSC (honest read cost)
```

**Criterion cannot measure mutations honestly.** `iter_batched` runs
a setup closure, times the routine, and drops the value inside the
timer — the scaffolding costs 55–60 ns per iteration, more than the
operation. The numbers are **regression baselines**, not performance
claims.

**RDTSC cannot measure mutations honestly either.** Batch-subtraction
of mutations was attempted and abandoned — the compiler consistently
found a difference between baseline-loop and full-loop setup costs
that dominated the operation. See
[Why two tools](#why-two-tools-for-the-order-book).

---

## Results: `timestamp_now()` cost

Pinned to core 2 via `sched_setaffinity`. 100,000 iterations per row.

| Path | p50 | p99 | p99.9 | max |
|---|---|---|---|---|
| baseline (`rdtsc; rdtsc`) | 26 ns | 30 ns | 35 ns | 82,098 ns |
| full (`rdtsc; now; rdtsc`) | 54 ns | 58 ns | 64 ns | 111,460 ns |
| **difference (true cost)** | **28 ns** | **28 ns** | **29 ns** | — |

**Reading these numbers:**

- The **difference row is the number to quote.** p50 = p99 = p99.9
  within 1 ns. This tightness proves `timestamp_now()` is
  deterministic: 99.9% of measurements have identical cost. If the
  function had a hidden branch or cache miss, this distribution
  would spread.
- The **component-row tails are OS noise that cancels.** Baseline max
  (82 µs) and full max (111 µs) are single samples — one iteration
  out of 100,000 where the OS preempted the thread on core 2.
  Because the same OS events hit both loops statistically, the noise
  cancels in the subtraction at every percentile. The p99.9 of the
  difference row (29 ns) proves the cancellation is working.
- **28 ns × 2.591 GHz ≈ 73 CPU cycles** for one `lfence` + one
  `rdtsc`. On a Spectre-hardened CPU this is dominated by `lfence`
  cost (30–50 cycles instead of the theoretical 1). This is a
  **fixed tax**, not a variable — every future benchmark subtracts
  it.
- **Max is not subtracted.** `max(baseline)` and `max(full)` are
  individual samples from different iterations; their difference is
  meaningless. Max is shown on the component rows as a **diagnostic**
  of environment noise, not as a claim about the code.

---

## Results: order book operations

Two tools, two capabilities. RDTSC measures `best_bid_read` honestly.
Criterion measures mutations with ~55–60 ns of overhead — upper
bounds, useful for regression detection.

| Operation | Time | Source | Trust |
|---|---|---|---|
| `best_bid_read` | **3 ns** (p50 = p99 = 3 ns, p99.9 = 8 ns) | RDTSC, baseline-subtracted | **Honest** |
| `add_same_price` | 78.7 ns | criterion `iter_batched` | Upper bound |
| `add_new_price_best` | 75.8 ns | criterion `iter_batched` | Upper bound |
| `add_new_price_mid` | 74.2 ns | criterion `iter_batched` | Upper bound |
| `add_new_price_worst` | 69.0 ns | criterion `iter_batched` | Upper bound |
| `cancel_partial` | 63.0 ns | criterion `iter_batched` | Upper bound |
| `cancel_full` | 70.7 ns | criterion `iter_batched` | Upper bound |

**Field-removal optimization measured.** Removing the semantically
broken `order_count: u32` from `PriceLevel` shrank it from 24 to 16
bytes and `OrderBook` from 976 to 656 bytes. Measured effects:

- `best_bid_read` 2.60 → 3 ns via RDTSC (single 128-bit load vs. two
  loads). The 1–3 ns range across runs reflects firmware-controlled
  frequency drift; the p99.9 tail (8 ns) is the load occasionally
  missing L1, which the previous 1/1/1 run did not expose.
- All mutations 95–110 → 63–79 ns criterion (−25 to −35%) — smaller
  struct means less stack memory shuttled by `iter_batched`'s
  setup/drop. The operation itself did not get 33% faster; the
  scaffolding did.

**Why 3/3/8 is the honest number, not 1/1/1.** `rdtsc` on x86 can be
reordered with ordinary loads: the `lfence` before `rdtsc` prevents
prior loads from moving, but does not prevent a subsequent load from
starting before the counter reads. When the load is very cheap and
the pipeline is unpressured, it can overlap with the second `rdtsc`
and appear to cost ~0 cycles. The 1/1/1 run was almost certainly
pipeline overlap, not a sub-cycle load. The 3/3/8 run had slightly
more pressure and could not fully overlap — it is the more honest
measurement, with a real tail instead of a suspiciously-perfect
flat distribution.

**Within-run deltas on criterion** reveal the shift cost, though
weakly:

- `cancel_partial` 63.0 vs `cancel_full` 70.7 → 7.7 ns for 9 shifted
  levels → **~0.85 ns per shifted `PriceLevel`**
- `add_new_price_worst` 69.0 vs `add_new_price_best` 75.8 → 6.8 ns
  for 10 shifted levels → **~0.68 ns per shift**

At `DEPTH = 20`, the worst-case shift (20 levels) costs ~14–17 ns on
top of the base operation. **This is the O(depth) insert cost a
fixed-depth array pays for O(1) reads.** Depth is a real tuning knob.

**Cross-benchmark comparisons at the ~5–10% level are confounded**
by firmware-controlled CPU frequency drift between benchmark runs
within the same criterion process. Only within-run deltas are
meaningful.

---

## Why two tools for the order book

`best_bid_read` is a load of `bids[0]` — 16 bytes, one `movdqa` on
x86_64. It has no side effects, no dependencies on prior state, and
the compiler cannot elide it because we `black_box` the result.
RDTSC measures it at 3 ns (p50 = p99), 8 ns at p99.9,
baseline-subtracted. That is honest.

Mutations are different. Each is a read-modify-write on a 656-byte
struct. Two measurement strategies were tried:

**Strategy 1 — criterion `iter_batched`.** Setup closure builds a
fresh `OrderBook` per iteration, drops it inside the timer. Overhead
~55–60 ns per iteration. The mutation definitely runs (criterion is
designed to guarantee this), but the number includes ~60 ns of
scaffolding. Usable as an upper bound and for regression detection.

**Strategy 2 — RDTSC batch-subtraction.** Baseline loop: `rdtsc;
setup(); rdtsc` × N. Full loop: `rdtsc; setup(); op(); rdtsc` × N.
Subtract totals, divide by N. Same methodology that worked for
`timestamp_now()`. It failed here:

- `cancel_full` (9 shifted levels) reported **faster** than
  `cancel_partial` (0 shifts) — physically impossible.
- `add_same_price` (fast path, no shift) reported **slower** than
  `add_new_price_worst` (no shift) — reversed.
- Two `black_box` defenses (seed from `rdtsc()`, fence between setup
  and window) were not enough. The compiler found a difference
  between the baseline-loop and full-loop setup costs — almost
  certainly cache-effect asymmetry — that dominated the operation.

The decision: **criterion for mutations, RDTSC for reads.** Making
the RDTSC defense strong enough would have required restructuring
the benchmark into something that no longer resembled the operation.
The failure and the decision are documented rather than hidden,
because a wrong number that looks real is worse than a right number
that is honestly labeled as an upper bound.

---

## What these numbers do NOT include

- **Kernel bypass** — no `io_uring`, `AF_XDP`, `DPDK`, `RDMA`. Any
  network path uses the Linux kernel stack (~tens of µs).
- **Market-data decoding** — the `zerocopy` decode path does not yet
  exist; decode latency will be measured separately from book update.
- **`isolcpus=2,3`** — not applied on this laptop (GRUB edit + reboot
  risk). On dedicated hardware: `isolcpus=2,3 nohz_full=2,3
  rcu_nocbs=2,3`. Expected to drop `max` from tens of µs to single-
  digit µs by removing the OS scheduler from measurement cores.
- **`SCHED_FIFO`** — measurement thread runs at normal priority.
  Real-time scheduling would prevent ordinary processes from
  preempting it.
- **Huge pages** — no `mmap(MAP_HUGETLB)`. Relevant once the order
  book exceeds L1 cache (currently 656 B, well within L1).
- **NUMA** — single-socket laptop, not applicable.
- **Mutation absolute latency** — criterion gives upper bounds only;
  RDTSC mutation batch was abandoned (see
  [Why two tools](#why-two-tools-for-the-order-book)).

Each item is a real cost a production HFT system would pay. Making
the delta explicit is the point of this section.

---

## Roadmap

- [x] RDTSC timing harness with `lfence` serialization
- [x] TSC frequency calibration against wall clock
- [x] Percentile statistics (p50/p90/p99/p99.9/max/mean)
- [x] Thread pinning via `sched_setaffinity`
- [x] Empty-loop baseline subtraction
- [x] Governor limitation documented (firmware-controlled)
- [x] Order book implemented: 20-level fixed-depth, sorted-array,
      binary search, aggregated by price
- [x] Field-removal optimization measured: `order_count` removed,
      24 → 16 byte `PriceLevel`
- [x] `best_bid_read` RDTSC-measured at 3 ns (p50 = p99), 8 ns
      (p99.9) (honest)
- [x] RDTSC batch measurement of mutations attempted and abandoned;
      criterion upper bounds documented as the alternative
- [ ] `isolcpus=2,3 nohz_full=2,3 rcu_nocbs=2,3` — deferred (GRUB
      edit risk); document only
- [ ] `SCHED_FIFO` on measurement thread
- [ ] Feed handler benchmarks — decode + book update in one pass
- [ ] End-to-end tick-to-trade

---

## Change log

- **2026-10-24** — Initial harness: TSC calibrated at 2591 MHz; smoke
  test measured 100,000 samples unpinned and pinned; pinning
  confirmed effective on tail (167 µs → 71 µs).
- **2026-10-24** — Added empty-loop baseline subtraction. True
  `timestamp_now()` cost reported as the difference between full and
  baseline loops: p50 = p99 = 28 ns, p99.9 = 29 ns. Tight
  distribution confirms the function is deterministic. Governor
  documented as firmware-controlled; absolute numbers drift ~30%
  between runs, difference row is stable.
- **2026-10-24** — Order book implemented. Removed broken
  `order_count` field (counted API calls, not orders); `PriceLevel`
  shrunk 24 → 16 bytes, `OrderBook` 976 → 656 bytes. Measured
  improvement: `best_bid_read` 2.60 → 1.93 ns criterion, mutations
  95–110 → 63–79 ns via criterion `iter_batched`. Per-shift cost
  ~0.68–0.85 ns; DEPTH=20 worst-case shift ~14–17 ns.
- **2026-10-24** — `best_bid_read` RDTSC benchmark added: 3 ns at
  p50 = p99, 8 ns at p99.9. Range across runs 1–3 ns (firmware
  frequency drift); p99.9 tail is real, the previous 1/1/1 run did
  not expose it. RDTSC batch measurement of mutations attempted
  and abandoned (compiler found setup-cost asymmetry between
  baseline and full loops that dominated the op; `cancel_full`
  reported faster than `cancel_partial` — impossible). Criterion
  remains the mutation baseline. Two tools, two capabilities,
  documented.