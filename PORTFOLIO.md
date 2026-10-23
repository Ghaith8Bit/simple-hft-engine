# Portfolio talking points

Interview narratives for the `simple-hft-engine` showcase project.
Every claim here links to a section in [`BENCHMARKS.md`](BENCHMARKS.md)
where the measurement and methodology live. Numbers are referenced,
not duplicated — BENCHMARKS.md is the single source of truth.

If you are a hiring manager or client: read the numbers in
BENCHMARKS.md first, then come here for the reasoning. The two files
are designed to be read in that order.

---

## The 30-second pitch

> I built a fixed-depth, zero-allocation limit order book in Rust and
> a custom RDTSC-based timing harness to measure it honestly. The
> harness reports the true cost of `timestamp_now()` as 28 ns at
> p50, p99, and p99.9 — the tightness proves the primitive is
> deterministic, and the subtraction cancels OS noise that the raw
> numbers would attribute to the code. The order book is a 656-byte
> struct with `best_bid` at 1.93 ns and a per-shift cost of ~0.7 ns,
> measured through a struct-size reduction. Every number in the
> write-up comes with a methodology note, an explicit list of
> exclusions, and a roadmap of what I would do differently for
> production.

See [BENCHMARKS.md § Results](BENCHMARKS.md#results-timestamp_now-cost)
for the timestamp numbers and
[BENCHMARKS.md § order book results](BENCHMARKS.md#results-order-book-operations-criterion)
for the book numbers.

---

## Why Rust for a hot-path systems project

Rust gives C++-level control over memory layout and instruction
stream, with compile-time memory safety. The trade-off is a smaller
HFT ecosystem and a smaller talent pool — no `OnixS` FIX engine, no
decades of `boost::asio` tuning. For a showcase project the safety
guarantees are a feature: I can write `unsafe` blocks in `timing.rs`
and `affinity.rs` and the compiler still enforces ownership and
borrowing everywhere else. The alternative was C++, which has
stronger HFT libraries but weaker default safety and a much slower
iteration-to-confidence loop.

---

## Why a multi-crate workspace from day one

`hft-core` (types) → `hft-benchmarks` (harness) → `hft-orderbook`
(book) — three crates with enforced, one-directional dependency
arrows. `hft-core` has `#![no_std]` and zero dependencies; if I ever
accidentally add `Vec` or `String` there, the compiler refuses to
build. If `hft-core` had lived inside `hft-orderbook`, the harness
would need to depend on the book to get the timing types. Boundaries
as compile-time contracts, not folder conventions.

---

## Why `#![no_std]` — not just "we tried to be careful"

`#![cfg_attr(not(test), no_std)]` in `hft-core` removes `Vec`,
`String`, `HashMap`, `println!`, and allocation from the crate's
vocabulary. A code reviewer would have to notice every accidental
allocation; the compiler refuses to compile it. Trade-off: no
ergonomic test output in production, `core::` instead of `std::`,
and a `cfg_attr` dance to keep the test harness working. The
compile-time guarantee is worth the ergonomic cost.

---

## Why `#[repr(transparent)]` on newtypes

`Price`, `Quantity`, `Timestamp` are all newtypes over primitives.
`#[repr(transparent)]` makes them bit-identical to the wrapped type
— same size, same alignment, same layout — so `zerocopy` decodes
market data directly into them without a copy or a conversion. The
test `assert_eq!(size_of::<Price>(), 8)` turns the promise into a
runtime check. Trade-off: no second field on `Price`, ever. That
constraint is the point.

---

## Why RDTSC instead of `Instant::now()`

`Instant::now()` on Linux is a `clock_gettime(CLOCK_MONOTONIC)` vDSO
call — ~20–30 ns, plus kernel clock-resolution granularity. For
nanosecond-budget measurements I read the CPU's timestamp counter
directly: one instruction, ~7 cycles on recent hardware,
sub-nanosecond granularity. The cost is that RDTSC gives cycle
counts, not nanoseconds, so I calibrate against a wall clock once at
startup. `Instant` is still the right tool at millisecond resolution
— which is why criterion uses it and the harness does not.

---

## Why the stopwatch itself costs 28 ns

`lfence; rdtsc` costs ~73 TSC cycles on this machine. The theoretical
minimum is 1 + ~7. The gap is Spectre/Meltdown mitigations: `lfence`
was hardened to ~30–50 cycles because the CPU has to serialize
speculative loads before letting the fence complete. This is a fixed
tax on every measurement, not a variable. The baseline subtraction
removes it — the difference row (28 ns at p50, p99, and p99.9) is
the code's honest cost, not the fence's.

**If you ever see a benchmark claiming "5 ns for a `rdtsc`" without
addressing Spectre, ask which CPU they tested on and with which
mitigations.**

See [BENCHMARKS.md § timestamp results](BENCHMARKS.md#results-timestamp_now-cost).

---

## Why pinning matters, and why it did not fix the tail

`sched_setaffinity` removes a class of tail events: **core
migrations**, where the OS moves the measurement thread to a
different core, invalidating L1, L2, and TLB. Pre-pinning max was
167 µs; post-pinning 71 µs. That is the migration story.

But pinning does not *isolate* — other processes, kernel workers,
and IRQs can still be scheduled on the pinned core. The post-pinning
max (71 µs) and the subsequent 111 µs are the remaining class:
**OS preemption on core 2**. To eliminate those requires
`isolcpus=2,3 nohz_full=2,3 rcu_nocbs=2,3` at the kernel parameter
level plus `SCHED_FIFO` on the thread. Deferred on the dev laptop
(GRUB edit + reboot risk); documented in
[BENCHMARKS.md § Roadmap](BENCHMARKS.md#roadmap) as the production
step.

---

## Why the difference row is trustworthy and the max is not

`p50`, `p99`, `p99.9` are **distributional summaries**. When the
baseline loop and the full loop share the same noise environment,
their percentiles are drawn from comparable positions in comparable
distributions — subtracting them is defensible, and the result is
stable across runs.

`max(baseline)` and `max(full)` are **individual samples** from
different iterations. Subtracting two individual samples is
arithmetically valid and statistically meaningless. That is why max
appears on the component rows (diagnostic: how noisy is the
environment?) but not on the difference row.

**The tightness of the difference row is the proof.** 28 / 28 / 29 ns
for p50 / p99 / p99.9 means the tail of the subtraction is *the same
as* the median — there is no tail, because the code has no branches
or data-dependent memory access to create one. If `timestamp_now()`
had a hidden branch, p99.9 of the difference would jump.

---

## Why fixed-depth 20, sorted array, aggregated by price

Three trade-off decisions a reviewer will ask about:

**Fixed-depth array vs `BTreeMap`.** BTreeMap gives unbounded depth at
O(log n) insert — but **allocates on every insert**. That is fatal in
the nanosecond hot path. Fixed array gives O(1) reads and O(depth)
inserts; depth is a compile-time constant (20) and the shift is
cheap. Measured per-shift cost: 0.68–0.85 ns. Depth 20 → worst-case
insert shift ~14–17 ns. Depth is the tuning knob, and the
measurement supports the number.

See [BENCHMARKS.md § order book results](BENCHMARKS.md#results-order-book-operations-criterion)
for the shift measurements.

**Sorted array vs dense-array price-indexed.** A dense array with a
`price → index` mapping would be O(1) access with zero shift, but it
requires knowing the price universe (min, max, tick) up front — an
exchange-specific tuning. The sorted array is general and fast enough
at depth 20.

**Aggregated by price vs per-order.** Per-order time priority needs a
slab allocator keyed by order ID plus an intrusive linked list per
level — meaningfully more code, and the same top-of-book information
as aggregation. Per-order tracking is a roadmap item. Aggregated is
the simpler, smaller-footprint version.

---

## The `order_count` removal — a measured optimization story

`PriceLevel` originally had `price`, `qty`, and `order_count: u32` —
24 bytes with padding. But `order_count` was **semantically broken**:
it incremented by 1 on every `add_order` call, so `add_order(qty=100)`
counted as 1, not 100. It counted API calls, not orders.

Removing the field:

- Shrank `PriceLevel` from 24 to 16 bytes (`OrderBook` 976 → 656).
- `best_bid_read`: 2.60 → 1.93 ns (−26%, single 128-bit load vs. two
  loads).
- Mutations via criterion: 95–110 → 63–79 ns (−25 to −35%), because
  `iter_batched` shuttles 33% less stack memory per iteration.

**The operation itself did not get 33% faster.** The scaffolding did.
That distinction is the point — a reviewer can tell the difference
between "I optimized the code" and "I made the *thing around the
code* smaller and criterion noticed."

Full numbers in
[BENCHMARKS.md § order book results](BENCHMARKS.md#results-order-book-operations-criterion).

---

## Why both criterion and RDTSC

Criterion is the right tool for **regression detection**: statistical
analysis, HTML reports, comparison across builds. Its `iter_batched`
overhead (~55–60 ns) is a *known, constant* tax — if a future
refactor adds a heap allocation to `add_order`, criterion will show
~300 ns and the regression is caught.

RDTSC is the right tool for **honest p50/p99 numbers**:
sub-nanosecond resolution, explicit baseline subtraction, no
scaffolding. Its `unsafe` block and platform-specific intrinsics are
the cost.

The mistake reviewers see in other portfolios is using criterion for
both and reporting ~100 ns for a ~10 ns operation. **Two tools, two
jobs.** The order-book RDTSC benchmark is the next step, and it will
report the honest figures the criterion numbers only bound.

---

## What this project is not, and why that is intentional

Not live trading. Not multi-venue. Not alpha. Not a production
system. The strategy is a stub because the project is about **systems
engineering**: latency, determinism, memory layout, the measurement
methodology itself. A complex strategy would distract from the
signal. The realistic extension path (per-order tracking, feed
handler, risk, gateway) is on the
[BENCHMARKS.md roadmap](BENCHMARKS.md#roadmap), not in the current
scope.

If a client needs exchange connectivity, that is a well-defined
extension on top of this design, not a rewrite. The boundary between
"showcase" and "production" is explicitly documented in
[BENCHMARKS.md § What these numbers do NOT include](BENCHMARKS.md#what-these-numbers-do-not-include),
so a reviewer can judge the gap.

---

## What is not yet in this file

Additional narratives that will be added as more components land:

- Per-order tracking (slab allocator + intrusive list) — why the
  data structure choice changes when order identity is preserved.
- Feed handler (UDP, `zerocopy` decode) — why decode and book
  update are measured separately, and how the latency budget is
  split.
- Risk gate placement — inline vs off-path, and the latency cost of
  each.
- End-to-end tick-to-trade — how the independent numbers compose
  (or do not) into a single latency figure.