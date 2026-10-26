//! Integration test: decode a real stream, feed sequences to the tracker,
//! assert the classifications.

use hft_feed::decode::decode;
use hft_feed::replay::encode_add;
use hft_feed::sequence::{SequenceEvent, SequenceStats, SequenceTracker};

/// Decode each message in the stream and observe its sequence number.
/// Decode each message in the stream and observe its sequence number.
fn run_tracker(sequences: &[u32]) -> (Vec<SequenceEvent>, SequenceStats) {
    let mut tracker = SequenceTracker::new();
    let mut events = Vec::with_capacity(sequences.len());
    for &seq in sequences {
        let buf = encode_add(seq, 0, 100, 10);
        let msg = decode(buf.as_slice()).expect("encode_add always decodes");
        events.push(tracker.observe(msg.seq()));
    }
    (events, tracker.stats())
}

#[test]
fn sequential_stream_is_all_normal() {
    let (events, stats) = run_tracker(&[0, 1, 2, 3, 4]);
    assert!(matches!(events[0], SequenceEvent::First));
    for e in &events[1..] {
        assert!(matches!(e, SequenceEvent::Normal));
    }
    assert_eq!(stats.gaps, 0);
    assert_eq!(stats.total_lost, 0);
    assert_eq!(stats.duplicates, 0);
    assert_eq!(stats.out_of_order, 0);
}

#[test]
fn single_gap_is_detected_with_correct_count() {
    // 0, 1, 2, 5, 6 → seq 3 and 4 are lost.
    let (events, stats) = run_tracker(&[0, 1, 2, 5, 6]);

    assert!(matches!(events[3], SequenceEvent::Gap(_)));
    match events[3] {
        SequenceEvent::Gap(g) => {
            assert_eq!(g.expected, 3);
            assert_eq!(g.actual, 5);
            assert_eq!(g.lost, 2);
        }
        _ => unreachable!(),
    }

    assert_eq!(stats.gaps, 1);
    assert_eq!(stats.total_lost, 2);
}

#[test]
fn duplicate_is_detected() {
    let (events, stats) = run_tracker(&[0, 1, 1, 2]);
    assert!(matches!(events[2], SequenceEvent::Duplicate(1)));
    assert_eq!(stats.duplicates, 1);
    assert_eq!(stats.gaps, 0);
    assert_eq!(stats.total_lost, 0);
}

#[test]
fn out_of_order_is_detected() {
    let (events, stats) = run_tracker(&[0, 1, 5, 3]);
    assert!(matches!(
        events[3],
        SequenceEvent::OutOfOrder {
            previous: 5,
            actual: 3
        }
    ));
    assert_eq!(stats.out_of_order, 1);
}

#[test]
fn multiple_gaps_accumulate_stats() {
    // Gap of 1, then gap of 3.
    let (_events, stats) = run_tracker(&[0, 2, 6]);
    assert_eq!(stats.gaps, 2);
    assert_eq!(stats.total_lost, 1 + 3);
}

#[test]
fn reordered_packet_does_not_cause_false_gap() {
    // seq 5, then delayed seq 3, then seq 6.
    // Seq 3 is out-of-order; seq 6 is Normal. No gap should be reported.
    let mut t = SequenceTracker::new();
    assert!(matches!(t.observe(5), SequenceEvent::First));
    assert!(matches!(t.observe(3), SequenceEvent::OutOfOrder { .. }));
    assert!(matches!(t.observe(6), SequenceEvent::Normal));

    let s = t.stats();
    assert_eq!(s.gaps, 0);
    assert_eq!(s.total_lost, 0);
    assert_eq!(s.out_of_order, 1);
}

#[test]
fn duplicate_after_reorder_is_classified_against_forward_reference() {
    // 5, 3 (out of order), 5 (duplicate of the forward reference).
    let mut t = SequenceTracker::new();
    t.observe(5);
    assert!(matches!(t.observe(3), SequenceEvent::OutOfOrder { .. }));
    assert!(matches!(t.observe(5), SequenceEvent::Duplicate(5)));

    let s = t.stats();
    assert_eq!(s.duplicates, 1);
    assert_eq!(s.out_of_order, 1);
    assert_eq!(s.gaps, 0);
}

#[test]
fn gap_then_late_fill_is_out_of_order_not_extra_gap() {
    // 5, 8 (gap, last_seq=8), then late 6 and 7.
    // The late packets are out-of-order. Gap count stays 1.
    let mut t = SequenceTracker::new();
    t.observe(5);
    assert!(matches!(t.observe(8), SequenceEvent::Gap(_)));
    assert!(matches!(t.observe(6), SequenceEvent::OutOfOrder { .. }));
    assert!(matches!(t.observe(7), SequenceEvent::OutOfOrder { .. }));

    let s = t.stats();
    assert_eq!(s.gaps, 1);
    assert_eq!(s.total_lost, 2); // optimistic — 6 and 7 were counted lost
    assert_eq!(s.out_of_order, 2);
}

#[test]
fn u32_wraparound_is_forward() {
    let mut t = SequenceTracker::new();
    assert!(matches!(t.observe(u32::MAX - 1), SequenceEvent::First));
    assert!(matches!(t.observe(u32::MAX), SequenceEvent::Normal));
    assert!(matches!(t.observe(0), SequenceEvent::Normal));
    assert!(matches!(t.observe(1), SequenceEvent::Normal));

    let s = t.stats();
    assert_eq!(s.gaps, 0);
    assert_eq!(s.out_of_order, 0);
}
