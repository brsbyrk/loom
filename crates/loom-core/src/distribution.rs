//! Empirical distribution statistics — computed from Monte Carlo samples.
//!
//! All stats are computed in-place from a `Vec<f64>` — no external stat crates needed.

/// Summary statistics for a set of samples.
///
/// Computed via [`Distribution::from_samples`]. Percentiles use linear interpolation
/// between adjacent sorted values.
#[derive(Debug, Clone)]
pub struct Distribution {
    /// Arithmetic mean.
    pub mean: f64,
    /// Population standard deviation.
    pub std: f64,
    /// 5th percentile.
    pub p5: f64,
    /// 25th percentile.
    pub p25: f64,
    /// 50th percentile (median).
    pub p50: f64,
    /// 75th percentile.
    pub p75: f64,
    /// 95th percentile.
    pub p95: f64,
    /// Minimum value.
    pub min: f64,
    /// Maximum value.
    pub max: f64,
}

impl Distribution {
    /// Compute statistics from a vector of samples.
    ///
    /// Sorts the samples in place. Returns an empty distribution with all fields
    /// set to 0.0 if the input is empty.
    pub fn from_samples(samples: &mut [f64]) -> Self {
        if samples.is_empty() {
            return Self::empty();
        }

        samples.sort_unstable_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

        let n = samples.len() as f64;
        let mean = samples.iter().sum::<f64>() / n;
        let variance = samples.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / n;
        let std = variance.sqrt();

        Self {
            mean,
            std,
            p5: percentile(samples, 0.05),
            p25: percentile(samples, 0.25),
            p50: percentile(samples, 0.50),
            p75: percentile(samples, 0.75),
            p95: percentile(samples, 0.95),
            min: samples.first().copied().unwrap_or(0.0),
            max: samples.last().copied().unwrap_or(0.0),
        }
    }

    /// Create an empty distribution (all zeros).
    pub fn empty() -> Self {
        Self {
            mean: 0.0,
            std: 0.0,
            p5: 0.0,
            p25: 0.0,
            p50: 0.0,
            p75: 0.0,
            p95: 0.0,
            min: 0.0,
            max: 0.0,
        }
    }
}

/// Utility band over time — min/mean/max at each simulation step.
#[derive(Debug, Clone)]
pub struct TimeBand {
    /// Step index (0 = post-decision, 1..N = after passives).
    pub step: usize,
    /// Minimum utility across runs at this step.
    pub min: f64,
    /// Mean utility across runs at this step.
    pub mean: f64,
    /// Maximum utility across runs at this step.
    pub max: f64,
}

impl TimeBand {
    /// Compute time bands from a per-run utility trace matrix.
    ///
    /// `traces` is `Vec<Vec<f64>>` where `traces[run][step]` is the utility score
    /// for run `run` at step `step`. Returns one `TimeBand` per step.
    /// Returns an empty vector if traces are empty.
    pub fn from_traces(traces: &[Vec<f64>]) -> Vec<TimeBand> {
        if traces.is_empty() {
            return Vec::new();
        }

        let num_steps = traces[0].len();
        let num_runs = traces.len();

        let mut bands = Vec::with_capacity(num_steps);
        for step in 0..num_steps {
            let mut values: Vec<f64> = traces.iter().map(|t| t[step]).collect();
            values.sort_unstable_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

            let mean = values.iter().sum::<f64>() / num_runs as f64;
            bands.push(TimeBand {
                step,
                min: values.first().copied().unwrap_or(0.0),
                mean,
                max: values.last().copied().unwrap_or(0.0),
            });
        }
        bands
    }
}

/// Compute a percentile from sorted data using linear interpolation.
///
/// When the rank falls between two values, linearly interpolates between them.
/// `fraction` is in [0.0, 1.0] (e.g., 0.5 for median, 0.95 for P95).
fn percentile(sorted: &[f64], fraction: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    if sorted.len() == 1 {
        return sorted[0];
    }

    let n = sorted.len() as f64;
    let rank = fraction * (n - 1.0);
    let lo = rank.floor() as usize;
    let hi = rank.ceil() as usize;

    if lo == hi {
        return sorted[lo];
    }

    let frac = rank - lo as f64;
    sorted[lo] + frac * (sorted[hi] - sorted[lo])
}

// ── Tests ────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_distribution() {
        let d = Distribution::from_samples(&mut []);
        assert_eq!(d.mean, 0.0);
        assert_eq!(d.min, 0.0);
        assert_eq!(d.max, 0.0);
    }

    #[test]
    fn single_value() {
        let d = Distribution::from_samples(&mut [42.0]);
        assert_eq!(d.mean, 42.0);
        assert_eq!(d.std, 0.0);
        assert_eq!(d.p50, 42.0);
    }

    #[test]
    fn uniform_distribution() {
        let mut samples: Vec<f64> = (0..100).map(|i| i as f64).collect();
        let d = Distribution::from_samples(&mut samples);
        assert!((d.mean - 49.5).abs() < 0.01);
        assert!((d.p50 - 49.5).abs() < 0.5); // median of 0..99 is 49.5
        assert!((d.min - 0.0).abs() < 0.01);
        assert!((d.max - 99.0).abs() < 0.01);
    }

    #[test]
    fn percentile_edge_cases() {
        let sorted = vec![10.0, 20.0, 30.0];
        assert_eq!(percentile(&sorted, 0.0), 10.0);
        assert_eq!(percentile(&sorted, 1.0), 30.0);
        assert_eq!(percentile(&sorted, 0.5), 20.0);
    }

    #[test]
    fn percentile_interpolation() {
        // 0, 25, 50, 75, 100
        let sorted = vec![0.0, 25.0, 50.0, 75.0, 100.0];
        // P25: rank = 0.25 * 4 = 1 → sorted[1] = 25
        assert_eq!(percentile(&sorted, 0.25), 25.0);
        // P50: rank = 0.50 * 4 = 2 → sorted[2] = 50
        assert_eq!(percentile(&sorted, 0.50), 50.0);
        // P75: rank = 0.75 * 4 = 3 → sorted[3] = 75
        assert_eq!(percentile(&sorted, 0.75), 75.0);
    }

    #[test]
    fn time_bands_from_traces() {
        let traces = vec![
            vec![0.0, 10.0, 20.0],
            vec![5.0, 15.0, 25.0],
        ];
        let bands = TimeBand::from_traces(&traces);
        assert_eq!(bands.len(), 3);
        assert_eq!(bands[0].step, 0);
        assert!((bands[0].mean - 2.5).abs() < 0.01);
        assert!(bands[0].min == 0.0);
        assert!(bands[0].max == 5.0);
    }
}
