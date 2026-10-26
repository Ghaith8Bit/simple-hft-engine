//! Encode helpers + synthetic stream generator. Used by tests, benchmarks,
//! and the UDP demo sender. This is the "encoder" side of the wire format;
//! `decode.rs` is the decoder.

use crate::wire::{AddOrderMsg, AlignedMsg, CancelMsg, Header, MSG_TYPE_ADD, MSG_TYPE_CANCEL};
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
///
/// Returns `Vec<AlignedMsg>`. Each `AlignedMsg` is
/// `#[repr(C, align(8))]`, so the vector's allocation is 8-aligned by
/// the Rust allocator contract (`Layout::array::<T>()` uses
/// `align_of::<T>()`). This is a type-system guarantee, unlike
/// `Vec<u8>`, whose alignment is an allocator property.
pub fn generate_stream(n: usize) -> Vec<AlignedMsg> {
    let mut buf = Vec::with_capacity(n);
    for i in 0..n {
        let seq = i as u32;
        let msg = if i % 2 == 0 {
            let price = 100 - ((i / 2) as i64 % 10) * 2;
            encode_add(seq, 0, price, 10)
        } else {
            let price = 102 + ((i / 2) as i64 % 10) * 2;
            encode_add(seq, 1, price, 10)
        };
        buf.push(msg);
    }
    buf
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wire::MSG_SIZE;

    #[test]
    fn stream_has_expected_size() {
        let s = generate_stream(100);
        assert_eq!(s.len(), 100);
    }

    #[test]
    fn stream_slices_are_aligned() {
        let s = generate_stream(100);
        assert_eq!(
            s.as_ptr() as usize % 8,
            0,
            "Vec<AlignedMsg> base must be 8-aligned (Rust allocator guarantee)"
        );
        for msg in s.iter() {
            let slice = msg.as_slice();
            assert_eq!(
                slice.as_ptr() as usize % 8,
                0,
                "each AlignedMsg slice must be 8-aligned"
            );
            assert_eq!(slice.len(), MSG_SIZE);
        }
    }
}
