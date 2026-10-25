//! UDP sender. Blasts a synthetic stream.
//!
//! Run: cargo run -p hft-feed --release --bin udp_sender [addr]
//! Default addr: 127.0.0.1:9000

use hft_feed::replay::encode_add;
use std::net::UdpSocket;

fn main() -> std::io::Result<()> {
    let addr = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "127.0.0.1:9000".into());
    let socket = UdpSocket::bind("127.0.0.1:0")?;
    eprintln!("sending to {}", addr);

    for i in 0..100_000u32 {
        let price = 100 - (i as i64 % 10);
        let buf = encode_add(i, 0, price, 10);
        socket.send_to(buf.as_slice(), &addr)?;
    }
    Ok(())
}
