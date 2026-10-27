# Order-book design

The order book stores up to 20 price levels on each side. Index 0 is the best level: bids are sorted highest to lowest, asks lowest to highest.

```text
Bids: highest price -> lowest price
Asks: lowest price  -> highest price
Index 0 is the best level on each side.
```

## Add operations
- Quantity at an existing price is aggregated into that level.
- A new price is inserted into the sorted side and may shift entries.
- If a side is full, a new level worse than the current worst level is not retained; a better level can displace the worst level.

Fixed arrays make capacity explicit and avoid dynamic allocation for level storage, but impose a depth limit and make insertion involve shifting.

## Cancel operations

Cancellation acts on aggregated quantity at a price, not a unique order ID. The model cannot express per-order cancellation or time priority. The current feed application path ignores the returned cancel result, so a failed cancel is not surfaced by the pipeline.

## Best prices and limits

The best bid is the highest retained bid; the best ask is the lowest retained ask. Crossed books are permitted. This is not a full matching engine: it does not maintain order IDs, FIFO priority, acknowledgements, or executions.
