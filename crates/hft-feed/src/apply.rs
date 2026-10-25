//! Apply a decoded message to an order book.

use crate::decode::Message;
use hft_core::{Price, Quantity, Side};
use hft_orderbook::OrderBook;

pub fn apply(msg: Message<'_>, book: &mut OrderBook) {
    match msg {
        Message::Add(m) => {
            let side = if m.header.side == 0 {
                Side::Bid
            } else {
                Side::Ask
            };
            book.add_order(side, Price(m.price), Quantity(m.qty));
        }
        Message::Cancel(m) => {
            let side = if m.header.side == 0 {
                Side::Bid
            } else {
                Side::Ask
            };
            let _ = book.cancel_order(side, Price(m.price), Quantity(m.qty));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decode::decode;
    use crate::replay::{encode_add, encode_cancel};

    #[test]
    fn apply_add_updates_book() {
        let mut book = OrderBook::new();
        let buf = encode_add(1, 0, 100, 10);
        apply(decode(buf.as_slice()).unwrap(), &mut book);
        assert_eq!(book.best_bid().unwrap().price, Price(100));
        assert_eq!(book.best_bid().unwrap().qty, Quantity(10));
    }

    #[test]
    fn apply_cancel_removes_level() {
        let mut book = OrderBook::new();
        apply(
            decode(encode_add(1, 0, 100, 10).as_slice()).unwrap(),
            &mut book,
        );
        apply(
            decode(encode_cancel(2, 0, 100, 10).as_slice()).unwrap(),
            &mut book,
        );
        assert!(book.best_bid().is_none());
    }
}
