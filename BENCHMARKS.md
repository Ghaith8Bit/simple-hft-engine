# Benchmarks

Every number below is taken from the repository's existing benchmark notes. This page separates the measured result from what the measurement can support.

> **Read the method before quoting the number.** These are measurements from one laptop environment, not universal hardware guarantees.

## At a glance

```mermaid
flowchart LR
    A["Choose operation"] --> B["Choose measurement tool"]
    B --> C["Collect samples / timed runs"]
    C --> D["Report statistic"]
    D --> E["Read caveats before comparing"]
```

| Workload | Reported result | Tool | How to interpret it |
|---|---:|---|---|
| `best_bid_read` / `best_ask_read` | p50 1–3 ns; p99.9 5–8 ns across runs | RDTSC with baseline subtraction | Tiny read cost; measurement overlap and cache effects matter |
| `timestamp_now()` | 28 ns p50; 28 ns p99; 29 ns p99.9 | RDTSC with baseline subtraction | Reported cost for this environment and method |
| `decode_add` | 3.17 ns | Criterion `iter()` | Decode benchmark on a prebuilt message |
| `decode_cancel` | 2.35 ns | Criterion `iter()` | Decode benchmark on a prebuilt message |
| `decode_stream_1000` | 2.50 ns per message | Criterion `iter()` | Repeated decode over a prebuilt synthetic stream |
| Order-book mutations | 61.1–72.9 ns | Criterion `iter_batched` | Upper bounds; setup/drop overhead is roughly 55–60 ns |
| Pipeline | 68.1M messages/sec median; range 66.1–70.2M | `Instant`, 100,000-message in-memory stream | Cache-warm synthetic throughput, not network throughput |
| Decoder fuzz run | 18.5M inputs in 61 seconds; 0 crashes, 0 panics | `cargo-fuzz` + libFuzzer | One recorded run; not a proof that all possible bugs are absent |

## Environment recorded in the repository

| Property | Recorded value |
|---|---|
| Machine | ASUS ROG laptop, Linux, 12 cores |
| TSC frequency | 2591 MHz, wall-clock calibrated |
| Power governor | Firmware-controlled; `intel_pstate` rejected the CPU |
| CPU isolation | None |
| Build | `--release` |

The repository notes roughly 30% absolute-number drift between runs because firmware controls CPU frequency. Baseline subtraction is used for the quoted RDTSC values; raw values are not treated as directly comparable.

## How the measurements work

### RDTSC — small read/timing operations

The harness uses `rdtsc` with `lfence`. It measures an empty baseline (`rdtsc; rdtsc`) and a full path (`rdtsc; work; rdtsc`), then subtracts matching percentiles.

The read itself has material cost, so baseline subtraction is essential. The repository documents possible pipeline overlap and cache-boundary effects for order-book reads.

### Criterion `iter()`

Used for feed decoder benchmarks. The message buffer or stream is prepared before the measured closure, avoiding per-iteration setup in these benchmarks.

### Criterion `iter_batched`

Used for order-book mutations. The recorded harness overhead is about 55–60 ns, which dominates the operations. The values are therefore **upper bounds**, useful as regression baselines but not precise mutation latency.

The repository records that an RDTSC mutation approach was attempted and abandoned because baseline/full-loop setup differences dominated the operation and produced implausible comparisons.

### End-to-end synthetic pipeline

The pipeline in `crates/hft-feed/src/bin/pipeline.rs` generates a stream of 100,000 aligned messages, then times the loop that decodes each message, observes its sequence number, applies it to the order book, and performs a spread-threshold check. Unless `--no-warmup` is passed, a full warmup run is discarded. With multiple runs, the code sorts throughput values and reports the median and range; for an even number of runs it averages the two middle values.

The documented command is:

```bash
cargo run -p hft-feed --release --bin pipeline -- --runs 5 --quiet
```

The recorded stream is about 2.4 MB and is described in the repository as L3-resident/cache-warm. The kernel UDP receive path is not part of this measurement. The repository notes thermal variation and reports a median of 68.1M messages/sec with a 66.1–70.2M range for the recorded five-run setup.

## Detail tables

### `timestamp_now()`

| Path | p50 | p99 | p99.9 |
|---|---:|---:|---:|
| Baseline | 26 ns | 30 ns | 35 ns |
| Full | 54 ns | 58 ns | 64 ns |
| **Baseline-subtracted cost** | **28 ns** | **28 ns** | **29 ns** |

### Order-book reads

| Operation | p50 | p99 | p99.9 |
|---|---:|---:|---:|
| `best_bid_read` | 1 ns | 2 ns | 6 ns |
| `best_ask_read` | 1 ns | 2 ns | 5 ns |

Across runs, the documented range is p50 1–3 ns and p99.9 5–8 ns. The repository recommends 3 ns as the conservative p50 to quote, and notes that the low end can reflect RDTSC pipeline overlap while the high end can reflect an L1/L2 boundary hit.

### Order-book mutations — upper bounds only

| Operation | Time (upper bound) |
|---|---:|
| `add_same_price` | 61.5 ns |
| `add_exist_price_update` | 61.1 ns |
| `add_new_price_best` | 72.9 ns |
| `add_new_price_mid` | 66.3 ns |
| `add_new_price_worst` | 66.1 ns |
| `cancel_partial` | 61.4 ns |
| `cancel_full` | 65.9 ns |

The repository derives within-run differences of 4.5 ns between full and partial cancel for nine shifts, and 6.8 ns between best-price insert and worst-price append for ten shifts. These are differences between upper-bound measurements, not isolated precision latency claims.

### Feed decoder

| Operation | Time |
|---|---:|
| `ref_from_bytes` floor (no dispatch) | 0.97 ns |
| `decode_cancel` | 2.35 ns |
| `decode_add` | 3.17 ns |
| `decode_stream_1000` (per message) | 2.50 ns |

The repository attributes the floor-to-full difference to message-type dispatch and side validation. The decoder returns a reference into the input buffer instead of allocating a decoded copy.

### Recorded decoder fuzz run

| Metric | Recorded value |
|---|---:|
| Duration | 61 seconds |
| Inputs | 18.5 million |
| Executions/sec | about 303,000 |
| Crashes | 0 |
| Panics | 0 |

The fuzz target feeds arbitrary byte slices to `decode()`. The stated target is to avoid panics, out-of-bounds reads, and nontermination for arbitrary input. This is a record of one run, not a guarantee that the decoder is bug-free.

## Reproduce

```bash
cargo test -p hft-benchmarks --release -- --nocapture --test-threads=1
cargo bench -p hft-orderbook --bench book
cargo bench -p hft-orderbook --bench book_rdtsc
cargo bench -p hft-feed --bench decode
cargo run -p hft-feed --release --bin pipeline -- --runs 5 --quiet
```

Fuzzing (nightly + `cargo-fuzz`):

```bash
cd crates/hft-feed
cargo +nightly fuzz run decode
```

## What is not measured

- Order-book mutation latency with RDTSC; that approach was abandoned.
- UDP receive-path latency or network throughput; `udp_demo` is a separate demo.
- `isolcpus` / `SCHED_FIFO` tuning.
- Huge pages or NUMA behavior.
- Strategy alpha; the strategy is a stub.
- Multi-instrument scaling; the documented setup is single-instrument.

