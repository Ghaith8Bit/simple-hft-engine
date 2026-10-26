//! Encode helpers + synthetic stream generator. Used by tests, benchmarks,
//! and the UDP demo sender. This is the "encoder" side of the wire format;
//! `decode.rs` is the decoder.

use crate::wire::{
    AddOrderMsg, AlignedMsg, CancelMsg, Header, MSG_SIZE, MSG_TYPE_ADD, MSG_TYPE_CANCEL,
};
use zerocopy::IntoBytes;

pub fn encode_add(seq: u32, side: u8, price: i64, qty: u64) -> AlignedMsg {
    let msg = AddOrderMsg {
        header: Header {
            msg_type: MSG_TYPE_ADD,
            side,
            _pad: [0; 2],
            seq,
        },
        price,
        qty,
    };
    let mut out = AlignedMsg::zeroed();
    out.as_mut_slice().copy_from_slice(msg.as_bytes());
    out
}

pub fn encode_cancel(seq: u32, side: u8, price: i64, qty: u64) -> AlignedMsg {
    let msg = CancelMsg {
        header: Header {
            msg_type: MSG_TYPE_CANCEL,
            side,
            _pad: [0; 2],
            seq,
        },
        price,
        qty,
    };
    let mut out = AlignedMsg::zeroed();
    out.as_mut_slice().copy_from_slice(msg.as_bytes());
    out
}

/// Build a synthetic stream of `n` messages. Alternates add on both
/// sides: even i = bid, odd i = ask. Bid prices descend from 100 by 2
/// (100, 98, 96, ...); ask prices ascend from 102 by 2 (102, 104, ...).
/// Two-sided stream so `best_bid()` and `best_ask()` are both populated.
///
/// Alignment note: slicing the returned `Vec<u8>` at multiples of
/// `MSG_SIZE` (24) is *usually* 8-aligned because standard allocators
/// return 8- or 16-aligned base pointers, and 24 is a multiple of 8.
/// This is an allocator property, not a Rust API guarantee. Decoding
/// unaligned slices returns `DecodeError::Unaligned` — no panic, no UB.
pub fn generate_stream(n: usize) -> Vec<u8> {
    let mut buf = Vec::with_capacity(n * MSG_SIZE);
    for i in 0..n {
        let seq = i as u32;
        let msg = if i % 2 == 0 {
            let price = 100 - ((i / 2) as i64 % 10) * 2;
            encode_add(seq, 0, price, 10)
        } else {
            let price = 102 + ((i / 2) as i64 % 10) * 2;
            encode_add(seq, 1, price, 10)
        };
        buf.extend_from_slice(msg.as_slice());
    }
    buf
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stream_has_expected_size() {
        let s = generate_stream(100);
        assert_eq!(s.len(), 100 * MSG_SIZE);
    }

    #[test]
    fn stream_slices_are_aligned() {
        let s = generate_stream(100);
        for i in 0..100 {
            let slice = &s[i * MSG_SIZE..(i + 1) * MSG_SIZE];
            assert_eq!(
                slice.as_ptr() as usize % 8,
                0,
                "message {} not 8-aligned",
                i
            );
        }
    }
}
