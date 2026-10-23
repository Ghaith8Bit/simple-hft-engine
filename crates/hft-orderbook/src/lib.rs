#![cfg_attr(not(test), no_std)]

//! Fixed-depth, price-aggregated limit order book.
//!
//! # Design constraints
//!
//! - `DEPTH` levels per side, compile-time constant. Both the depth and
//!   the lack of heap allocation are intentional: a `BTreeMap` would
//!   permit unbounded depth at O(log n) insert cost, but every insert
//!   allocates, which is unacceptable in the nanosecond hot path. A
//!   dense array of `DEPTH` slots gives O(1) reads and O(DEPTH)
//!   inserts — the constant is small (20) and the shift is cheap.
//!
//! - Bids sorted descending (index 0 = best bid, highest price).
//!   Asks sorted ascending  (index 0 = best ask, lowest price).
//!
//! - Aggregated by price: each level tracks total qty only. Per-order
//!   time priority (intrusive linked list + slab allocator keyed by
//!   order ID) is a roadmap item, not in this version. The trade-off
//!   is that we lose individual order identity to keep the level
//!   compact.
//!
//! # What this crate does NOT enforce
//!
//! - **Crossed books.** Adding a bid at a price >= best ask (or vice
//!   versa) is permitted. Matching logic lives in the strategy layer,
//!   not in the data structure.
//!
//! - **Price/quantity ranges.** `Price(i64)` and `Quantity(u64)` accept
//!   any value; `Quantity(0)` on add is a no-op, but negative prices
//!   are technically constructable. Validation belongs at the boundary
//!   where market data enters the system, not at every arithmetic
//!   operation.
//!
//! - **Order identity.** No per-order tracking. See above.

use hft_core::{Price, Quantity, Side};

pub const DEPTH: usize = 20;

/// One price level: aggregated quantity at a single price.
///
/// Per-order tracking is a roadmap item. Without it, an "order count"
/// field would count *calls*, not orders — a single `add_order(qty=100)`
/// is one call but could represent any number of underlying orders.
/// The field is omitted until per-order identity is added.
#[derive(Clone, Copy, Default)]
pub struct PriceLevel {
    pub price: Price,
    pub qty: Quantity,
}

pub struct OrderBook {
    bids: [PriceLevel; DEPTH],
    asks: [PriceLevel; DEPTH],
    bid_len: usize,
    ask_len: usize,
}

impl Default for OrderBook {
    fn default() -> Self {
        Self::new()
    }
}

impl OrderBook {
    pub fn new() -> Self {
        Self {
            bids: [PriceLevel::default(); DEPTH],
            asks: [PriceLevel::default(); DEPTH],
            bid_len: 0,
            ask_len: 0,
        }
    }

    /// Add `qty` at `price` on `side`. If the price already exists on
    /// that side, its quantity is aggregated. If the book is full and
    /// the new price is worse than the worst existing level, the order
    /// is silently dropped (this matches how a fixed-depth view of an
    /// exchange book behaves).
    ///
    /// `qty == 0` is a no-op.
    pub fn add_order(&mut self, side: Side, price: Price, qty: Quantity) {
        if qty.0 == 0 {
            return;
        }
        match side {
            Side::Bid => self.add_bid(price, qty),
            Side::Ask => self.add_ask(price, qty),
        }
    }

    /// Cancel `qty` at `price` on `side`.
    ///
    /// Returns `true` if an existing level's quantity was reduced.
    /// Returns `false` if the level does not exist, the requested
    /// quantity exceeds the level's tracked quantity, or `qty == 0`.
    #[must_use = "ignoring the return value hides a failed cancel"]
    pub fn cancel_order(&mut self, side: Side, price: Price, qty: Quantity) -> bool {
        if qty.0 == 0 {
            return false;
        }
        match side {
            Side::Bid => self.cancel_bid(price, qty),
            Side::Ask => self.cancel_ask(price, qty),
        }
    }

    #[inline(always)]
    pub fn best_bid(&self) -> Option<PriceLevel> {
        if self.bid_len == 0 {
            None
        } else {
            Some(self.bids[0])
        }
    }

    #[inline(always)]
    pub fn best_ask(&self) -> Option<PriceLevel> {
        if self.ask_len == 0 {
            None
        } else {
            Some(self.asks[0])
        }
    }

    #[inline(always)]
    pub fn bid_depth(&self) -> usize {
        self.bid_len
    }

    #[inline(always)]
    pub fn ask_depth(&self) -> usize {
        self.ask_len
    }

    #[inline(always)]
    pub fn bid_level(&self, i: usize) -> Option<&PriceLevel> {
        if i < self.bid_len {
            Some(&self.bids[i])
        } else {
            None
        }
    }

    #[inline(always)]
    pub fn ask_level(&self, i: usize) -> Option<&PriceLevel> {
        if i < self.ask_len {
            Some(&self.asks[i])
        } else {
            None
        }
    }

    fn add_bid(&mut self, price: Price, qty: Quantity) {
        debug_assert!(self.bid_len <= DEPTH);
        // Bids sort descending: higher price = better = lower index.
        // `.reverse()` flips the comparator so binary_search sees the
        // array as ascending from its perspective.
        let search =
            self.bids[..self.bid_len].binary_search_by(|l| l.price.0.cmp(&price.0).reverse());
        match search {
            Ok(idx) => {
                // Fast path: price already on the book.
                self.bids[idx].qty.0 = self.bids[idx].qty.0.saturating_add(qty.0);
            }
            Err(idx) => {
                if self.bid_len < DEPTH {
                    // Shift [idx, bid_len) right by one, write at idx.
                    self.bids[..].copy_within(idx..self.bid_len, idx + 1);
                    self.bids[idx] = PriceLevel { price, qty };
                    self.bid_len += 1;
                } else if idx < DEPTH {
                    // Book full: insert at idx, drop the worst level
                    // (position DEPTH-1).
                    self.bids[..].copy_within(idx..DEPTH - 1, idx + 1);
                    self.bids[idx] = PriceLevel { price, qty };
                }
                // else idx == DEPTH: new price worse than the worst
                // level on a full book. Drop silently.
            }
        }
    }

    fn add_ask(&mut self, price: Price, qty: Quantity) {
        debug_assert!(self.ask_len <= DEPTH);
        let search = self.asks[..self.ask_len].binary_search_by(|l| l.price.0.cmp(&price.0));
        match search {
            Ok(idx) => {
                self.asks[idx].qty.0 = self.asks[idx].qty.0.saturating_add(qty.0);
            }
            Err(idx) => {
                if self.ask_len < DEPTH {
                    self.asks[..].copy_within(idx..self.ask_len, idx + 1);
                    self.asks[idx] = PriceLevel { price, qty };
                    self.ask_len += 1;
                } else if idx < DEPTH {
                    self.asks[..].copy_within(idx..DEPTH - 1, idx + 1);
                    self.asks[idx] = PriceLevel { price, qty };
                }
            }
        }
    }

    fn cancel_bid(&mut self, price: Price, qty: Quantity) -> bool {
        debug_assert!(self.bid_len <= DEPTH);
        let search =
            self.bids[..self.bid_len].binary_search_by(|l| l.price.0.cmp(&price.0).reverse());
        if let Ok(idx) = search {
            if self.bids[idx].qty.0 >= qty.0 {
                self.bids[idx].qty.0 -= qty.0;
                if self.bids[idx].qty.0 == 0 {
                    // Remove level: shift [idx+1, bid_len) left by one.
                    self.bids[..].copy_within(idx + 1..self.bid_len, idx);
                    self.bid_len -= 1;
                }
                return true;
            }
        }
        false
    }

    fn cancel_ask(&mut self, price: Price, qty: Quantity) -> bool {
        debug_assert!(self.ask_len <= DEPTH);
        let search = self.asks[..self.ask_len].binary_search_by(|l| l.price.0.cmp(&price.0));
        if let Ok(idx) = search {
            if self.asks[idx].qty.0 >= qty.0 {
                self.asks[idx].qty.0 -= qty.0;
                if self.asks[idx].qty.0 == 0 {
                    self.asks[..].copy_within(idx + 1..self.ask_len, idx);
                    self.ask_len -= 1;
                }
                return true;
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(n: i64) -> Price {
        Price(n)
    }
    fn q(n: u64) -> Quantity {
        Quantity(n)
    }

    #[test]
    fn empty_book_has_no_best() {
        let book = OrderBook::new();
        assert!(book.best_bid().is_none());
        assert!(book.best_ask().is_none());
        assert_eq!(book.bid_depth(), 0);
        assert_eq!(book.ask_depth(), 0);
    }

    #[test]
    fn add_single_bid() {
        let mut book = OrderBook::new();
        book.add_order(Side::Bid, p(100), q(10));
        let best = book.best_bid().unwrap();
        assert_eq!(best.price, p(100));
        assert_eq!(best.qty, q(10));
    }

    #[test]
    fn add_single_ask() {
        let mut book = OrderBook::new();
        book.add_order(Side::Ask, p(105), q(7));
        let best = book.best_ask().unwrap();
        assert_eq!(best.price, p(105));
        assert_eq!(best.qty, q(7));
    }

    #[test]
    fn bids_sorted_descending() {
        let mut book = OrderBook::new();
        book.add_order(Side::Bid, p(100), q(1));
        book.add_order(Side::Bid, p(102), q(1));
        book.add_order(Side::Bid, p(101), q(1));
        assert_eq!(book.best_bid().unwrap().price, p(102));
        assert_eq!(book.bid_level(1).unwrap().price, p(101));
        assert_eq!(book.bid_level(2).unwrap().price, p(100));
    }

    #[test]
    fn asks_sorted_ascending() {
        let mut book = OrderBook::new();
        book.add_order(Side::Ask, p(105), q(1));
        book.add_order(Side::Ask, p(103), q(1));
        book.add_order(Side::Ask, p(104), q(1));
        assert_eq!(book.best_ask().unwrap().price, p(103));
        assert_eq!(book.ask_level(1).unwrap().price, p(104));
        assert_eq!(book.ask_level(2).unwrap().price, p(105));
    }

    #[test]
    fn same_price_aggregates() {
        let mut book = OrderBook::new();
        book.add_order(Side::Bid, p(100), q(10));
        book.add_order(Side::Bid, p(100), q(5));
        let best = book.best_bid().unwrap();
        assert_eq!(best.qty, q(15));
        assert_eq!(book.bid_depth(), 1);
    }

    #[test]
    fn cancel_partial_reduces_qty() {
        let mut book = OrderBook::new();
        book.add_order(Side::Bid, p(100), q(10));
        assert!(book.cancel_order(Side::Bid, p(100), q(4)));
        let best = book.best_bid().unwrap();
        assert_eq!(best.qty, q(6));
        assert_eq!(book.bid_depth(), 1);
    }

    #[test]
    fn cancel_full_removes_level() {
        let mut book = OrderBook::new();
        book.add_order(Side::Bid, p(100), q(10));
        assert!(book.cancel_order(Side::Bid, p(100), q(10)));
        assert!(book.best_bid().is_none());
        assert_eq!(book.bid_depth(), 0);
    }

    #[test]
    fn cancel_nonexistent_returns_false() {
        let mut book = OrderBook::new();
        book.add_order(Side::Bid, p(100), q(10));
        assert!(!book.cancel_order(Side::Bid, p(999), q(1)));
        assert!(!book.cancel_order(Side::Ask, p(100), q(1)));
    }

    #[test]
    fn cancel_more_than_available_returns_false() {
        let mut book = OrderBook::new();
        book.add_order(Side::Bid, p(100), q(5));
        assert!(!book.cancel_order(Side::Bid, p(100), q(10)));
        assert_eq!(book.best_bid().unwrap().qty, q(5));
    }

    #[test]
    fn add_zero_qty_is_noop() {
        let mut book = OrderBook::new();
        book.add_order(Side::Bid, p(100), q(0));
        assert!(book.best_bid().is_none());
        assert_eq!(book.bid_depth(), 0);
    }

    #[test]
    fn cancel_zero_qty_returns_false() {
        let mut book = OrderBook::new();
        book.add_order(Side::Bid, p(100), q(10));
        assert!(!book.cancel_order(Side::Bid, p(100), q(0)));
        assert_eq!(book.best_bid().unwrap().qty, q(10));
    }

    #[test]
    fn crossed_book_is_permitted() {
        // The data structure does NOT enforce bid < ask. Matching is
        // the strategy layer's job. Adding a high bid and low ask
        // creates a crossed book; that is permitted by design.
        let mut book = OrderBook::new();
        book.add_order(Side::Bid, p(105), q(1));
        book.add_order(Side::Ask, p(100), q(1));
        assert_eq!(book.best_bid().unwrap().price, p(105));
        assert_eq!(book.best_ask().unwrap().price, p(100));
        assert!(book.best_bid().unwrap().price.0 > book.best_ask().unwrap().price.0);
    }

    #[test]
    fn book_fills_to_depth() {
        let mut book = OrderBook::new();
        for i in 0..DEPTH as i64 {
            book.add_order(Side::Bid, p(100 - i), q(1));
        }
        assert_eq!(book.bid_depth(), DEPTH);
        assert_eq!(book.best_bid().unwrap().price, p(100));
    }

    #[test]
    fn new_better_price_evicts_worst_when_full() {
        let mut book = OrderBook::new();
        for i in 0..DEPTH as i64 {
            book.add_order(Side::Bid, p(100 - i), q(1));
        }
        book.add_order(Side::Bid, p(200), q(1));
        assert_eq!(book.bid_depth(), DEPTH);
        assert_eq!(book.best_bid().unwrap().price, p(200));
        assert_eq!(
            book.bid_level(DEPTH - 1).unwrap().price,
            p(100 - (DEPTH as i64 - 2))
        );
    }

    #[test]
    fn worse_than_worst_is_dropped_when_full() {
        let mut book = OrderBook::new();
        for i in 0..DEPTH as i64 {
            book.add_order(Side::Bid, p(100 - i), q(1));
        }
        book.add_order(Side::Bid, p(50), q(1));
        assert_eq!(book.bid_depth(), DEPTH);
        assert_eq!(
            book.bid_level(DEPTH - 1).unwrap().price,
            p(100 - (DEPTH as i64 - 1))
        );
    }
}
