# Portfolio — design problems and how they were solved

Each section is a design question that came up while building the
project, and the reasoning behind the choice. Numbers live in
[`BENCHMARKS.md`](BENCHMARKS.md).

---

## 1. Measuring nanoseconds

**Problem:** `Instant::now()` costs 20–30 ns — more than the code being
measured. RDTSC is faster but has overhead and can be reordered.

**Choice:** RDTSC + `lfence` + baseline subtraction. Measure
`rdtsc; rdtsc` (floor), `rdtsc; work; rdtsc` (full), subtract
per-percentile.

**Why:** both loops share the same noise environment. `timestamp_now()`
measured 28 / 28 / 29 ns — flat across percentiles, proving
determinism.

**Trade-off:** three RDTSC reads per sample, careful percentile
handling. `max` is not subtracted — two unrelated single samples.

---

## 2. RDTSC fails for mutations

**Problem:** baseline subtraction on order book mutations produced
physically impossible results. `cancel_full` (9 shifts) reported
faster than `cancel_partial` (0 shifts). `add_same_price` (fast path)
slower than `add_new_price_worst` (no shift).

**Why:** the compiler found a difference between baseline-loop and
full-loop setup costs — cache-effect asymmetry — larger than the
operation. Two `black_box` defenses did not fix it.

**Choice:** criterion for mutations, RDTSC for reads. Criterion
`iter_batched` accepts 55–60 ns overhead in exchange for guaranteeing
the operation runs.

**Trade-off:** mutation numbers are upper bounds, not precise. A
labeled upper bound is worth more than a precise wrong number.

---

## 3. Fixed-depth array vs `BTreeMap`

**Problem:** need 20 levels per side, sorted by price, O(1) best bid/ask.

**Candidates:** `BTreeMap` — O(log n), **allocates every insert**.
Fixed array — O(1) reads, O(depth) insert.

**Choice:** fixed-depth sorted array.

**Why:** shift cost measured at **~0.5 ns/level**. Depth 20 → ~10–14 ns
worst case. Bounded, zero allocation, predictable.

**Trade-off:** max 20 levels. Fine for top-of-book, not full depth.

---

## 4. Aggregated vs per-order

**Problem:** track total qty per level, or every order with identity?

**Choice:** aggregated by price.

**Why:** per-order needs a slab allocator keyed by ID plus an
intrusive list per level. More code, more state, larger footprint —
same top-of-book info. Aggregated is smaller, simpler, faster.

**Trade-off:** no order identity. Cannot match a specific resting
order. Roadmap item.

---

## 5. `#[repr(transparent)]` on newtypes

**Problem:** `Price(i64)` looks like a value type, but the compiler
can pad or reorder it — `zerocopy` cannot reinterpret bytes into it
safely.

**Choice:** `#[repr(transparent)]` on every newtype.

**Why:** same size, alignment, bit layout as the primitive.
`size_of::<Price>() == 8` enforced by a test.

**Trade-off:** one field only, forever. That is the point.

---

## 6. `no_std` in `hft-core`

**Problem:** hot path must not allocate. Review cannot catch every
`Vec::push`.

**Choice:** `#![cfg_attr(not(test), no_std)]`.

**Why:** the compiler refuses to compile `Vec`, `String`, `HashMap`,
`println!`, allocation in `hft-core`. Enforced at build time.

**Trade-off:** tests need `cfg_attr(not(test), ...)`. `core::`
instead of `std::`. Worth it for the guarantee.

---

## 7. Zero-copy decode

**Problem:** `serde` decoding costs hundreds of ns (allocates,
parses). Hand-written byte-reader: 20–50 ns.

**Choice:** `zerocopy::ref_from_prefix` — reinterprets `&[u8]` as
`&AddOrderMsg` in place.

**Why:** 3.17 ns per message. No copy, no allocation, no parsing.

**Alignment problem:** `u64` fields need 8-byte alignment; a `[u8; 24]`
on the stack is 1. Solution: `AlignedMsg` — `#[repr(C, align(8))]`
wrapper. Compiler enforces; runtime succeeds.

**Trade-off:** callers must use `AlignedMsg`. Documented.

---

## 8. Removing `order_count`

**Problem:** the field was incremented by 1 per `add_order()` call.
`add_order(qty=100)` counted as 1, not 100. **Counted API calls, not
orders.**

**Choice:** remove the field.

**Effect:** `PriceLevel` 24 → 16 bytes (`OrderBook` 976 → 656).
Criterion mutations 95–110 → 63–79 ns — scaffolding shuttled less
memory, operation unchanged.

A field that lies is worse than no field.

---

## 9. Multi-crate workspace

**Problem:** four logical components — where do they live?

**Choice:** four packages in one workspace.

**Why:** enforced dependency direction. `hft-core` has zero deps and
cannot accidentally pull in `hft-orderbook`. Boundaries are
compile-time contracts.

**Trade-off:** slower initial builds; explicit members. Worth it.

---

## 10. `no_std` for `hft-feed`?

**Problem:** should the feed crate also be `no_std`?

**Choice:** no, keep `std`.

**Why:** `hft-feed` reads UDP — a kernel syscall. No `no_std`
version. It is the boundary between kernel and hot-path. Below it,
`no_std` is enforced.

**Trade-off:** explicit layering. Cleaner than pretending the syscall
can be `no_std`.

---

## 11. When a benchmark looks too good

**Problem:** the RDTSC `best_bid` bench produced `1 / 1 / 1`
(p50/p99/p99.9) once, `3 / 3 / 8` another time. Tempting to quote the
lower.

**Why the low one is suspect:** `rdtsc` is not serializing. A cheap
load can overlap with the second `rdtsc`'s fence and appear sub-cycle.
**A sub-cycle load is not real.**

**Choice:** report the range (1–3 ns p50, 5–8 ns p99.9), quote the
conservative 3 ns.

**Lesson:** when a microbenchmark reports a number a physicist would
reject, suspect the harness.

---

## 12. Decoder cost ≠ pipeline cost

**Problem:** the README once quoted *"1M msgs/sec uses 0.3% of one
core"* based on decoder alone. But decoder is not pipeline.

**What changed:** the `pipeline` binary gave a real end-to-end number.

- **Decoder alone:** 3.17 ns/msg.
- **End-to-end:** **67.7M msgs/sec median** on 2.4 MB in-memory stream
  (5 runs, 37–76M range).

At 67.7M msgs/sec, one message takes **~15 ns end-to-end**. Decoder is
~3 ns; the rest is book update, strategy check, loop overhead.

**What it excludes:** the stream is L3-resident. The kernel UDP
receive path is not measured — that is tens of µs, two orders slower.

**Why it matters:** *"3 ns per message"* and *"67M msgs/sec"* answer
different questions. Only the second is what a reviewer cares about —
with the caveat.

---

## 13. Fuzzing the decoder

**Problem:** `decode()` takes untrusted bytes. A panic, out-of-bounds,
or infinite loop is a production incident. Unit tests only cover
inputs a human thought of.

**Choice:** `cargo-fuzz` + libFuzzer on `decode()` with arbitrary
`&[u8]`.

**Why:** libFuzzer generates, mutates, and keeps inputs that reach new
code paths. Adversarial in a way unit tests cannot be.

**Result:** 18.5 million inputs in 61 s. No crashes, no panics.

**What went wrong:** `cargo fuzz init` misdetected the parent crate
in this workspace (named the fuzz crate `hft-benchmarks-fuzz` and
pointed the dependency at the wrong path). The `Cargo.toml` was
hand-written, plus an empty `[workspace]` to stop Cargo walking up.

**Signal:** fuzz-tested parsers demonstrate trust-boundary awareness.
Most Rust showcases have no fuzzing. The tooling breakdown + fix
appears in the commit — the tool was debugged, not just used.

---

## 14. When to stop

**Problem:** the project is always one more crate away from finished.

**Choice:** define "done" as a list.

- `cargo test --workspace --release` — 38 tests, 0 failures
- `cargo clippy --workspace --all-targets -- -D warnings` — clean
- `cargo deny check` — advisories / bans / licenses / sources ok
- README matches the numbers
- CI green (test, audit, fuzz-build)
- One command reproduces the headline number
- Version tag `v0.2.0` exists

**Why:** every item is what a reviewer checks in 2–3 minutes.
Everything else is on the roadmap, not the current scope.

**Trade-off:** no strategy. Deliberate. The project is about systems
engineering.