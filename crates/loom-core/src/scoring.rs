//! Scoring and decision analysis — goal vectors, cliff utilities, DecisionAnalysis.
//!
//! Phase 3: builds on `SimulationResult` (raw Monte Carlo data) to produce structured
//! decision analysis — distributions, time bands, sensitivity markers, outcome probabilities.

use crate::distribution::{Distribution, TimeBand};
use serde::{Deserialize, Serialize};

use crate::traits::Valuation;

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

impl Valuation for GoalVector {
    fn score(&self, state: &[f64]) -> f64 {
        self.utility(state)
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

    /// Per-schedule-entry outcome probabilities.
    /// `outcome_probabilities[i]` is the probability distribution for schedule entry `i`.
    /// Each inner entry is `(outcome_index, probability)`.
    /// For single-decision simulations, this has one outer entry.
    pub outcome_probabilities: Vec<Vec<(usize, f64)>>,
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

        // Outcome probabilities — grouped by schedule entry index
        let num_entries = result
            .outcome_counts
            .keys()
            .map(|(e, _)| *e)
            .max()
            .map_or(0, |m| m + 1);
        let mut outcome_probabilities: Vec<Vec<(usize, f64)>> = vec![Vec::new(); num_entries];
        for ((entry_idx, outcome_idx), &count) in &result.outcome_counts {
            outcome_probabilities[*entry_idx]
                .push((*outcome_idx, count as f64 / num_runs as f64));
        }
        // Sort each entry's outcomes by index for deterministic output
        for probs in &mut outcome_probabilities {
            probs.sort_by_key(|(idx, _)| *idx);
        }

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

// ── Pareto frontier ───────────────────────────────────────────────────────────────────

/// Compute the Pareto frontier — indices of non-dominated alternatives.
///
/// Each alternative has a score vector where higher is better in all dimensions.
/// Alternative `i` dominates `j` if it is at least as good in every dimension
/// and strictly better in at least one.
///
/// # Arguments
/// * `scores` — list of `(label, score_vector)`, one per alternative.
///
/// # Returns
/// Indices of non-dominated alternatives, sorted.
///
/// # Note
/// Use mean utility from `DecisionAnalysis` for each GoalVector dimension.
/// Ensure all scores are oriented "higher is better" (negate minimization goals).
pub fn pareto_frontier(scores: &[(String, Vec<f64>)]) -> Vec<usize> {
    let n = scores.len();
    if n <= 1 {
        return (0..n).collect();
    }

    let mut dominated = vec![false; n];

    for i in 0..n {
        if dominated[i] {
            continue;
        }
        for j in 0..n {
            if i == j || dominated[j] {
                continue;
            }
            if dominates(&scores[i].1, &scores[j].1) {
                dominated[j] = true;
            } else if dominates(&scores[j].1, &scores[i].1) {
                dominated[i] = true;
                break;
            }
        }
    }

    dominated
        .iter()
        .enumerate()
        .filter(|&(_, d)| !d)
        .map(|(i, _)| i)
        .collect()
}

/// True if `a` dominates `b`: all dims >= and at least one >.
fn dominates(a: &[f64], b: &[f64]) -> bool {
    assert_eq!(a.len(), b.len(), "score vectors must have same dimension");
    let mut strictly_better = false;
    for i in 0..a.len() {
        if a[i] < b[i] {
            return false;
        }
        if a[i] > b[i] {
            strictly_better = true;
        }
    }
    strictly_better
}

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
            schedule_aborted: 0,
            final_states: vec![vec![100.0, 50.0], vec![200.0, 30.0]],
            utility_traces: vec![vec![100.0, 120.0], vec![150.0, 140.0]],
            outcome_counts: HashMap::from([((0, 0), 2)]),
        };

        let analysis = DecisionAnalysis::from_result(&result);

        assert!(analysis.decision_available);
        // Final utilities: 120, 140 → mean 130
        assert!((analysis.utility_distribution.mean - 130.0).abs() < 0.01);
        // Attribute 0: 100, 200 → mean 150
        assert!((analysis.attribute_outcomes[0].mean - 150.0).abs() < 0.01);
        // Outcome 0: 2/2 = 1.0
        assert_eq!(analysis.outcome_probabilities, vec![vec![(0, 1.0)]]);
        // Two time steps
        assert_eq!(analysis.utility_over_time.len(), 2);
    }

    // ── Pareto tests ───────────────────────────────────────────────────────────

    #[test]
    fn pareto_single_alternative() {
        let scores = vec![("A".into(), vec![10.0, 20.0])];
        assert_eq!(pareto_frontier(&scores), vec![0]);
    }

    #[test]
    fn pareto_one_dominates() {
        let scores = vec![
            ("A".into(), vec![100.0, 50.0]),
            ("B".into(), vec![50.0, 30.0]),
        ];
        // A dominates B in both dimensions
        assert_eq!(pareto_frontier(&scores), vec![0]);
    }

    #[test]
    fn pareto_non_dominated_pair() {
        let scores = vec![
            ("wealth".into(), vec![100.0, 20.0]),   // good wealth, bad health
            ("health".into(), vec![20.0, 100.0]),   // good health, bad wealth
        ];
        // Neither dominates the other
        let frontier = pareto_frontier(&scores);
        assert_eq!(frontier, vec![0, 1]);
    }

    #[test]
    fn pareto_three_with_tradeoff() {
        // A: high wealth, low health. B: medium both. C: low wealth, high health.
        let scores = vec![
            ("A".into(), vec![100.0, 10.0]),
            ("B".into(), vec![50.0, 50.0]),
            ("C".into(), vec![10.0, 100.0]),
        ];
        // B is dominated by nothing — it's not the best in either dimension
        // but it's not worse than A in BOTH or C in BOTH. Wait: A dominates B?
        // A: 100>50 (wealth) but 10<50 (health). No.
        // C: 10<50 (wealth) but 100>50 (health). No.
        // So all three are non-dominated. Pareto: {0, 1, 2}
        assert_eq!(pareto_frontier(&scores), vec![0, 1, 2]);
    }

    #[test]
    fn pareto_middle_is_dominated() {
        // A dominates C in both dimensions. B and C don't dominate each other.
        // A and B: A is better in both (100>70, 90>60) → A dominates B.
        // A dominates both. Frontier: only A.
        let scores = vec![
            ("A".into(), vec![100.0, 90.0]),
            ("B".into(), vec![70.0, 60.0]),
            ("C".into(), vec![50.0, 40.0]),  // dominated by A (and B? B 70>50, 60>40 — yes B dominates C too)
        ];
        // A dominates B, A dominates C. B dominates C. Frontier: [0]
        assert_eq!(pareto_frontier(&scores), vec![0]);
    }

    #[test]
    fn pareto_nontrivial_frontier() {
        // Two dimensions: wealth, health. All higher is better.
        // A: high wealth, low health. B: low wealth, high health. C: dominated middle.
        let scores = vec![
            ("A".into(), vec![95.0, 20.0]),
            ("B".into(), vec![30.0, 90.0]),
            ("C".into(), vec![50.0, 40.0]),  // A is better than C in both dims? 95>50, 20<40 — no
        ];
        // A (95,20) vs C (50,40): A better wealth but worse health — non-dominated
        // B (30,90) vs C (50,40): B better health but worse wealth — non-dominated
        // A vs B: clearly non-dominated
        // So all three should be frontier
        assert_eq!(pareto_frontier(&scores), vec![0, 1, 2]);
    }

    #[test]
    fn dominates_strictly_better_required() {
        // a >= b everywhere, but never > — NOT domination
        assert!(!dominates(&[5.0, 5.0], &[5.0, 5.0]));
        // a > b in one dim, equal in other — dominates
        assert!(dominates(&[5.0, 6.0], &[5.0, 5.0]));
    }
}
