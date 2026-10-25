//! UDP listener. Receives 24-byte messages, decodes, tracks sequence,
//! applies to book. Logs gaps, duplicates, and out-of-order arrivals.
//!
//! Run: cargo run -p hft-feed --release --bin udp_demo [addr]
//! Default addr: 127.0.0.1:9000

use hft_feed::apply::apply;
use hft_feed::decode::decode;
use hft_feed::sequence::{SequenceEvent, SequenceTracker};
use hft_feed::wire::{AlignedMsg, MSG_SIZE};
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
    let mut tracker = SequenceTracker::new();

    loop {
        let (n, _) = socket.recv_from(buf.as_mut_slice())?;
        if n != MSG_SIZE {
            eprintln!("dropped {} bytes (expected {})", n, MSG_SIZE);
            continue;
        }
        let Ok(msg) = decode(buf.as_slice()) else {
            continue;
        };

        match tracker.observe(msg.seq()) {
            SequenceEvent::First => eprintln!("tracking started at seq {}", msg.seq()),
            SequenceEvent::Normal => {}
            SequenceEvent::Gap(g) => {
                eprintln!(
                    "gap: expected seq {}, got {} — {} lost",
                    g.expected, g.actual, g.lost
                );
            }
            SequenceEvent::Duplicate(s) => eprintln!("duplicate seq {}", s),
            SequenceEvent::OutOfOrder { previous, actual } => {
                eprintln!("out-of-order: previous {}, got {}", previous, actual);
            }
        }

        apply(msg, &mut book);
        count += 1;

        if count.is_multiple_of(10_000) {
            let bid = book.best_bid().map(|l| l.price.0).unwrap_or(0);
            let ask = book.best_ask().map(|l| l.price.0).unwrap_or(0);
            let s = tracker.stats();
            eprintln!(
                "msgs={} bid={} ask={} | gaps={} lost={} dup={} ooo={}",
                count, bid, ask, s.gaps, s.total_lost, s.duplicates, s.out_of_order
            );
        }
    }
}
