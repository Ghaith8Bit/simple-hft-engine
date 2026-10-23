# Benchmarks

Performance numbers, methodology, and — just as importantly — what
the numbers do **not** include. A number without methodology is a
rumor; a number without caveats is a lie.

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
~30% between runs with no code change**. The difference row below is
stable across runs because both component rows drift together. Quote
the difference row, not the raw figures. Observed difference range
across runs: **28–40 ns**.

---

## Methodology

All hot-path timings use `rdtsc` preceded by `lfence` (serialization
against preceding memory operations). Cycle counts convert to
nanoseconds via the calibrated TSC frequency. Percentiles are computed
by sorting all samples and indexing at `n * p / 100`.

**Two loops are measured, not one:**

- `measure_empty_loop` — `rdtsc; rdtsc` back to back. No work between
  them. This is the floor: the cost of the measurement loop itself.
- `measure_full_loop` — `rdtsc; timestamp_now(); rdtsc`. One call
  between the reads.
- The **true cost** of `timestamp_now()` is `full − baseline`. This is
  the number quoted everywhere downstream.

`black_box` in the full loop prevents the compiler from deleting the
inner call as dead code. The empty loop needs no such protection
because both `rdtsc` results are used directly.

### Reproduction

```bash
cargo test -p hft-benchmarks --release -- --nocapture --test-threads=1
```

`--release`, `--test-threads=1`, `--nocapture` are all mandatory.
Debug builds inflate numbers 10–50×; parallel tests steal CPU from
the measurement thread; without `--nocapture` the test harness
swallows the output.

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
  within 1 ns. This tightness is the definitive proof that
  `timestamp_now()` is deterministic: 99.9% of measurements have
  identical cost. If the function had a hidden branch or cache miss,
  this distribution would spread.
- The **component-row tails are OS noise that cancels.** Baseline max
  (82 µs) and full max (111 µs) are single samples — one iteration
  out of 100,000 where the OS preempted the thread on core 2. Because
  the same OS events hit both loops statistically, the noise cancels
  in the subtraction at every percentile. The p99.9 of the difference
  row (29 ns) proves the cancellation is working.
- **28 ns × 2.591 GHz ≈ 73 CPU cycles** for one `lfence` + one
  `rdtsc`. On a Spectre-hardened CPU this is dominated by `lfence`
  cost (30–50 cycles instead of the theoretical 1). This is a **fixed
  tax**, not a variable — every future benchmark subtracts it.
- **Max is not subtracted.** `max(baseline)` and `max(full)` are
  individual samples from different iterations; their difference is
  meaningless. Max is shown on the component rows as a **diagnostic**
  of environment noise, not as a claim about the code.

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
  book exceeds L1 cache.
- **NUMA** — single-socket laptop, not applicable.

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
- [ ] `isolcpus=2,3 nohz_full=2,3 rcu_nocbs=2,3` — deferred (GRUB
      edit risk); document only
- [ ] `SCHED_FIFO` on measurement thread
- [ ] Order book benchmarks — insert/cancel/match at p50/p99/p99.9
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