# Tiny glossary

| Term | Plain-English meaning |
|---|---|
| Market-data message | A small record describing an update in the synthetic feed format used here. |
| Wire format | The agreed arrangement of bytes inside a message. |
| Decode | Check a byte slice and interpret it as a supported message type. |
| Zero-copy | The decoded message borrows the original input bytes instead of allocating a separate copy. |
| Sequence number | A number used to compare the order in which messages arrive. |
| Gap | A sequence jump that indicates one or more expected numbers were missed. |
| Duplicate | A message with the same sequence number as the previous message. |
| Out of order | A message whose sequence number is older than the tracked one. |
| Order book | A structure holding buy-side bids and sell-side asks by price. |
| Bid | A buy-side price level. |
| Ask | A sell-side price level. |
| Price level | One price and the total quantity tracked at that price. |
| Spread | Best ask price minus best bid price. |
| Fixed depth | The book keeps a bounded number of levels per side; here `DEPTH = 20`. |
| RDTSC | A CPU timestamp-counter instruction used by the benchmark harness. |
| p50 / p99 / p99.9 | Percentiles: the value at the 50th, 99th, or 99.9th percentile of sorted samples. |
| Upper bound | A measurement that includes extra overhead and should not be presented as the exact operation cost. |
| Fuzzing | Running a function against many generated inputs to find crashes or unexpected behavior. |
| Strategy stub | A small demonstration check, not a complete trading strategy. |
