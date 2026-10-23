# Benchmarks

This file records every performance number the project claims, the
methodology that produced it, and — just as importantly — what the
numbers do **not** include. A number without methodology is a rumor;
a number without caveats is a lie. Both live here.

---

## What is being measured right now

At this stage of the project, nothing trading-related exists yet. The
only thing under measurement is the **timing harness itself** — the
RDTSC-based stopwatch that every future benchmark will rely on. If the
stopwatch is wrong, every downstream number is wrong too, so it gets
validated first.

The smoke test answers one question: *is the measurement infrastructure
trustworthy on this machine?* It does not yet answer *how fast is the
order book?* — that comes in a later stage.

---

## Environment

| Property | Value |
|---|---|
| Machine | GHAITH-ROG (ASUS ROG laptop) |
| OS | Linux |
| CPU | x86_64, TSC present |
| Measured TSC frequency | **2591 MHz** (via 200 ms wall-clock calibration) |
| Power governor | **default** (not `performance`) — see Roadmap |
| CPU isolation | **none** (no `isolcpus`) — see Roadmap |
| Rust | stable (`rust-toolchain.toml` pinned) |
| Build profile | `--release` |

The TSC frequency is measured at runtime, not assumed from the CPU's
marketing nameplate. CPU vendors report a "nominal" frequency that
often differs from the actual rate the TSC ticks at. Calibrating
against a wall clock gives the true value on *this* machine.

---

## Methodology

All hot-path timings use **`rdtsc`** (Read Time-Stamp Counter) preceded
by **`lfence`** to serialize the read against preceding memory
operations. This gives cycle-resolution timing at sub-nanosecond
precision, without the ~20–30 ns of overhead that `Instant::now()`
incurs through the vDSO.

Cycle counts are converted to nanoseconds via the calibrated TSC
frequency. Percentiles are computed by sorting all samples and
indexing at `n * p / 100`.

The full loop for a single sample is:

```
start = rdtsc()
ts    = timestamp_now()
end   = rdtsc()
sample = end - start
```

**The per-sample value includes the measurement overhead.** A single
sample costs three `rdtsc` reads (with their `lfence` fences), one
`black_box`, one `Vec::push`, and loop bookkeeping — not just the
cost of `timestamp_now()`. The `p50` printed below is therefore an
**upper bound** on the cost of `timestamp_now()`, not a direct
measurement of it. Baseline subtraction is on the Roadmap.

### Reproduction

```bash
cargo test -p hft-benchmarks --release -- --nocapture --test-threads=1
```

`--release` is mandatory: debug builds insert bounds checks and skip
optimizations, inflating every number by 10–50×.

`--test-threads=1` is mandatory: parallel tests steal CPU from the
measurement thread and pollute every percentile.

`--nocapture` is required to see the `println!` output; otherwise the
test harness swallows it.

---

## Results: `timestamp_now()` overhead

Sampled 100,000 iterations, once unpinned and once pinned to core 2
via `sched_setaffinity`.

| Condition | p50 | p99 | max |
|---|---|---|---|
| unpinned | 78 ns | 83 ns | 167,524 ns |
| pinned to core 2 | 78 ns | 83 ns | **71,521 ns** |

**Reading these numbers:**

- **`p50 = 78 ns`** is the median cost of *one measurement tick*, not of
  `timestamp_now()` alone. Includes 3× `rdtsc`+`lfence`, `black_box`,
  `Vec::push`, and loop bookkeeping.
- **`p50 = p99`** across pinned and unpinned: the median and 99th
  percentile are dominated by a deterministic instruction sequence and
  are unaffected by core migration. Migration events are rare enough
  (single-digit counts in 100,000 samples) that they do not move p99.
- **`max` collapses 2.3× with pinning** (167 µs → 71 µs). This
  confirms that `sched_setaffinity` is doing what it should: it
  removes *migrations*, which are one class of OS-driven preemption.
- **`max` after pinning is still 71 µs.** This is *not* a bug. Pinning
  restricts which core the thread may run on; it does **not** restrict
  what *else* may run on that core. Scheduler ticks, hardware
  interrupts, and other processes still preempt the thread. Kernel
  isolation (`isolcpus`) is required to eliminate that — see Roadmap.

**Do not quote `p50 = 78 ns` as the cost of reading the TSC.** That
number is coming, but it requires the empty-loop baseline that has not
yet been implemented. Until then, the numbers above are a
**methodology validation**, not a performance claim.

---

## What these numbers do NOT include

Every benchmark write-up is judged as much by what it excludes as by
what it reports. The following are **outside** the current
measurement:

- **Kernel bypass.** No `io_uring`, `AF_XDP`, `DPDK`, or `RDMA`. Any
  network path goes through the Linux kernel stack and its associated
  latency (tens of microseconds, not nanoseconds).
- **Market-data decoding.** The `zerocopy` decode path does not yet
  exist. When it does, decode latency will be measured separately from
  book-update latency.
- **`performance` CPU governor.** The CPU is running at its default
  power-management state. Switching to `cpupower frequency-set -g
  performance` typically reduces `p50` by 20–40% because it prevents
  the CPU from downclocking between samples.
- **CPU isolation (`isolcpus`).** The measurement thread shares cores
  with the OS. Kernel-level isolation would drop `max` from tens of
  microseconds to single-digit microseconds.
- **`SCHED_FIFO`.** The measurement thread runs at normal priority.
  Real-time scheduling would prevent ordinary processes from preempting
  it.
- **Empty-loop baseline.** Not yet subtracted (see Roadmap).
- **Huge pages.** No `mmap(MAP_HUGETLB)`. Relevant for the order book
  once its memory footprint exceeds L1.
- **NUMA awareness.** Single-socket laptop; not applicable yet.

Each of these is a real cost that will be paid by a production HFT
system. The purpose of listing them is not to excuse the numbers — it
is to make the delta between "what this showcase measures" and "what
a real deployment would need" explicit and honest.

---

## Roadmap

Ordered by expected signal-to-effort ratio. Each item is a committed
follow-up, not a wishlist entry.

- [x] **RDTSC timing harness** with `lfence` serialization
- [x] **TSC frequency calibration** against wall clock
- [x] **Percentile statistics** (p50/p90/p99/p99.9/max/mean)
- [x] **Thread pinning** via `sched_setaffinity`
- [ ] **Empty-loop baseline subtraction** — replace raw `p50 = 78 ns`
      with the true cost of `timestamp_now()`
- [ ] **`performance` CPU governor** — document the shell incantation
      and re-run; expect p50 to drop 20–40%
- [ ] **`isolcpus=2,3` kernel parameter** — isolate measurement cores
      from the scheduler; expect max to drop below 5 µs
- [ ] **`SCHED_FIFO` scheduling** — prevent ordinary processes from
      preempting the measurement thread
- [ ] **Order book benchmarks** — insert/cancel/match at p50/p99
- [ ] **Feed handler benchmarks** — decode + book update in one pass
- [ ] **End-to-end tick-to-trade** — the headline number

---

## Change log

- **2026-10-24** — Initial harness. TSC calibrated at 2591 MHz on
  GHAITH-ROG. Smoke test measured at 100,000 samples, unpinned and
  pinned to core 2. Pinning confirmed effective on `max` (167 µs →
  71 µs). Baseline subtraction, governor tuning, and `isolcpus` are
  scheduled as follow-ups before the order book is measured.