# Visual guide

These diagrams describe the current prototype, not a production exchange architecture.

## Workspace

```mermaid
flowchart TB
    Core["hft-core<br/>Domain types"]
    Book["hft-orderbook<br/>Fixed-depth price levels"]
    Feed["hft-feed<br/>Wire and pipeline"]
    Bench["hft-benchmarks<br/>Timing and statistics"]
    Core --> Book
    Core --> Feed
    Book --> Feed
    Bench -. measures .-> Book
    Bench -. measures .-> Feed
```

## Message processing

```mermaid
flowchart TD
    Bytes["24-byte message"] --> Validate["Validate length, alignment, type, side"]
    Validate --> Borrow["Borrow typed view"]
    Borrow --> Seq["Observe sequence"]
    Seq --> Apply["Apply add/cancel"]
    Apply --> Book["Update price-aggregated book"]
    Book --> Quote["Read best bid / ask"]
```

Invalid input fails decoding. Sequence tracking detects anomalies but does not recover them; the current pipeline still applies successfully decoded messages after sequence observation.

## Message layout

```text
0       1       2       4                       8
| type  | side  | padding | sequence number     |
| price (8 bytes)                         |
| quantity (8 bytes)                      |
Total: 24 bytes
```

See [message-format.md](message-format.md) for the field table.

## Order-book levels

```text
Bids: highest price -> lowest price
Asks: lowest price  -> highest price
Index 0 is best on each side.
Each side retains at most 20 price levels.
```

## Benchmark boundary

```mermaid
flowchart LR
    Prepare["Prepare synthetic data"] --> Timed["Timed in-memory loop"]
    Timed --> Result["Throughput summary"]
    UDP["Separate UDP demo"] -. not included .-> Timed
```
