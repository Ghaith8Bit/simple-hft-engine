use core::arch::x86_64::{_mm_lfence, _rdtsc};
use hft_core::Timestamp;
use std::time::Instant;

/// Read the CPU timestamp counter with a full load fence in front.
///
/// The `lfence` guarantees all prior instructions have committed before
/// the TSC is sampled; `rdtsc` itself is not a serializing instruction
/// and can otherwise be reordered with preceding memory operations.
///
/// On Spectre-hardened CPUs this costs ~30–50 cycles instead of the
/// theoretical 1, which dominates the measurement floor. See
/// `BENCHMARKS.md`.
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

        let core = 2_usize.min(crate::affinity::available_cores().saturating_sub(1));
        crate::affinity::pin_to_core(core).expect("pin_to_core failed");

        let n: usize = 100_000;

        // Warm up both paths so the first sample is not dominated by
        // cold-cache effects.
        let _ = measure_empty_loop(1_000);
        let _ = measure_full_loop(1_000);

        let baseline = Stats::from_sorted(&measure_empty_loop(n));
        let full = Stats::from_sorted(&measure_full_loop(n));

        // Baseline and full are distributional summaries computed over
        // the same sample size in the same noise environment, so it is
        // defensible to subtract their percentiles. max is NOT
        // subtracted: max(baseline) and max(full) are individual
        // samples from different iterations and their difference is
        // meaningless. See BENCHMARKS.md.
        let b_p50 = cycles_to_ns(baseline.p50, freq);
        let b_p99 = cycles_to_ns(baseline.p99, freq);
        let b_p999 = cycles_to_ns(baseline.p999, freq);
        let b_max = cycles_to_ns(baseline.max, freq);

        let f_p50 = cycles_to_ns(full.p50, freq);
        let f_p99 = cycles_to_ns(full.p99, freq);
        let f_p999 = cycles_to_ns(full.p999, freq);
        let f_max = cycles_to_ns(full.max, freq);

        let true_p50 = f_p50.saturating_sub(b_p50);
        let true_p99 = f_p99.saturating_sub(b_p99);
        let true_p999 = f_p999.saturating_sub(b_p999);

        println!(
            "timestamp_now() cost (core {}, TSC {} MHz):",
            core,
            freq / 1_000_000
        );
        println!(
            "  baseline (rdtsc; rdtsc):   p50={:>4}ns  p99={:>4}ns  p99.9={:>4}ns  max={:>8}ns",
            b_p50, b_p99, b_p999, b_max
        );
        println!(
            "  full (rdtsc; now; rdtsc):  p50={:>4}ns  p99={:>4}ns  p99.9={:>4}ns  max={:>8}ns",
            f_p50, f_p99, f_p999, f_max
        );
        println!(
            "  TRUE cost (difference):    p50={:>4}ns  p99={:>4}ns  p99.9={:>4}ns",
            true_p50, true_p99, true_p999
        );

        assert!(true_p99 < 100, "true p99 too slow: {} ns", true_p99);
    }

    /// Two `rdtsc` reads back to back, subtract, push. This is the
    /// floor: the cost of the measurement loop itself with nothing
    /// being measured between the two reads.
    fn measure_empty_loop(n: usize) -> Vec<u64> {
        let mut samples = Vec::with_capacity(n);
        for _ in 0..n {
            let start = rdtsc();
            let end = rdtsc();
            samples.push(end.saturating_sub(start));
        }
        samples.sort_unstable();
        samples
    }

    /// Two `rdtsc` reads with `timestamp_now()` in between. Difference
    /// against `measure_empty_loop` isolates the cost of
    /// `timestamp_now()` itself.
    ///
    /// `black_box` prevents the compiler from deleting the inner call
    /// as dead code (its return value is immediately discarded). The
    /// empty loop needs no such protection because both `rdtsc` results
    /// are used directly.
    fn measure_full_loop(n: usize) -> Vec<u64> {
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
