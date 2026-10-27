# Testing and validation

## Routine checks

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --release
cargo bench --workspace --no-run
```

These commands check formatting, Clippy warnings, release tests, and benchmark compilation.

## Decoder coverage

Decoder tests cover round-tripping, malformed input, invalid message type and side, alignment rejection, and confirmation that decoded views borrow the original input buffer. Consult the crate tests for the exact current cases.

Sequence tracking tests cover classifications and counters; order-book tests cover add/cancel and best-price behavior.

## Fuzzing

With nightly Rust and `cargo-fuzz` installed:

```bash
cd crates/hft-feed
cargo +nightly fuzz run decode
```

Record duration, input count, and whether crashes were observed. A clean run is evidence only for the tested run, not proof of correctness.

The CI fuzz-build step is informational and configured not to fail the overall workflow if that optional step fails. Check `.github/workflows/ci.yml` for current authoritative behavior.

## Before publishing results

- Record the exact command and toolchain.
- Identify whether setup or data generation is inside the timed region.
- State whether results are per message, per operation, or per workload.
- Describe CPU pinning and environment constraints.
- Avoid presenting a single run as a stable guarantee.
