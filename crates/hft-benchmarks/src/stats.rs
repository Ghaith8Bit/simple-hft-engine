/// Summary statistics computed from a sorted slice of samples.
pub struct Stats {
    pub min: u64,
    pub p50: u64,
    pub p90: u64,
    pub p99: u64,
    pub p999: u64,
    pub max: u64,
    pub mean: u64,
}

impl Stats {
    pub fn from_sorted(sorted: &[u64]) -> Self {
        assert!(!sorted.is_empty(), "cannot compute stats on empty slice");
        let n = sorted.len();
        let sum: u128 = sorted.iter().map(|&x| x as u128).sum();
        Self {
            min: sorted[0],
            p50: sorted[n * 50 / 100],
            p90: sorted[n * 90 / 100],
            p99: sorted[n * 99 / 100],
            p999: sorted[(n * 999 / 1000).min(n - 1)],
            max: sorted[n - 1],
            mean: (sum / n as u128) as u64,
        }
    }
}
