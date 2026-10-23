#![cfg_attr(not(test), no_std)]

//! Core value types for the HFT engine.
//!
//! Every type here is a newtype wrapper around a primitive, marked
//! `#[repr(transparent)]` so it has the same memory layout as the
//! primitive it wraps — no overhead, no allocation, no padding.

/// Fixed-point price, scaled by 10^8. `Price(100_000_000)` = 1.0
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Price(pub i64);

/// Order quantity in whole units.
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Quantity(pub u64);

/// CPU timestamp counter (TSC) value, in cycles.
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Timestamp(pub u64);

/// Side of the book an order belongs to.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Side {
    Bid = 0,
    Ask = 1,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn price_is_exactly_i64_sized() {
        assert_eq!(core::mem::size_of::<Price>(), 8);
    }

    #[test]
    fn price_orders_numerically() {
        assert!(Price(100) < Price(200));
    }

    #[test]
    fn side_is_one_byte() {
        assert_eq!(core::mem::size_of::<Side>(), 1);
    }
}
