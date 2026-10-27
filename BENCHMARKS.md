# Benchmarks

## Reproduction

Run from the repository root:

```bash
cargo test -p hft-benchmarks --release -- --nocapture --test-threads=1
cargo bench -p hft-orderbook --bench book
cargo bench -p hft-orderbook --bench book_rdtsc
cargo bench -p hft-feed --bench decode
cargo run -p hft-feed --release --bin pipeline -- --runs 5 --quiet
cargo bench --workspace --no-run
```

## Recorded results

These are recorded observations from the development environment, not universal guarantees.

### In-memory pipeline

| Measurement | Result |
| --- | ---: |
| Median throughput | 68.1 million messages/s |
| Five-run range | 66.1–70.2 million messages/s |
| Messages per run | 100,000 synthetic messages |
| Approximate stream size | 2.4 MB |

The stream is prepared before timing and is cache-warm. This is not network throughput.

### Decoder

| Benchmark | Recorded result |
| --- | ---: |
| `ref_from_bytes_add_floor` | 0.97 ns |
| `decode_cancel` | 2.35 ns |
| `decode_add` | 3.17 ns |
| `decode_stream_1000` | 2.50 ns/message |

Check the current benchmark harness and reporting convention before comparing these values.

### Order-book reads

| Operation | p50 | p99 | p99.9 |
| --- | ---: | ---: | ---: |
| Best bid read | 1 ns | 2 ns | 6 ns |
| Best ask read | 1 ns | 2 ns | 5 ns |

These very small RDTSC measurements are sensitive to methodology and should be treated as estimates.

### Order-book mutations

These figures are upper bounds because setup/teardown may contribute to the measured interval.

| Operation | Recorded upper bound |
| --- | ---: |
| Add at same price | 61.5 ns |
| Existing-price update case | 61.1 ns |
| Add new best level | 72.9 ns |
| Add new middle level | 66.3 ns |
| Add new worst level | 66.1 ns |
| Partial cancel | 61.4 ns |
| Full cancel | 65.9 ns |

### Timestamp timing

| Measurement | p50 | p99 | p99.9 |
| --- | ---: | ---: | ---: |
| Back-to-back RDTSC baseline | 26 ns | 30 ns | 35 ns |
| Full timestamp measurement | 54 ns | 58 ns | 64 ns |
| Baseline-subtracted estimate | 28 ns | 28 ns | 29 ns |

Baseline subtraction is an estimate and does not eliminate all measurement error.

### Decoder fuzzing

A recorded run processed about 18.5 million inputs in 61 seconds (roughly 303,000 inputs/s), with zero crashes or panics observed in that run. This is not proof that all malformed inputs are safe.

## Environment and interpretation

The recorded environment had 12 cores, firmware-controlled CPU behavior, no CPU isolation, and approximately 30% drift in absolute values between runs. The RDTSC benchmark attempts to pin to core 2 but continues if pinning fails.

Criterion is used for decoder and book operations. RDTSC is used for tiny read/timing measurements. The pipeline timer measures the processing loop over a generated in-memory stream.

Compare results only when workload, setup, compiler, machine, and measurement method are understood. These figures do not represent end-to-end exchange latency or production capacity.
