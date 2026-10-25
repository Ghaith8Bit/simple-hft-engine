//! Synthetic ITCH-style wire format.
//!
//! Every message is exactly 24 bytes. The `FromBytes` / `IntoBytes`
//! derives let `zerocopy` reinterpret a `&[u8]` as `&AddOrderMsg`
//! without copying or parsing.
//!
//! Wire layout (little-endian, x86_64 native):
//!
//! ```text
//! Header (8 bytes)
//!   0: msg_type  u8   b'A' (Add) or b'X' (Cancel)
//!   1: side      u8   0 = Bid, 1 = Ask
//!   2: pad       [u8; 2]
//!   4: seq       u32
//!
//! Body (16 bytes)
//!   8: price     i64
//!  16: qty       u64
//! ```
//!
//! # Alignment requirement
//!
//! `AddOrderMsg` has alignment 8 (because of its `u64` field).
//! Zero-copy decode via `ref_from_prefix` requires the source slice's
//! pointer to be 8-byte aligned. Use `AlignedMsg` (below) for buffers
//! you control. Slices into a `Vec<u8>` are 8-aligned because the
//! allocator returns at least 16-byte-aligned pointers and 24 is a
//! multiple of 8 — so `&stream[i*24..(i+1)*24]` is always aligned.

use zerocopy::{FromBytes, Immutable, IntoBytes, KnownLayout};

pub const MSG_TYPE_ADD: u8 = b'A';
pub const MSG_TYPE_CANCEL: u8 = b'X';
pub const MSG_SIZE: usize = 24;

#[repr(C)]
#[derive(Debug, Clone, Copy, FromBytes, IntoBytes, KnownLayout, Immutable)]
pub struct Header {
    pub msg_type: u8,
    pub side: u8,
    pub _pad: [u8; 2],
    pub seq: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, FromBytes, IntoBytes, KnownLayout, Immutable)]
pub struct AddOrderMsg {
    pub header: Header,
    pub price: i64,
    pub qty: u64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, FromBytes, IntoBytes, KnownLayout, Immutable)]
pub struct CancelMsg {
    pub header: Header,
    pub price: i64,
    pub qty: u64,
}

// Compile-time size assertions: any change to layout breaks the build.
const _: () = assert!(core::mem::size_of::<AddOrderMsg>() == MSG_SIZE);
const _: () = assert!(core::mem::size_of::<CancelMsg>() == MSG_SIZE);
const _: () = assert!(core::mem::align_of::<AddOrderMsg>() == 8);

/// A 24-byte buffer guaranteed to be 8-byte aligned.
///
/// `[u8; 24]` on the stack has alignment 1. `AddOrderMsg` needs 8.
/// `AlignedMsg` bridges the gap: `#[repr(C, align(8))]` gives the
/// struct 8-byte alignment, and its single `[u8; 24]` field sits at
/// offset 0. So `&self.0 as *const u8` is 8-aligned, and
/// `ref_from_prefix::<AddOrderMsg>(self.as_slice())` succeeds at
/// runtime.
///
/// This is the standard pattern for zero-copy UDP / stream buffers.
#[repr(C, align(8))]
#[derive(Copy, Clone)]
pub struct AlignedMsg(pub [u8; MSG_SIZE]);

impl AlignedMsg {
    pub const fn zeroed() -> Self {
        Self([0u8; MSG_SIZE])
    }

    pub fn as_slice(&self) -> &[u8] {
        &self.0
    }

    pub fn as_mut_slice(&mut self) -> &mut [u8] {
        &mut self.0
    }
}
