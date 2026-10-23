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
> numbers would attribute to the code. `best_bid_read` measures 3 ns
> via RDTSC, with a p99.9 tail of 8 ns. For mutations I fell back to
> criterion because RDTSC could not defeat the optimizer — and I
> documented why, because a number that looks real but is not is
> worse than an honest upper bound. Every claim comes with a
> methodology note, an explicit list of exclusions, and a roadmap of
> what I would do differently for production.

See [BENCHMARKS.md § Results](BENCHMARKS.md#results-timestamp_now-cost)
for the timestamp numbers and
[BENCHMARKS.md § order book results](BENCHMARKS.md#results-order-book-operations)
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

See [BENCHMARKS.md § order book results](BENCHMARKS.md#results-order-book-operations)
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
- `best_bid_read`: 2.60 → 3 ns via RDTSC (single 128-bit load vs.
  two loads; earlier runs measured 1 ns, the p99.9 tail of 8 ns here
  is the more honest figure).
- Mutations via criterion: 95–110 → 63–79 ns, because
  `iter_batched` shuttles 33% less stack memory per iteration.

**The operation itself did not get 33% faster.** The scaffolding did.
That distinction is the point — a reviewer can tell the difference
between "I optimized the code" and "I made the *thing around the
code* smaller and criterion noticed." The RDTSC number (3 ns) is the
one to quote for the read path; the criterion improvement is
scaffolding-driven and is documented as such.

Full numbers in
[BENCHMARKS.md § order book results](BENCHMARKS.md#results-order-book-operations).

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
jobs.** `best_bid_read` at 3 ns comes from RDTSC; mutations are
criterion upper bounds.

---

## When measurement infrastructure fails — and what to do about it

I tried to measure order-book mutations with RDTSC, using the same
batch-subtraction methodology that worked for `timestamp_now()`. The
compiler defeated the measurement. `cancel_full` (9 shifted levels)
reported *faster* than `cancel_partial` (0 shifts), which is
physically impossible. `add_same_price` (fast path, no shift)
reported *slower* than `add_new_price_worst` (no shift). The
optimizer was finding a difference between the baseline-loop and
full-loop setup costs that dominated the operation — cache-effect
asymmetry, most likely.

Two `black_box` calls (one on a seed from `rdtsc()`, one between
setup and the timing window) were not enough to prevent it. Making
the defense stronger would have meant restructuring the benchmark
into something that no longer resembled the operation being
measured.

**The decision: fall back to criterion for mutations, keep RDTSC for
reads.** Criterion's `iter_batched` accepts 55–60 ns of overhead in
exchange for the guarantee that the operation actually runs.
`best_bid_read` at 3 ns comes from RDTSC directly — reads are not
subject to the mutation-elision problem.

**Why this matters more than the numbers:** an engineer who reports a
nonsense result because the harness said so is worse than one who
recognizes the harness is broken and uses the right tool for the job.
`BENCHMARKS.md` explicitly documents which numbers are honest and
which are upper bounds. A reviewer who reads that distinction should
trust the honest numbers *more*, not less — because it means the
author was watching.

See [BENCHMARKS.md § Why two tools for the order book](BENCHMARKS.md#why-two-tools-for-the-order-book).

---

## When the number looks too good — pipeline overlap

The `best_bid_read` RDTSC benchmark has produced two very different
results across runs: `1 / 1 / 1` (p50 / p99 / p99.9 ns) once, and
`3 / 3 / 8` another time. The temptation is to publish the 1/1/1 —
it is the better number. **That would be wrong.**

`rdtsc` on x86 is not a serializing instruction. The `lfence`
before it guarantees prior loads have completed, but a *subsequent*
load can start before the counter reads. If the load is very cheap
and the pipeline is unpressured, the load and the second `rdtsc`
overlap in the out-of-order window, and the measured time can drop
below the load's actual latency. The 1/1/1 run was almost certainly
this effect: a sub-cycle result that cannot be real.

The 3/3/8 run had slightly more pipeline pressure and could not fully
overlap. It has a real tail (p99.9 = 8 ns), which is what an honest
measurement of an 8-cycle operation should look like — most samples
flat, occasional stalls on L1/L2 boundaries or branch mispredicts on
`bid_len == 0`.

**The lesson: the tightest distribution is not the truest one.**
When a microbenchmark reports a number that a physicist would reject
(a load cannot be faster than the load's own latency), suspect the
harness before you celebrate. The 3 ns figure is the one that
survives that check.

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