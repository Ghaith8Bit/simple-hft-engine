# Portfolio — design problems and why we solved them this way

Each section is a design question that came up while building the
project, and the reasoning behind the choice. Numbers referenced
here live in [`BENCHMARKS.md`](BENCHMARKS.md).

---

## Problem 1: how do you measure nanoseconds honestly?

**The problem:** you cannot just call `Instant::now()` around your
code. It costs 20–30 ns, which is more than the code you are trying
to measure. And RDTSC — the CPU's cycle counter — is a raw
instruction with its own overhead (~26 ns) plus the risk of being
reordered with surrounding code.

**The choice:** RDTSC + `lfence` for serialization, plus **baseline
subtraction**.

- Measure `rdtsc; rdtsc` (no work) — this is the floor.
- Measure `rdtsc; work; rdtsc` (one operation).
- Subtract the two, per-percentile.

**Why it works:** both loops share the same noise environment (same
core, same OS). The differences at p50/p99/p99.9 are the honest cost
of `work` alone. `timestamp_now()` measured at 28/28/29 ns — flat
across percentiles, which proves it is deterministic.

**What we do not subtract:** the `max`. Max(baseline) and max(full)
are single samples from different iterations — subtracting two
unrelated samples is mathematically meaningless. Max is a diagnostic
of environment noise, not a claim about the code.

**Trade-off:** requires discipline — three separate RDTSC reads per
sample, careful percentile handling. But the resulting numbers have
a trust level that no other method provides.

---

## Problem 2: RDTSC stops working for mutations — why, and what to do

**The problem:** applying the baseline-subtraction method to order
book mutations produced *physically impossible* results.
`cancel_full` (9 shifted levels) reported faster than
`cancel_partial` (0 shifts). `add_same_price` (fast path) reported
slower than `add_new_price_worst` (no shift).

**Why:** the compiler found a difference between the baseline-loop
and full-loop setup costs — cache-effect asymmetry, most likely —
that was larger than the operation itself. Two `black_box` defenses
did not fix it.

**The choice:** fall back to criterion for mutations, keep RDTSC for
reads.

- **RDTSC for reads:** `best_bid_read` is a load with no setup, no
  drop, no state — RDTSC measures it directly at 3 ns.
- **Criterion for mutations:** `iter_batched` accepts ~55–60 ns of
  overhead in exchange for guaranteeing the operation actually runs.
  Numbers are upper bounds, labeled as such.

**Why this is honest:** a number that looks precise but is wrong is
worse than an upper bound that is labeled. Every table in
`BENCHMARKS.md` marks each number as *honest*, *upper bound*, or
*inferred*.

**Trade-off:** the mutation numbers are not precise. But they are
useful for regression detection, which is criterion's actual purpose.

---

## Problem 3: fixed-depth array vs `BTreeMap` for the order book

**The problem:** an order book needs to track 20 levels per side,
sorted by price, with O(1) access to the best bid/ask.

**Two candidates:**

- **`BTreeMap<Price, Level>`** — unbounded depth, O(log n) access,
  but **allocates on every insert**. Allocation in the nanosecond
  hot path is fatal.
- **Fixed-depth sorted array** — 20 levels per side, O(1) access,
  O(depth) insert because elements shift.

**The choice:** fixed-depth sorted array.

**Why:** the insert cost is O(depth) but depth is 20. Measured shift
cost: ~0.7 ns per level. Worst case is ~14 ns of shifting — cheap
enough, and bounded. Zero allocation. Zero heap. Predictable.

**Trade-off:** the book cannot represent more than 20 levels. For a
top-of-book strategy that is fine. For a full-depth view it is not.
Documented as a design constraint, not a bug.

---

## Problem 4: aggregated by price vs per-order tracking

**The problem:** an order book can either track **total quantity per
price level** (aggregated) or **every individual order with its own
identity** (per-order, time-priority).

**The choice:** aggregated by price.

**Why:** per-order requires a slab allocator keyed by order ID plus
an intrusive linked list per price level. More code, more state,
larger footprint — for the same top-of-book information. Aggregated
is smaller, simpler, faster.

**Trade-off:** no order identity. Cannot match a specific resting
order to an incoming aggressive order. Per-order tracking is on the
Roadmap as the next step.

---

## Problem 5: `#[repr(transparent)]` on newtypes — what does it buy?

**The problem:** `Price(i64)` looks like a value type, but if the
compiler is allowed to reorder or pad it, it is not bit-identical
to `i64` — and `zerocopy` cannot reinterpret bytes into it safely.

**The choice:** `#[repr(transparent)]` on every newtype.

**Why:** guarantees the same size, alignment, and bit layout as the
wrapped primitive. `size_of::<Price>() == 8` becomes a compile-time
invariant, checked by a test that fails if someone removes the
attribute.

**Trade-off:** the newtype can only ever have one field. `Price` is
permanently a single `i64`. That constraint is the point.

---

## Problem 6: `no_std` in `hft-core` — what does it enforce?

**The problem:** the hot path must not allocate, hash, or do I/O.
A code reviewer cannot reliably catch every accidental `Vec::push`
across a large codebase.

**The choice:** `#![cfg_attr(not(test), no_std)]` on `hft-core`.

**Why:** the compiler **refuses to compile** `Vec`, `String`,
`HashMap`, `println!`, or any allocation in `hft-core`. Enforced at
build time, not by review.

**Trade-off:** tests get an exception (`cfg_attr(not(test), ...)`).
Non-test builds pay a small ergonomic cost — `core::` instead of
`std::`. Worth it for the guarantee.

---

## Problem 7: zero-copy decode — what makes it 3 ns?

**The problem:** decoding a 24-byte wire message with `serde` costs
hundreds of nanoseconds (allocates strings, parses integers from
text). A hand-written byte-reader is 20–50 ns.

**The choice:** `zerocopy::ref_from_prefix` on the receive buffer.

**Why:** the message struct derives `FromBytes`, so `zerocopy`
reinterprets `&[u8]` as `&AddOrderMsg` in place. No copy, no
allocation, no parsing. Measured: **3.17 ns per message.** At that
speed, a 1M msg/sec feed uses 0.3% of one core.

**The alignment problem:** `AddOrderMsg` has `u64` fields (alignment
8). `ref_from_prefix` requires the source pointer to be 8-aligned.
A `[u8; 24]` on the stack is aligned to 1. **Solution:**
`AlignedMsg` — a `#[repr(C, align(8))]` wrapper around the byte
buffer. The compiler enforces alignment; `ref_from_prefix` succeeds.
Standard pattern for zero-copy UDP receive buffers.

**The proof:** `decode_borrows_from_input` test asserts the decoded
reference's pointer equals the source buffer's pointer. If those two
match, no copy happened.

**Trade-off:** requires alignment discipline — the caller must use
`AlignedMsg` or slice from an aligned `Vec<u8>`. Documented.

---

## Problem 8: why the `order_count` field was removed

**The problem:** the `PriceLevel` struct had `price`, `qty`, and
`order_count: u32`. But `order_count` was incremented by 1 on every
`add_order()` call — so `add_order(qty=100)` counted as 1, not 100.
**The field counted API calls, not orders.** It was a lie.

**The choice:** remove the field entirely.

**Measured effect:** `PriceLevel` shrank 24 → 16 bytes. `OrderBook`
shrank 976 → 656 bytes. `best_bid_read` went from 2.60 → 3 ns in
criterion (single 128-bit load vs. two). Mutation criterion numbers
went 95–110 → 63–79 ns — but **the scaffolding got smaller, not the
operation.** That distinction is in `BENCHMARKS.md`.

**The field returns** when per-order tracking is added. At that
point it counts real orders, not calls. A broken field is worse than
no field.

---

## Problem 9: multi-crate workspace vs single crate

**The problem:** the project has four logical components — types,
order book, feed handler, benchmark harness. Where do they live?

**The choice:** four packages in one workspace.

**Why:** enforced dependency direction. `hft-core` has zero
dependencies and cannot accidentally pull in `hft-orderbook`. The
harness does not depend on the book in release mode (dev-dependency
only). Boundaries are compile-time contracts, not folder conventions.

**Trade-off:** slightly slower initial builds; must list members
explicitly. Worth it for the enforced layering.

---

## Problem 10: `#![no_std]` vs `std` for the feed crate

**The problem:** should `hft-feed` also be `no_std`?

**The choice:** keep it `std`.

**Why:** `hft-feed` reads UDP. UDP is a kernel syscall — there is no
`no_std` version. The feed crate is the boundary between kernel
world and hot-path world. Below it (`hft-core`, `hft-orderbook`),
`no_std` is enforced. Above it, `std` is allowed because it is
unavoidable.

**Trade-off:** the layering is explicit: `no_std` for the data
structures, `std` for the I/O boundary. Cleaner than pretending
`hft-feed` could be `no_std` when it uses a syscall on every
message.

---

## Problem 11: how to know when the project is done

**The problem:** there is always one more crate to add. A strategy.
A risk gate. FIX connectivity. When does the project stop growing?

**The choice:** define "done" as an explicit list.

- `cargo test --workspace --release` passes with no warnings
- `cargo clippy --workspace -- -D warnings` passes
- README exists
- CI runs on push
- LICENSE exists
- One command reproduces the headline number
- A version tag exists (`v0.1.0`)

**Why this list:** every item is what a reviewer actually checks in
2–3 minutes. Everything else — strategy, gateway, multi-venue — is
in the Roadmap, not in the current scope. Adding them now would make
the project *less* complete, not more.

**Trade-off:** the project does not have a strategy. That is
intentional. The project is about systems engineering, not trading.
The strategy is a stub because a complex strategy would distract
from the signal.