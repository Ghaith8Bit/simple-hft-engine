# Portfolio engineering notes

## Summary

Simple HFT Engine is a Rust systems-engineering prototype focused on byte representation, decoding, sequence observations, a bounded order book, and performance measurement. It is not a production trading platform and does not claim profitability.

## Engineering topics

- **Data representation:** fixed-size message format with explicit layout and alignment.
- **Borrowed decoding:** validate bytes and create typed views without copying fields.
- **Input validation:** explicit error categories and malformed-input tests.
- **Sequence observability:** classify expected order, gaps, duplicates, and out-of-order observations.
- **Bounded data structures:** fixed-depth, price-aggregated book.
- **Benchmark methodology:** distinguish setup from timed work and document measurement error.
- **Scope discipline:** separate in-memory throughput from a UDP demonstration.

## Discussion questions

1. Why does a zero-copy view still need validation?
2. Why doesn't `Vec<u8>` guarantee typed alignment?
3. What trade-offs come with fixed arrays versus trees?
4. Why is detecting a sequence gap different from recovering missing messages?
5. Why is in-memory throughput not network throughput?
6. How can benchmark setup contaminate a short measurement?
7. What semantics are needed for per-order identity and time priority?

## Claims to avoid

Do not describe this as a production HFT engine, live exchange integration, complete matching engine, or profitable strategy. Do not present microbenchmark figures as guaranteed latency. See [BENCHMARKS.md](BENCHMARKS.md) and [docs/limitations.md](docs/limitations.md).
