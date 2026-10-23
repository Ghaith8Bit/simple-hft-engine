use core::arch::x86_64::{_mm_lfence, _rdtsc};
use hft_core::Timestamp;
use std::time::Instant;

/// Read the CPU timestamp counter with a full load fence in front.
///
/// The `lfence` guarantees all prior instructions have committed before
/// the TSC is sampled; `rdtsc` itself is not a serializing instruction
/// and can otherwise be reordered with preceding memory operations.
#[inline(always)]
pub fn rdtsc() -> u64 {
    unsafe {
        _mm_lfence();
        _rdtsc()
    }
}

/// Wrap `rdtsc()` in a `Timestamp`. One instruction sequence, one move.
#[inline(always)]
pub fn timestamp_now() -> Timestamp {
    Timestamp(rdtsc())
}

/// Measure the TSC frequency in Hz by reading the counter around a
/// wall-clock sleep. Called once per benchmark process; the result is
/// cached by the caller, not by this function.
pub fn calibrate_tsc_frequency() -> u64 {
    // Warm up the instruction cache and the TSC itself.
    let _ = rdtsc();

    let wall_start = Instant::now();
    let tsc_start = rdtsc();

    std::thread::sleep(std::time::Duration::from_millis(200));

    let tsc_end = rdtsc();
    let wall_elapsed_ns = wall_start.elapsed().as_nanos() as u64;

    if wall_elapsed_ns == 0 {
        return 0;
    }
    let cycles = tsc_end.saturating_sub(tsc_start);
    (cycles as u128 * 1_000_000_000 / wall_elapsed_ns as u128) as u64
}

#[inline(always)]
pub fn cycles_to_ns(cycles: u64, freq_hz: u64) -> u64 {
    if freq_hz == 0 {
        return 0;
    }
    (cycles as u128 * 1_000_000_000 / freq_hz as u128) as u64
}

#[inline(always)]
pub fn ns_to_cycles(ns: u64, freq_hz: u64) -> u64 {
    (ns as u128 * freq_hz as u128 / 1_000_000_000) as u64
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stats::Stats;
    use std::hint::black_box;

    #[test]
    fn tsc_is_monotonic() {
        let a = rdtsc();
        let b = rdtsc();
        assert!(b >= a, "TSC went backwards: {} then {}", a, b);
    }

    #[test]
    fn calibration_produces_plausible_frequency() {
        let freq = calibrate_tsc_frequency();
        println!("Calibrated TSC: {} MHz", freq / 1_000_000);
        assert!(
            freq > 500_000_000,
            "TSC frequency implausibly low: {} Hz",
            freq
        );
        assert!(
            freq < 10_000_000_000,
            "TSC frequency implausibly high: {} Hz",
            freq
        );
    }

    #[test]
    fn smoke_test_timestamp_overhead() {
        let freq = calibrate_tsc_frequency();
        assert!(freq > 0);

        let n: usize = 100_000;

        let unpinned = measure_overhead(n);
        let s_un = Stats::from_sorted(&unpinned);

        let core = 2_usize.min(crate::affinity::available_cores().saturating_sub(1));
        crate::affinity::pin_to_core(core).expect("pin_to_core failed");

        let pinned = measure_overhead(n);
        let s_pin = Stats::from_sorted(&pinned);

        println!("timestamp_now() overhead (TSC {} MHz):", freq / 1_000_000);
        println!(
            "  unpinned:     p50={:>5}ns  p99={:>5}ns  max={:>10}ns",
            cycles_to_ns(s_un.p50, freq),
            cycles_to_ns(s_un.p99, freq),
            cycles_to_ns(s_un.max, freq),
        );
        println!(
            "  pinned c{:>2}:  p50={:>5}ns  p99={:>5}ns  max={:>10}ns",
            core,
            cycles_to_ns(s_pin.p50, freq),
            cycles_to_ns(s_pin.p99, freq),
            cycles_to_ns(s_pin.max, freq),
        );

        assert!(
            cycles_to_ns(s_pin.p99, freq) < 1000,
            "p99 too slow: {} ns",
            cycles_to_ns(s_pin.p99, freq)
        );
    }

    /// Time `n` iterations of `timestamp_now()` and return the sorted
    /// per-iteration cycle counts.
    ///
    /// Each iteration reads `rdtsc` twice more than strictly necessary —
    /// once before the measured call and once after — so the per-sample
    /// value includes the measurement overhead, not just the cost of
    /// `timestamp_now()`. That is intentional: the overhead is a
    /// constant that will be subtracted against an empty-loop baseline
    /// in a later pass.
    fn measure_overhead(n: usize) -> Vec<u64> {
        let mut samples = Vec::with_capacity(n);
        for _ in 0..n {
            let start = rdtsc();
            let _ts = black_box(timestamp_now());
            let end = rdtsc();
            samples.push(end.saturating_sub(start));
        }
        samples.sort_unstable();
        samples
    }
}
