# Limitations and possible next steps

## Current limitations

- **Synthetic feed:** Not a live exchange feed or a published exchange protocol.
- **Alignment and portability:** The decoder requires 8-byte alignment and uses a little-endian, x86_64-oriented layout assumption.
- **Sequence handling:** Detects some anomalies but does not reorder, recover missing data, request retransmission, or reconcile snapshots. The current pipeline still applies successfully decoded duplicate/out-of-order messages.
- **Simplified book:** Aggregates quantity by price, has fixed depth, and does not track individual order IDs or time priority. Crossed books are allowed.
- **Transport:** UDP demo is separate from the in-memory pipeline; UDP does not guarantee delivery or ordering.
- **Benchmarks:** Selected microbenchmarks and synthetic throughput do not represent end-to-end exchange latency or production capacity.
- **Strategy:** The spread-threshold check is illustrative and does not establish profitability or account for realistic execution and risk.

## Possible future work

These are ideas, not existing features:

1. Define a versioned, portable message format with explicit endian conversion.
2. Specify recovery behavior for gaps, duplicates, and out-of-order messages.
3. Expand property-based and fuzz testing and document invariants.
4. Add order identity and time priority if pursuing a matching-engine model.
5. Improve benchmark isolation, repeatability, and machine-readable output.
6. Add integration tests for transport guarantees.
7. Add operational controls only if the scope expands toward real execution.
