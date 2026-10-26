# Visual guide: simple-hft-engine

Read top to bottom. The diagrams carry the explanation; each caption adds only the context needed to avoid misleading conclusions.

## 01. What is a market-data engine?

A message is a small record of an update. This project practices reading those records and maintaining a compact view of prices.

```mermaid
flowchart LR
    A["Synthetic market update"] --> B["Small message made of bytes"]
    B --> C["Rust code reads the message"]
    C --> D["Order book changes"]
    D --> E["Current best bid and ask"]
    E --> F["Simple spread check"]
```

## 02. What does this project do?

The main local pipeline uses generated messages. It decodes them, observes sequence numbers, applies updates, then checks the spread.

```mermaid
flowchart TB
    A["Input: generated messages"] --> B["Decode"]
    B --> C["Track sequence"]
    C --> D["Apply update"]
    D --> E["Book state"]
    E --> F["Spread-threshold signal count"]
    G["Benchmarks"] -. "measure selected paths" .-> B
    G -.-> D
```

## 03. What are we balancing?

The code and benchmarks cover correctness checks, performance measurements, and visibility into sequence anomalies. The diagram is a framing aid, not a claim of production completeness.

```mermaid
flowchart LR
    A["Correctness<br/>reject malformed input"] --- B["Performance<br/>measure selected operations"]
    B --- C["Reliability visibility<br/>report sequence anomalies"]
    A ~~~ C
```

## 04. The system at a glance

The four crates and their relationships are taken from the workspace manifests and source modules.

```mermaid
flowchart TB
    CORE["hft-core<br/>Price · Quantity · Timestamp · Side"]
    BOOK["hft-orderbook<br/>20 levels per side"]
    FEED["hft-feed<br/>wire · decode · sequence · apply · replay"]
    BENCH["hft-benchmarks<br/>RDTSC · stats · affinity"]
    CORE --> BOOK
    CORE --> FEED
    BOOK --> FEED
    BENCH -. "benchmark support" .-> BOOK
```

## 05. Follow one message

The sequence follows the pipeline code: decode first; observe sequence; apply; inspect the book; perform the spread-threshold check. Invalid decode input exits the loop iteration.

```mermaid
sequenceDiagram
    participant R as Replay generator
    participant P as Pipeline loop
    participant D as Decoder
    participant S as Sequence tracker
    participant A as Apply
    participant B as Order book
    participant T as Strategy stub
    R-->>P: Stream of 24-byte aligned messages
    P->>D: Decode next message
    D->>D: Check length, type, alignment, side
    D-->>P: Error: pipeline counts rejection and skips
    D-->>P: Valid message borrowing input bytes
    P->>S: Observe sequence number
    S-->>P: Sequence event
    P->>A: Apply decoded message
    A->>B: Add or cancel at price
    B-->>P: Updated book state
    P->>T: Read best bid / ask
    T->>T: Check spread threshold
```

## 06. What happens to invalid bytes?

The decoder returns explicit errors for empty, truncated, unaligned, unknown-type, and invalid-side input. In the pipeline, decode errors are counted as rejected and skipped.

```mermaid
flowchart TD
    A["Input bytes"] --> B{"Decoder checks"}
    B -->|"Empty / too short"| X["Decode error"]
    B -->|"Unknown type"| X
    B -->|"Bad side"| X
    B -->|"Unaligned message"| X
    B -->|"Supported type and valid side"| C["Decoded message borrows input bytes"]
    C --> D["Sequence event observed"]
    D --> E["Pipeline applies decoded message"]
    E --> F["Book and spread check"]
```

## 07. Read the order book

The illustration uses the benchmark's example setup: bids at 100, 98, 96 and asks at 102, 104, 106, with quantity 10 at each shown level. These are example internal values, not quoted real-world prices.

```mermaid
flowchart LR
    subgraph BIDS["Bids: highest first"]
      B1["100<br/>qty 10"]
      B2["98<br/>qty 10"]
      B3["96<br/>qty 10"]
    end
    subgraph SPREAD["Best prices"]
      S["Best bid 100<br/>Best ask 102<br/>Spread 2"]
    end
    subgraph ASKS["Asks: lowest first"]
      A1["102<br/>qty 10"]
      A2["104<br/>qty 10"]
      A3["106<br/>qty 10"]
    end
    B1 --- S
    S --- A1
```

## 08. Understand sequence tracking

The tracker distinguishes first, normal, gap, duplicate, and out-of-order events. It detects anomalies but does not implement feed recovery. The pipeline still applies successfully decoded messages after observing the event.

```mermaid
flowchart TD
    A["Observe next sequence number"] --> B{"Compare with last"}
    B -->|"First message"| C["First"]
    B -->|"Same number"| D["Duplicate"]
    B -->|"Previous + 1"| E["Normal"]
    B -->|"Forward jump"| F["Gap: count missing numbers"]
    B -->|"Older number"| G["Out of order"]
    C --> H["Update statistics / sequence state as implemented"]
    D --> I["Increment duplicate count"]
    E --> H
    F --> H
    G --> J["Increment out-of-order count<br/>do not move last sequence backward"]
    K["Detection only: no recovery is implemented"] -.-> F
```

## 09. Why memory alignment matters

The synthetic message is 24 bytes and the typed message needs 8-byte alignment. `AlignedMsg` provides an 8-byte-aligned buffer; unaligned input returns an error.

```mermaid
flowchart TB
    A["24-byte message buffer"] --> B{"Buffer aligned to 8 bytes?"}
    B -->|"Yes"| C["zerocopy can borrow typed message reference"]
    B -->|"No"| D["Decoder returns Unaligned"]
    C --> E["Read header, sequence, price, quantity"]
    F["AlignedMsg uses repr(C, align(8))"] --> A
    G["Decoded message refers into original buffer"] -. "no separate decoded copy" .-> E
```

## 10. What happens inside processing?

The decoder returns `Message::Add` or `Message::Cancel`; `apply` maps it to an order-book add or cancel operation. The pipeline then reads best bid/ask for its spread check.

```mermaid
flowchart LR
    A["decode(bytes)"] --> B["Message::Add or Message::Cancel"]
    B --> C["SequenceTracker::observe(seq)"]
    C --> D["apply(message, book)"]
    D --> E{"Message kind"}
    E -->|"Add"| F["add_order(side, price, qty)"]
    E -->|"Cancel"| G["cancel_order(side, price, qty)"]
    F --> H["OrderBook"]
    G --> H
    H --> I["best_bid / best_ask"]
    I --> J["Spread > threshold?"]
```

## 11. How are results measured?

Different workloads use different tools. Mutation measurements are explicitly upper bounds because Criterion's batch overhead dominates the operation.

```mermaid
flowchart TD
    A["Choose workload"] --> B{"Operation"}
    B -->|"Small read / timestamp"| C["RDTSC + lfence"]
    B -->|"Decoder"| D["Criterion iter()"]
    B -->|"Book mutation"| E["Criterion iter_batched"]
    B -->|"Full synthetic pipeline"| F["Instant around 100k-message loop"]
    C --> G["Subtract baseline percentiles"]
    D --> H["Report Criterion timing"]
    E --> I["Report upper bound"]
    F --> J["Report throughput across runs"]
    G --> K["Interpret with method caveats"]
    H --> K
    I --> K
    J --> K
```

## 12. How is it checked?

CI runs formatting, Clippy, tests, benchmark compilation, an informational RDTSC benchmark, an audit, and an informational fuzz-target build. A separate fuzz run is recorded in the existing benchmark notes.

```mermaid
flowchart TB
    A["Local / CI checks"] --> B["cargo fmt --check"]
    A --> C["cargo clippy -- -D warnings"]
    A --> D["cargo test --workspace --release"]
    A --> E["Compile benchmarks"]
    A --> F["RDTSC benchmark: informational"]
    A --> G["cargo-deny audit"]
    A --> H["Build fuzz target: informational, continue-on-error"]
    I["Separate recorded fuzz run"] --> J["18.5M inputs in 61s<br/>0 crashes / 0 panics reported"]
    H -. "build is not a fuzz campaign" .-> I
```

## 13. How do I run it?

Use the commands below to run tests, the synthetic pipeline, and individual benchmarks. See `BENCHMARKS.md` before interpreting the output.

```mermaid
flowchart LR
    A["cargo test --workspace --release"] --> B["Check tests"]
    C["cargo run -p hft-feed --release --bin pipeline -- --runs 5 --quiet"] --> D["Warmup, then timed runs"]
    D --> E["Median and range"]
    F["cargo bench -p hft-feed --bench decode"] --> G["Decoder benchmark"]
    H["cargo bench -p hft-orderbook --bench book_rdtsc"] --> I["Best bid / ask read benchmark"]
```

## 14. What does it not prove?

The scope boundaries are explicit in the code and benchmark notes. In particular, the strategy is a stub and the in-memory benchmark is not network throughput.

```mermaid
flowchart TB
    A["This repository demonstrates"] --> B["Synthetic message decoding"]
    A --> C["Sequence anomaly detection"]
    A --> D["Fixed-depth aggregated order book"]
    A --> E["Selected microbenchmarks"]
    A --> F["In-memory synthetic pipeline"]
    G["Not demonstrated by these results"] --> H["Live exchange connectivity"]
    G --> I["Measured UDP receive latency"]
    G --> J["Complete trading strategy / alpha"]
    G --> K["Per-order identity or matching"]
    G --> L["Multi-instrument scaling"]
```

## Useful commands

```bash
cargo test --workspace --release
cargo run -p hft-feed --release --bin pipeline -- --runs 5 --quiet
cargo bench -p hft-feed --bench decode
cargo bench -p hft-orderbook --bench book_rdtsc
```

For exact reported results, environment, methodology, and limitations, see [`../BENCHMARKS.md`](../BENCHMARKS.md). For definitions, see [`glossary.md`](glossary.md).
