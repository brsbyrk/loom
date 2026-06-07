//! Scoring and decision analysis — goal vectors, cliff utilities, DecisionAnalysis.
//!
//! Phase 3: builds on `SimulationResult` (raw Monte Carlo data) to produce structured
//! decision analysis — distributions, time bands, sensitivity markers, outcome probabilities.

use crate::distribution::{Distribution, TimeBand};
use serde::{Deserialize, Serialize};

// ── Goal vector ──────────────────────────────────────────────────────────────────────

/// A threshold cliff: when an attribute drops below `min`, utility is penalized.
///
/// # Example
/// ```json
/// {"min": 30.0, "penalty": 1.0}
/// ```
/// Means: if attribute < 30, its utility contribution drops to 0 (100% penalty).
/// `penalty=0.5` would halve it; `penalty=0.0` is a no-op.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Threshold {
    /// The value below which the penalty applies.
    pub min: f64,
    /// Penalty factor in [0.0, 1.0]. 1.0 = complete zeroing below threshold,
    /// 0.0 = no effect (threshold is informational only).
    pub penalty: f64,
}

/// Defines what the agent optimizes for — importance weights + nonlinear cliffs.
///
/// # Utility calculation per attribute
///
/// ```text
/// utility_i = weight_i * state[i]
/// if state[i] < cliff_i.min:
///     utility_i *= (1.0 - cliff_i.penalty)
/// ```
///
/// # JSON format
/// ```json
/// {
///   "weights": [1.0, 0.5, -0.3],
///   "cliffs": [
///     null,
///     {"min": 30.0, "penalty": 1.0},
///     null
///   ]
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GoalVector {
    /// Per-attribute weight. Positive = maximize, negative = minimize, zero = ignore.
    pub weights: Vec<f64>,
    /// Per-attribute cliff thresholds. `None` = no cliff for this attribute.
    /// Length must match `weights`.
    #[serde(default)]
    pub cliffs: Vec<Option<Threshold>>,
}

impl GoalVector {
    /// Create a simple linear goal (no cliffs).
    pub fn linear(weights: Vec<f64>) -> Self {
        let len = weights.len();
        Self {
            weights,
            cliffs: vec![None; len],
        }
    }

    /// Compute utility for a single state vector.
    pub fn utility(&self, state: &[f64]) -> f64 {
        assert_eq!(
            state.len(),
            self.weights.len(),
            "state length {} != goal weights length {}",
            state.len(),
            self.weights.len()
        );

        state
            .iter()
            .enumerate()
            .map(|(i, &value)| {
                let base = value * self.weights[i];
                if let Some(ref cliff) = self.cliffs[i] {
                    if value < cliff.min {
                        base * (1.0 - cliff.penalty)
                    } else {
                        base
                    }
                } else {
                    base
                }
            })
            .sum()
    }

    /// Validate that cliffs and weights have the same dimension.
    pub fn dimension(&self) -> usize {
        self.weights.len()
    }
}

// ── Decision analysis ────────────────────────────────────────────────────────────────

/// Structured analysis output from a Monte Carlo simulation.
///
/// Computed from raw `SimulationResult` — this is the consumer-facing output
/// that a TUI, API, or decision assistant consumes.
#[derive(Debug, Clone)]
pub struct DecisionAnalysis {
    /// Whether the decision's preconditions were satisfied. If false, all other
    /// fields contain default/empty values.
    pub decision_available: bool,

    /// Distribution of final utility scores across all Monte Carlo runs.
    pub utility_distribution: Distribution,

    /// Per-attribute outcome distributions at the end of the horizon.
    /// `attribute_outcomes[i]` is the distribution of attribute `i`.
    pub attribute_outcomes: Vec<Distribution>,

    /// Utility bands over time — min/mean/max at each simulation step.
    /// For charting: does utility peak early and decay, or climb steadily?
    pub utility_over_time: Vec<TimeBand>,

    /// Empirical outcome probabilities. Each entry is `(outcome_index, probability)`.
    /// Indices correspond to `decision.outcomes`.
    pub outcome_probabilities: Vec<(usize, f64)>,
}

impl DecisionAnalysis {
    /// Compute analysis from raw simulation results.
    pub fn from_result(result: &crate::simulation::SimulationResult) -> Self {
        if !result.decision_available {
            return Self::unavailable();
        }

        let num_runs = result.final_states.len();
        let dim = result.final_states.first().map_or(0, |s| s.len());

        // Utility distribution
        let final_utilities: Vec<f64> = result
            .utility_traces
            .iter()
            .map(|trace| *trace.last().unwrap_or(&0.0))
            .collect();
        let mut util_samples = final_utilities;
        let utility_distribution = Distribution::from_samples(&mut util_samples);

        // Per-attribute distributions
        let mut attribute_outcomes = Vec::with_capacity(dim);
        for attr_idx in 0..dim {
            let mut samples: Vec<f64> = result
                .final_states
                .iter()
                .map(|state| state[attr_idx])
                .collect();
            attribute_outcomes.push(Distribution::from_samples(&mut samples));
        }

        // Utility over time
        let utility_over_time = TimeBand::from_traces(&result.utility_traces);

        // Outcome probabilities
        let outcome_probabilities: Vec<(usize, f64)> = result
            .outcome_counts
            .iter()
            .map(|(idx, count)| (*idx, *count as f64 / num_runs as f64))
            .collect();

        DecisionAnalysis {
            decision_available: true,
            utility_distribution,
            attribute_outcomes,
            utility_over_time,
            outcome_probabilities,
        }
    }

    /// Create an empty/unavailable analysis (preconditions not met).
    pub fn unavailable() -> Self {
        DecisionAnalysis {
            decision_available: false,
            utility_distribution: Distribution::empty(),
            attribute_outcomes: Vec::new(),
            utility_over_time: Vec::new(),
            outcome_probabilities: Vec::new(),
        }
    }
}

// ── Tests ────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linear_utility() {
        let goal = GoalVector::linear(vec![1.0, 0.5, -0.3]);
        let state = vec![100.0, 80.0, 50.0];
        // 100*1.0 + 80*0.5 + 50*(-0.3) = 100 + 40 - 15 = 125
        assert!((goal.utility(&state) - 125.0).abs() < f64::EPSILON);
    }

    #[test]
    fn cliff_utility_above_threshold() {
        let goal = GoalVector {
            weights: vec![1.0, 1.0],
            cliffs: vec![
                None,
                Some(Threshold {
                    min: 30.0,
                    penalty: 1.0,
                }),
            ],
        };
        let state = vec![100.0, 50.0]; // health=50 > cliff.min=30 → no penalty
        // 100 + 50 = 150
        assert!((goal.utility(&state) - 150.0).abs() < f64::EPSILON);
    }

    #[test]
    fn cliff_utility_below_threshold() {
        let goal = GoalVector {
            weights: vec![0.0, 1.0],
            cliffs: vec![
                None,
                Some(Threshold {
                    min: 30.0,
                    penalty: 1.0,
                }),
            ],
        };
        let state = vec![100.0, 20.0]; // health=20 < cliff.min=30 → penalty=1.0 → zero
        assert!((goal.utility(&state) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn cliff_partial_penalty() {
        let goal = GoalVector {
            weights: vec![0.0, 1.0],
            cliffs: vec![
                None,
                Some(Threshold {
                    min: 50.0,
                    penalty: 0.5,
                }),
            ],
        };
        let state = vec![0.0, 40.0]; // 40 < 50 → penalty=0.5 → 40 * (1-0.5) = 20
        assert!((goal.utility(&state) - 20.0).abs() < f64::EPSILON);
    }

    #[test]
    fn goal_vector_json_round_trip() {
        let json = r#"{
            "weights": [1.0, 0.5, -0.3],
            "cliffs": [null, {"min": 30.0, "penalty": 1.0}, null]
        }"#;
        let goal: GoalVector = serde_json::from_str(json).unwrap();
        assert_eq!(goal.dimension(), 3);
        assert!(goal.cliffs[0].is_none());
        assert_eq!(goal.cliffs[1].as_ref().unwrap().min, 30.0);
        assert!(goal.cliffs[2].is_none());

        let re_json = serde_json::to_string_pretty(&goal).unwrap();
        let goal2: GoalVector = serde_json::from_str(&re_json).unwrap();
        assert_eq!(goal, goal2);
    }

    #[test]
    fn decision_analysis_from_result() {
        use crate::simulation::SimulationResult;
        use std::collections::HashMap;

        let result = SimulationResult {
            decision_available: true,
            final_states: vec![vec![100.0, 50.0], vec![200.0, 30.0]],
            utility_traces: vec![vec![100.0, 120.0], vec![150.0, 140.0]],
            outcome_counts: HashMap::from([(0, 2)]),
        };

        let analysis = DecisionAnalysis::from_result(&result);

        assert!(analysis.decision_available);
        // Final utilities: 120, 140 → mean 130
        assert!((analysis.utility_distribution.mean - 130.0).abs() < 0.01);
        // Attribute 0: 100, 200 → mean 150
        assert!((analysis.attribute_outcomes[0].mean - 150.0).abs() < 0.01);
        // Outcome 0: 2/2 = 1.0
        assert_eq!(analysis.outcome_probabilities, vec![(0, 1.0)]);
        // Two time steps
        assert_eq!(analysis.utility_over_time.len(), 2);
    }
}
