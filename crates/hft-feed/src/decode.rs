//! Zero-copy decode. `decode(&buf)` returns a `Message` that borrows
//! directly from `buf` — no copy, no allocation, no parsing.

use crate::wire::{AddOrderMsg, CancelMsg, MSG_SIZE, MSG_TYPE_ADD, MSG_TYPE_CANCEL};
use zerocopy::FromBytes;

#[derive(Debug, PartialEq, Eq)]
pub enum DecodeError {
    Empty,
    Truncated,
    Unaligned,
    UnknownType(u8),
    BadSide(u8),
}

#[derive(Debug)]
pub enum Message<'a> {
    Add(&'a AddOrderMsg),
    Cancel(&'a CancelMsg),
}

pub fn decode(bytes: &[u8]) -> Result<Message<'_>, DecodeError> {
    if bytes.is_empty() {
        return Err(DecodeError::Empty);
    }
    if bytes.len() < MSG_SIZE {
        return Err(DecodeError::Truncated);
    }
    match bytes[0] {
        MSG_TYPE_ADD => {
            let (msg, _) =
                AddOrderMsg::ref_from_prefix(bytes).map_err(|_| DecodeError::Unaligned)?;
            if msg.header.side > 1 {
                return Err(DecodeError::BadSide(msg.header.side));
            }
            Ok(Message::Add(msg))
        }
        MSG_TYPE_CANCEL => {
            let (msg, _) = CancelMsg::ref_from_prefix(bytes).map_err(|_| DecodeError::Unaligned)?;
            if msg.header.side > 1 {
                return Err(DecodeError::BadSide(msg.header.side));
            }
            Ok(Message::Cancel(msg))
        }
        t => Err(DecodeError::UnknownType(t)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::replay::{encode_add, encode_cancel};

    #[test]
    fn decode_add_roundtrip() {
        let buf = encode_add(42, 0, 100, 10);
        match decode(buf.as_slice()).unwrap() {
            Message::Add(m) => {
                assert_eq!(m.header.seq, 42);
                assert_eq!(m.header.side, 0);
                assert_eq!(m.price, 100);
                assert_eq!(m.qty, 10);
            }
            _ => panic!("expected Add"),
        }
    }

    #[test]
    fn decode_cancel_roundtrip() {
        let buf = encode_cancel(7, 1, 200, 5);
        match decode(buf.as_slice()).unwrap() {
            Message::Cancel(m) => {
                assert_eq!(m.header.seq, 7);
                assert_eq!(m.header.side, 1);
                assert_eq!(m.price, 200);
                assert_eq!(m.qty, 5);
            }
            _ => panic!("expected Cancel"),
        }
    }

    #[test]
    fn decode_empty_errors() {
        assert_eq!(decode(&[]).unwrap_err(), DecodeError::Empty);
    }

    #[test]
    fn decode_truncated_errors() {
        let buf = [MSG_TYPE_ADD; 10];
        assert_eq!(decode(&buf).unwrap_err(), DecodeError::Truncated);
    }

    #[test]
    fn decode_bad_side_errors() {
        let mut buf = encode_add(1, 0, 100, 10);
        buf.0[1] = 99;
        assert_eq!(
            decode(buf.as_slice()).unwrap_err(),
            DecodeError::BadSide(99)
        );
    }

    #[test]
    fn decode_unknown_type_errors() {
        let buf = [b'Z'; 24];
        assert_eq!(decode(&buf).unwrap_err(), DecodeError::UnknownType(b'Z'));
    }

    #[test]
    fn decode_borrows_from_input() {
        // Zero-copy proof: the decoded reference must point into `buf`.
        let buf = encode_add(1, 0, 100, 10);
        let msg = decode(buf.as_slice()).unwrap();
        match msg {
            Message::Add(m) => {
                let msg_ptr = m as *const _ as *const u8;
                let buf_ptr = buf.as_slice().as_ptr();
                assert_eq!(msg_ptr, buf_ptr, "decode must be zero-copy");
            }
            _ => panic!(),
        }
    }
}
