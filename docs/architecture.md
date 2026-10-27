# Architecture

The repository is a learning prototype with two distinct ways to feed messages into processing: the synthetic in-memory pipeline and a separate UDP demo. Do not read the UDP demo as a stage inside the pipeline.

## Workspace

```mermaid
flowchart TB
    CORE["hft-core\nCore value types"]
    BOOK["hft-orderbook\nFixed-depth price-aggregated book"]
    FEED["hft-feed\nWire format, decode, sequence, apply, replay, binaries"]
    BENCH["hft-benchmarks\nTiming, statistics, affinity helpers"]
    CORE --> BOOK
    CORE --> FEED
    BOOK --> FEED
    BENCH -. "used by benchmark code" .-> BOOK
```

The crate names and responsibilities above come from the workspace manifests and source modules.

## In-memory pipeline

```mermaid
flowchart LR
    A["Synthetic stream\nreplay.rs"] --> B["Decode\ndecode.rs"]
    B --> C["Observe sequence\nsequence.rs"]
    C --> D["Apply message\napply.rs"]
    D --> E["Order book\nhft-orderbook"]
    E --> F["Spread-threshold check\npipeline.rs"]
```

The current pipeline observes sequence events but does not stop application when an event is a gap, duplicate, or out-of-order message. See [Pipeline and sequence handling](pipeline-and-sequence.md).

## Separate UDP demo

```mermaid
flowchart LR
    S["udp_sender"] -->|"UDP datagrams"| N["Local/network socket"]
    N --> R["udp_demo"]
    R --> D["Decode and observe sequence"]
    D --> A["Apply to order book"]
```

The UDP demo has its own receive loop. The in-memory pipeline benchmark does not measure the UDP socket path.

## Where to look

| Concern | Source |
|---|---|
| Core value types | `crates/hft-core/src/lib.rs` |
| Wire structs and aligned buffer | `crates/hft-feed/src/wire.rs` |
| Encode helpers and synthetic stream | `crates/hft-feed/src/replay.rs` |
| Decoder | `crates/hft-feed/src/decode.rs` |
| Sequence tracker | `crates/hft-feed/src/sequence.rs` |
| Message application | `crates/hft-feed/src/apply.rs` |
| Order book | `crates/hft-orderbook/src/lib.rs` |
| In-memory pipeline | `crates/hft-feed/src/bin/pipeline.rs` |
| UDP sender and receiver | `crates/hft-feed/src/bin/udp_sender.rs`, `udp_demo.rs` |
