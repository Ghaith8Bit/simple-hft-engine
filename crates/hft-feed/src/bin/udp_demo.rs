//! UDP listener. Receives 24-byte messages, decodes, applies to book.
//! Prints best bid/ask every 1000 messages.
//!
//! Run: cargo run -p hft-feed --release --bin udp_demo [addr]
//! Default addr: 127.0.0.1:9000

use hft_feed::apply::apply;
use hft_feed::decode::decode;
use hft_feed::wire::AlignedMsg;
use hft_orderbook::OrderBook;
use std::net::UdpSocket;

fn main() -> std::io::Result<()> {
    let addr = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "127.0.0.1:9000".into());
    let socket = UdpSocket::bind(&addr)?;
    eprintln!("listening on {}", addr);

    let mut book = OrderBook::new();
    let mut buf = AlignedMsg::zeroed();
    let mut count: u64 = 0;

    loop {
        let (n, _) = socket.recv_from(buf.as_mut_slice())?;
        if n != hft_feed::wire::MSG_SIZE {
            eprintln!(
                "dropped {} bytes (expected {})",
                n,
                hft_feed::wire::MSG_SIZE
            );
            continue;
        }
        if let Ok(msg) = decode(buf.as_slice()) {
            apply(msg, &mut book);
            count += 1;
            if count.is_multiple_of(1000) {
                let bid = book.best_bid().map(|l| l.price.0).unwrap_or(0);
                let ask = book.best_ask().map(|l| l.price.0).unwrap_or(0);
                eprintln!("msgs={} bid={} ask={}", count, bid, ask);
            }
        }
    }
}
