# Design decisions and trade-offs

These choices fit a compact prototype; they are not universal rules for production trading systems.

## Fixed-depth arrays
The book stores 20 levels per side. Capacity is explicit and level storage does not need dynamic allocation, but depth is bounded and insertions may shift entries.

## Price aggregation
Quantities at a price are aggregated. This simplifies the book but loses individual order identity and time priority; it is not a complete matching engine.

## Borrowed zero-copy decoding
`zerocopy` creates typed views over aligned input bytes instead of copying fields. Validation is still required for length, alignment, type, and side.

## Explicit alignment
`AlignedMsg` wraps a 24-byte message with 8-byte alignment. The representation is tailored to the current implementation and its x86_64-oriented assumptions, not a portable exchange protocol.

## Sequence tracking separate from decoding
Decoding validates representation; sequence tracking classifies stream order. The tracker detects anomalies but does not recover them. The pipeline still applies successfully decoded anomalous messages.

## Domain newtypes
`Price(i64)`, `Quantity(u64)`, and `Timestamp(u64)` make values more explicit. They use `#[repr(transparent)]`. `Timestamp` represents TSC cycles in timing-related code, not wall-clock time.

## `no_std` core
`hft-core` and `hft-orderbook` support `no_std` outside tests, reducing dependence on operating-system facilities. Feed processing and benchmark tooling use `std`.

## Separate in-memory and UDP demos
The pipeline benchmark processes a generated in-memory stream. It does not measure network throughput. UDP does not guarantee delivery or ordering.

## Measurement tools
Criterion is used for decoder and order-book operations; RDTSC is used for very short read/timing measurements. Setup/teardown can inflate mutation measurements, which are treated as upper bounds. CPU scheduling and frequency behavior affect results.

## Simple signal logic
The spread-threshold check demonstrates component wiring, not alpha, profitability, realistic execution, or risk-adjusted returns.

## Validation and scope
Decoder tests and fuzzing provide evidence for tested inputs, not proof of correctness. Live exchange integration, recovery, per-order identity, FIFO matching, production risk controls, and operational monitoring are outside the current scope.
