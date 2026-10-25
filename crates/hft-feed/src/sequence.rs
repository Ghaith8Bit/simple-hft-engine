//! Sequence-number tracking for a market-data feed.
//!
//! Detects gaps (missing messages), duplicates, and out-of-order arrivals.
//! Does **not** implement recovery — the caller decides what to do when a
//! gap is detected (log, mark stale, request snapshot, drop the stream).
//!
//! Assumes a mostly-in-order stream with occasional loss and duplicates,
//! the shape of a UDP feed on a lossy network. Reordering windows,
//! snapshot recovery, and multi-epoch sequences are out of scope.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GapInfo {
    /// First sequence number that was expected but never arrived.
    pub expected: u32,
    /// Sequence number that did arrive.
    pub actual: u32,
    /// Number of messages lost (`actual - expected`).
    pub lost: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SequenceEvent {
    /// First message seen. No previous sequence to compare against.
    First,
    /// Sequence is exactly `previous + 1`. Normal case.
    Normal,
    /// A gap: one or more messages are missing.
    Gap(GapInfo),
    /// Sequence number matches the previous one.
    Duplicate(u32),
    /// Sequence number is smaller than the previous (without being a duplicate).
    OutOfOrder { previous: u32, actual: u32 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SequenceStats {
    pub gaps: u64,
    pub total_lost: u64,
    pub duplicates: u64,
    pub out_of_order: u64,
}

#[derive(Default)]
pub struct SequenceTracker {
    last_seq: Option<u32>,
    gaps: u64,
    total_lost: u64,
    duplicates: u64,
    out_of_order: u64,
}

impl SequenceTracker {
    pub fn new() -> Self {
        Self::default()
    }

    /// Observe the next sequence number. Returns what happened.
    pub fn observe(&mut self, seq: u32) -> SequenceEvent {
        let event = match self.last_seq {
            None => SequenceEvent::First,
            Some(prev) if seq == prev => {
                self.duplicates += 1;
                SequenceEvent::Duplicate(seq)
            }
            Some(prev) if seq == prev.wrapping_add(1) => SequenceEvent::Normal,
            Some(prev) if seq > prev => {
                let lost = seq - prev - 1;
                self.gaps += 1;
                self.total_lost += lost as u64;
                SequenceEvent::Gap(GapInfo {
                    expected: prev.wrapping_add(1),
                    actual: seq,
                    lost,
                })
            }
            Some(prev) => {
                self.out_of_order += 1;
                SequenceEvent::OutOfOrder {
                    previous: prev,
                    actual: seq,
                }
            }
        };
        self.last_seq = Some(seq);
        event
    }

    pub fn stats(&self) -> SequenceStats {
        SequenceStats {
            gaps: self.gaps,
            total_lost: self.total_lost,
            duplicates: self.duplicates,
            out_of_order: self.out_of_order,
        }
    }
}