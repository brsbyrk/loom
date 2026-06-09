//! Monte Carlo simulation engine.
//!
//! The core loop: for each run, clone initial state → apply decision cost →
//! sample outcome → apply transform → tick passives for N steps → record results.
//!
//! # Overview
//!
//! ```text
//! Simulation::run(state, decision, goal, horizon=24, runs=1000)
//!   → SimulationResult {
//!       final_states,       // state after the full horizon, per run
//!       utility_traces,     // utility score at each step, per run
//!       outcome_counts,     // how many times each outcome was sampled
//!       decision_available, // did preconditions pass?
//!     }
//! ```

use crate::event::{Decision, Outcome, Project, ActiveProject};
use crate::events::ResolvedEvent;
use crate::schema::DynamicState;
use crate::scoring::{DecisionAnalysis, GoalVector};
use crate::traits::{Action, All, Predicate, Sequence, Valuation};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

// ── Passive effects ──────────────────────────────────────────────────────────────────

/// How often a passive effect ticks.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Frequency {
    /// Every step.
    EveryStep,
    /// Every N steps (e.g., every 12 steps = once per year if a step = 1 month).
    Every(usize),
    /// Triggered when a condition holds (checked each step).
    When(crate::event::Condition),
}

/// A recurring effect that the simulation applies automatically between decisions.
///
/// Examples:
/// - "income: wealth.cash += 5000 every step" (monthly salary on a monthly step)
/// - "stress_decay: health.stress -= 2 every step"
/// - "burnout_check: if health.stress > 80, health.physical -= 5 every step"
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PassiveEffect {
    /// Unique identifier.
    pub id: String,
    /// Human-readable label.
    pub label: String,
    /// When this effect ticks.
    pub frequency: Frequency,
    /// Deterministic attribute modifications applied when this effect fires.
    pub effects: Vec<crate::event::AttributeEffect>,
}

impl PassiveEffect {
    /// Create a passive that fires every step.
    pub fn every_step(id: &str, label: &str, effects: Vec<crate::event::AttributeEffect>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            frequency: Frequency::EveryStep,
            effects,
        }
    }

    /// Create a passive that fires every N steps.
    pub fn every_n(
        id: &str,
        label: &str,
        n: usize,
        effects: Vec<crate::event::AttributeEffect>,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            frequency: Frequency::Every(n),
            effects,
        }
    }

    /// Create a passive that fires when a condition holds.
    pub fn when(
        id: &str,
        label: &str,
        condition: crate::event::Condition,
        effects: Vec<crate::event::AttributeEffect>,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            frequency: Frequency::When(condition),
            effects,
        }
    }

    /// Check whether this passive should fire at the current step.
    pub fn should_fire(&self, state: &[f64], step: usize) -> bool {
        match &self.frequency {
            Frequency::EveryStep => true,
            Frequency::Every(n) => step % n == 0,
            Frequency::When(cond) => cond.check(state),
        }
    }

    /// Apply effects to a mutable state slice.
    pub fn apply(&self, state: &mut [f64]) {
        for effect in &self.effects {
            effect.apply(state);
        }
    }
}

// ── Simulation result ────────────────────────────────────────────────────────────────

/// Raw output from a Monte Carlo simulation.
///
/// This is what the engine returns. The analysis layer consumes this
/// to produce `DecisionAnalysis` — distributions, sensitivity, Pareto fronts.
#[derive(Debug, Clone)]
pub struct SimulationResult {
    /// Whether the decision's preconditions were satisfied. If `false`, no runs
    /// were executed and all other fields are empty.
    /// For schedules, true if the first scheduled decision's preconditions pass.
    pub decision_available: bool,

    /// Number of runs that were aborted because a `required` scheduled decision's
    /// preconditions weren't met. Always 0 for single-decision simulations.
    pub schedule_aborted: usize,

    /// Final state at the end of the horizon, per Monte Carlo run.
    /// `results.final_states[i]` corresponds to run `i`. Aborted runs produce
    /// the state at the point of abort.
    pub final_states: Vec<Vec<f64>>,

    /// Utility score at each step, per run. `utility_traces[i][j]` is the score
    /// at step `j` for run `i`.
    pub utility_traces: Vec<Vec<f64>>,

    /// Outcome sampling counts per schedule entry.
    /// Keys are `(schedule_entry_index, outcome_index)`. For single-decision
    /// simulations, all keys have `schedule_entry_index = 0`.
    pub outcome_counts: HashMap<(usize, usize), usize>,
}

// ── Simulation ───────────────────────────────────────────────────────────────────────

/// Configuration for a Monte Carlo forward simulation.
pub struct Simulation {
    /// Number of forward steps after the decision resolves.
    pub horizon: usize,

    /// Number of independent Monte Carlo runs.
    pub monte_carlo_runs: usize,

    /// Passive effects that tick forward each step.
    pub passives: Vec<PassiveEffect>,

    /// Resolved event templates — checked each step via [`crate::determine_firing`].
    /// Events fire when their preconditions are met and may chain-trigger or suppress
    /// other events.
    #[allow(dead_code)]
    pub events: Vec<ResolvedEvent>,

    /// Available projects.
    pub projects: Vec<Project>,

    /// Currently active projects.
    pub active_projects: Vec<ActiveProject>,
}

impl Simulation {
    /// Create a new simulation with default empty passives and events.
    pub fn new(horizon: usize, monte_carlo_runs: usize) -> Self {
        Self {
            horizon,
            monte_carlo_runs,
            passives: Vec::new(),
            events: Vec::new(),
            projects: Vec::new(),
            active_projects: Vec::new(),
        }
    }

    /// Run the Monte Carlo simulation.
    ///
    /// # Flow (per run)
    ///
    /// 1. Check preconditions — if not met, returns `SimulationResult::unavailable()`
    /// 2. Clone initial state
    /// 3. Apply deterministic decision cost
    /// 4. Sample outcome branch (weighted random from eligible outcomes)
    /// 5. Apply chosen outcome's transform
    /// 6. Clamp state to bounds
    /// 7. For each forward step (1..horizon):
    ///    a. Tick passive effects
    ///    b. Clamp state
    ///    c. Record utility via `goal.utility(&state)`
    /// 8. Record final state
    pub fn run(
        &mut self,
        initial_state: &DynamicState,
        decision: &Decision,
        goal: &GoalVector,
    ) -> SimulationResult {
        let dim = initial_state.len();
        assert_eq!(
            goal.dimension(),
            dim,
            "goal dimension {} must match state dimension {}",
            goal.dimension(),
            dim
        );

        // Build precondition: All conditions must hold
        let precond_boxes: Vec<Box<dyn Predicate>> = decision
            .preconditions
            .iter()
            .map(|c| Box::new(c.clone()) as Box<dyn Predicate>)
            .collect();
        let precondition = All(precond_boxes);

        // Build cost: apply all cost effects sequentially
        let cost_boxes: Vec<Box<dyn Action>> = decision
            .cost
            .iter()
            .map(|e| Box::new(e.clone()) as Box<dyn Action>)
            .collect();
        let cost = Sequence(cost_boxes);

        // Build outcome condition boxes (must outlive the references)
        let outcome_cond_boxes: Vec<Option<Box<dyn Predicate>>> = decision
            .outcomes
            .iter()
            .map(|o| {
                o.condition
                    .as_ref()
                    .map(|c| Box::new(c.clone()) as Box<dyn Predicate>)
            })
            .collect();

        // Build outcome transform boxes
        let outcome_transform_boxes: Vec<Box<dyn Action>> = decision
            .outcomes
            .iter()
            .map(|o| Box::new(o.transform.clone()) as Box<dyn Action>)
            .collect();

        // Build branches slice with references to the boxes
        let branches: Vec<(f64, Option<&dyn Predicate>, &dyn Action)> = decision
            .outcomes
            .iter()
            .enumerate()
            .map(|(i, o)| {
                (
                    o.weight,
                    outcome_cond_boxes[i]
                        .as_ref()
                        .map(|b| b.as_ref() as &dyn Predicate),
                    outcome_transform_boxes[i].as_ref() as &dyn Action,
                )
            })
            .collect();

        self.run_dynamic(initial_state, &precondition, &cost, &branches, goal)
    }

    /// Run the Monte Carlo simulation using trait objects.
    ///
    /// This is the trait-based entry point. [`Self::run`] delegates here after
    /// wrapping concrete types in trait objects.
    ///
    /// # Parameters
    ///
    /// * `precondition` — must evaluate to `true` for the run to proceed
    /// * `cost` — deterministic action applied before outcome sampling
    /// * `outcomes` — weighted branches: `(weight, optional_guard, action)`
    /// * `valuation` — scores state after each step
    pub fn run_dynamic(
        &mut self,
        initial_state: &DynamicState,
        precondition: &dyn Predicate,
        cost: &dyn Action,
        outcomes: &[(f64, Option<&dyn Predicate>, &dyn Action)],
        valuation: &dyn Valuation,
    ) -> SimulationResult {
        // Check preconditions
        if !precondition.evaluate(initial_state) {
            return SimulationResult {
                decision_available: false,
                schedule_aborted: 0,
                final_states: Vec::new(),
                utility_traces: Vec::new(),
                outcome_counts: HashMap::new(),
            };
        }

        let mut rng = StdRng::from_entropy();
        let mut final_states = Vec::with_capacity(self.monte_carlo_runs);
        let mut utility_traces = Vec::with_capacity(self.monte_carlo_runs);
        let mut outcome_counts: HashMap<(usize, usize), usize> = HashMap::new();

        for _run in 0..self.monte_carlo_runs {
            // 1. Clone initial state
            let mut state: Vec<f64> = initial_state.as_slice().to_vec();

            // 2. Apply cost
            cost.apply(&mut state);

            // 3. Sample outcome
            let outcome_idx = self.sample_outcome_dynamic(outcomes, &state, &mut rng);
            *outcome_counts.entry((0, outcome_idx)).or_insert(0) += 1;

            // 4. Apply outcome transform
            outcomes[outcome_idx].2.apply(&mut state);

            // 5. Clamp after decision
            Self::clamp_state(&mut state, initial_state);

            // 6. Forward simulation
            let mut run_utility = Vec::with_capacity(self.horizon + 1);

            // Event tracking state (active suppression and chain triggers)
            let mut active_ids: HashSet<usize> = HashSet::new();
            let mut fired_prev: HashSet<usize> = HashSet::new();
            let resolved_prev: HashSet<usize> = HashSet::new(); // no duration tracking yet

            // Record utility at step 0 (post-decision, pre-passives)
            run_utility.push(valuation.score(&state));

            for step in 1..=self.horizon {
                // Tick passive effects
                for passive in &self.passives {
                    if passive.should_fire(&state, step) {
                        passive.apply(&mut state);
                    }
                }

                // Clamp after passives
                Self::clamp_state(&mut state, initial_state);

                // --- Event firing (pure core shared with TimelineStore) ---
                let mut fired_this_step: HashSet<usize> = HashSet::new();
                if !self.events.is_empty() {
                    let firing_order = crate::events::determine_firing(
                        &self.events,
                        &state,
                        &active_ids,
                        &fired_prev,
                        &resolved_prev,
                    );

                    for &evt_idx in &firing_order {
                        let evt = &self.events[evt_idx];
                        for action in &evt.effects {
                            action.apply(&mut state);
                        }
                        fired_this_step.insert(evt_idx);
                    }

                    // Clamp after event effects
                    Self::clamp_state(&mut state, initial_state);

                    // Update tracking for next step
                    fired_prev = fired_this_step.clone();
                    active_ids = fired_prev.clone(); // events remain active for one step
                }

                // --- Project tick (advance timed decisions) ---
                if !self.active_projects.is_empty() {
                    let mut completed: Vec<usize> = Vec::new();
                    let mut interrupted: Vec<usize> = Vec::new();

                    for ap in &mut self.active_projects {
                        if ap.remaining == 0 {
                            continue; // already completed, awaiting cleanup
                        }

                        // Check interruption: did any event fire this step that interrupts this project?
                        let project = &self.projects[ap.project_id];
                        if let Some(ref ic) = project.interrupt {
                            let was_interrupted = ic.event_ids.iter().any(|eid| fired_this_step.contains(eid));
                            if was_interrupted {
                                ic.on_interrupt.apply(&mut state);
                                Self::clamp_state(&mut state, initial_state);
                                interrupted.push(ap.project_id);
                                continue;
                            }
                        }

                        ap.remaining = ap.remaining.saturating_sub(1);
                        if ap.remaining == 0 {
                            // Project completed!
                            project.on_complete.apply(&mut state);
                            Self::clamp_state(&mut state, initial_state);
                            completed.push(ap.project_id);
                        }
                    }

                    // Clean up completed/interrupted projects from active list
                    self.active_projects.retain(|ap| {
                        !completed.contains(&ap.project_id)
                            && !interrupted.contains(&ap.project_id)
                    });
                }

                // Record utility
                run_utility.push(valuation.score(&state));
            }

            final_states.push(state);
            utility_traces.push(run_utility);
        }

        SimulationResult {
            decision_available: true,
            schedule_aborted: 0,
            final_states,
            utility_traces,
            outcome_counts,
        }
    }

    /// Sample an outcome index from trait-object based outcome branches.
    fn sample_outcome_dynamic(
        &self,
        outcomes: &[(f64, Option<&dyn Predicate>, &dyn Action)],
        state: &[f64],
        rng: &mut StdRng,
    ) -> usize {
        // Build sampling pool: only eligible outcomes
        let pool: Vec<(usize, f64)> = outcomes
            .iter()
            .enumerate()
            .filter(|(_, (_, guard, _))| guard.map_or(true, |g| g.evaluate(state)))
            .map(|(i, (w, _, _))| (i, *w))
            .collect();

        if pool.is_empty() {
            return rng.gen_range(0..outcomes.len());
        }

        let total_weight: f64 = pool.iter().map(|(_, w)| w).sum();

        if total_weight <= 0.0 {
            let idx = rng.gen_range(0..pool.len());
            return pool[idx].0;
        }

        let roll: f64 = rng.r#gen::<f64>() * total_weight;
        let mut cumulative = 0.0;

        for (idx, weight) in &pool {
            cumulative += weight;
            if roll < cumulative {
                return *idx;
            }
        }

        // Fallback (floating-point edge case)
        pool.last().map(|(i, _)| *i).unwrap_or(0)
    }

    /// Run a schedule of decisions at specified steps during the simulation horizon.
    ///
    /// # Flow (per run)
    ///
    /// For each step 0..horizon:
    /// 1. Check if any decisions are scheduled at this step
    /// 2. For each scheduled decision:
    ///    a. Check preconditions — if fail and required=true, abort the run
    ///    b. Apply decision cost
    ///    c. Sample outcome
    ///    d. Apply outcome transform
    ///    e. Clamp state
    /// 3. Tick passive effects
    /// 4. Clamp state
    /// 5. Record utility
    pub fn run_schedule(
        &mut self,
        initial_state: &DynamicState,
        schedule: &crate::event::DecisionSchedule,
        goal: &GoalVector,
    ) -> SimulationResult {
        let dim = initial_state.len();
        assert_eq!(
            goal.dimension(),
            dim,
            "goal dimension {} must match state dimension {}",
            goal.dimension(),
            dim
        );

        let mut rng = StdRng::from_entropy();
        let mut final_states = Vec::with_capacity(self.monte_carlo_runs);
        let mut utility_traces = Vec::with_capacity(self.monte_carlo_runs);
        let mut outcome_counts: HashMap<(usize, usize), usize> = HashMap::new();
        let mut schedule_aborted = 0usize;

        for _run in 0..self.monte_carlo_runs {
            let mut state: Vec<f64> = initial_state.as_slice().to_vec();
            let mut run_utility = Vec::with_capacity(self.horizon + 1);
            let mut aborted = false;

            for step in 0..=self.horizon {
                // 1. Apply scheduled decisions at this step
                for (entry_idx, sched) in schedule.entries.iter().enumerate() {
                    if sched.at_step != step {
                        continue;
                    }
                    // Check preconditions
                    if !sched.decision.available(&state) {
                        if sched.required {
                            aborted = true;
                            break;
                        }
                        // Optional — skip
                        continue;
                    }

                    // Apply cost
                    for cost_effect in &sched.decision.cost {
                        cost_effect.apply(&mut state);
                    }

                    // Sample outcome
                    let outcome_idx =
                        self.sample_outcome(&sched.decision.outcomes, &state, &mut rng);
                    *outcome_counts.entry((entry_idx, outcome_idx)).or_insert(0) += 1;

                    // Apply outcome transform
                    sched.decision.outcomes[outcome_idx]
                        .transform
                        .apply(&mut state);

                    // Clamp
                    Self::clamp_state(&mut state, initial_state);
                }

                if aborted {
                    break;
                }

                // 2. Tick passives
                for passive in &self.passives {
                    if passive.should_fire(&state, step) {
                        passive.apply(&mut state);
                    }
                }
                Self::clamp_state(&mut state, initial_state);

                // 3. Record utility
                run_utility.push(goal.utility(&state));
            }

            if aborted {
                schedule_aborted += 1;
            }
            final_states.push(state);
            utility_traces.push(run_utility);
        }

        // decision_available: check if the first scheduled decision is available
        let decision_available = schedule
            .entries
            .first()
            .map_or(true, |s| s.decision.available(initial_state));

        SimulationResult {
            decision_available,
            schedule_aborted,
            final_states,
            utility_traces,
            outcome_counts,
        }
    }
    ///
    /// Returns the index into `outcomes` that was selected. Uses `condition` guards
    /// to filter the pool (outcomes whose condition fails the current state are excluded).
    fn sample_outcome(
        &self,
        outcomes: &[Outcome],
        state: &[f64],
        rng: &mut StdRng,
    ) -> usize {
        // Build sampling pool: only eligible outcomes
        let pool: Vec<(usize, f64)> = outcomes
            .iter()
            .enumerate()
            .filter(|(_, o)| o.condition.as_ref().map_or(true, |c| c.check(state)))
            .map(|(i, o)| (i, o.weight))
            .collect();

        // Shouldn't happen in practice (a decision with zero eligible outcomes is malformed),
        // but guard against it.
        if pool.is_empty() {
            // Fallback: choose uniformly from all outcomes, ignoring conditions.
            // This is a safety valve; real decisions should always have at least one
            // eligible outcome (use a condition-free fallback branch).
            return rng.gen_range(0..outcomes.len());
        }

        let total_weight: f64 = pool.iter().map(|(_, w)| w).sum();

        if total_weight <= 0.0 {
            // All zero weights — uniform random over pool
            let idx = rng.gen_range(0..pool.len());
            return pool[idx].0;
        }

        let roll: f64 = rng.r#gen::<f64>() * total_weight;
        let mut cumulative = 0.0;

        for (idx, weight) in &pool {
            cumulative += weight;
            if roll < cumulative {
                return *idx;
            }
        }

        // Fallback (floating-point edge case)
        pool.last().map(|(i, _)| *i).unwrap_or(0)
    }

    /// Apply attribute bounds clamping from the schema.
    fn clamp_state(state: &mut [f64], dynamic_state: &DynamicState) {
        let schema = dynamic_state.schema();
        for (i, attr) in schema.attributes.iter().enumerate() {
            if let Some((min, max)) = attr.bounds {
                state[i] = state[i].clamp(min, max);
            }
        }
    }

    /// Run simulation and return a fully analyzed result.
    ///
    /// Convenience method that calls [`Self::run`] and then computes
    /// [`DecisionAnalysis`](crate::DecisionAnalysis) from the raw output.
    pub fn run_and_analyze(
        &mut self,
        initial_state: &DynamicState,
        decision: &Decision,
        goal: &GoalVector,
    ) -> crate::scoring::DecisionAnalysis {
        let result = self.run(initial_state, decision, goal);
        crate::scoring::DecisionAnalysis::from_result(&result)
    }
}

// ── Batch comparison ─────────────────────────────────────────────────────────────────

/// Simulate N decisions against the same state, with shared passives.
///
/// Creates a fresh `Simulation` for each decision, runs the full Monte Carlo
/// analysis, and returns ranked results sorted by mean utility descending.
///
/// # Parameters
/// * `state` — initial state all decisions are evaluated against
/// * `decisions` — slice of `(label, decision)` pairs to compare
/// * `passives` — passive effects shared across all simulations
/// * `events` — resolved event templates (accepted but not cloned — each simulation
///   gets an empty event set, since `ResolvedEvent` uses non-Clone trait objects)
/// * `goal` — goal vector for scoring
/// * `horizon` — simulation horizon in steps
/// * `runs` — number of Monte Carlo runs per decision
pub fn batch_compare(
    state: &DynamicState,
    decisions: &[(String, &Decision)],
    passives: &[PassiveEffect],
    _events: &[ResolvedEvent],
    goal: &GoalVector,
    horizon: usize,
    runs: usize,
) -> Vec<(String, DecisionAnalysis)> {
    let mut results: Vec<(String, DecisionAnalysis)> = Vec::with_capacity(decisions.len());

    for (label, decision) in decisions {
        let mut sim = Simulation::new(horizon, runs);
        sim.passives = passives.to_vec();
        // Events left empty: ResolvedEvent contains Box<dyn Predicate/Box<dyn Action>
        // trait objects that don't implement Clone.
        let analysis = sim.run_and_analyze(state, decision, goal);
        results.push((label.clone(), analysis));
    }

    // Sort by utility distribution mean descending
    results.sort_by(|a, b| {
        b.1.utility_distribution
            .mean
            .partial_cmp(&a.1.utility_distribution.mean)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    results
}

// ── Tests ────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::{AttributeEffect, ComparisonOp, Condition, Decision, InterruptConfig,
        Outcome, Transform};
    use crate::schema::AttributeSchema;
    use std::sync::Arc;

    fn test_schema() -> Arc<AttributeSchema> {
        Arc::new(
            AttributeSchema::from_json(
                r#"{
                "version": 1,
                "attributes": [
                    {"name": "wealth", "unit": "$"},
                    {"name": "health", "unit": "pts", "bounds": [0, 100]},
                    {"name": "stress", "unit": "pts", "bounds": [0, 100]}
                ]
            }"#,
            )
            .unwrap(),
        )
    }

    fn test_decision() -> Decision {
        Decision {
            id: "test_decision".into(),
            label: "Test Decision".into(),
            preconditions: vec![],
            cost: vec![],
            outcomes: vec![
                Outcome {
                    label: "".into(),
                    weight: 1.0,
                    condition: None,
                    transform: Transform::simple(vec![AttributeEffect::fixed(0, 100.0)]),
                },
            ],
        }
    }

    #[test]
    fn simulation_runs_to_completion() {
        let schema = test_schema();
        let initial = DynamicState::from_vec(vec![1000.0, 100.0, 0.0], schema);
        let decision = test_decision();
        let mut sim = Simulation::new(5, 10);
        let goal = GoalVector::linear(vec![1.0, 0.5, -0.3]); // wealth+, health+, stress-

        let result = sim.run(&initial, &decision, &goal);

        assert!(result.decision_available);
        assert_eq!(result.final_states.len(), 10);
        assert_eq!(result.utility_traces.len(), 10);
        assert_eq!(result.utility_traces[0].len(), 6); // step 0 + 5 horizon steps = 6

        // Wealth should be 1100.0 (1000 + 100 from outcome)
        for state in &result.final_states {
            assert!((state[0] - 1100.0).abs() < f64::EPSILON);
        }
    }

    #[test]
    fn decision_unavailable_when_preconditions_fail() {
        let schema = test_schema();
        let initial = DynamicState::from_vec(vec![10.0, 100.0, 0.0], schema);
        let decision = Decision {
            id: "needs_wealth".into(),
            label: "Needs Wealth".into(),
            preconditions: vec![Condition {
                attribute_index: 0,
                operator: ComparisonOp::Gt,
                value: 50.0,
            }],
            cost: vec![],
            outcomes: vec![],
        };
        let mut sim = Simulation::new(5, 10);
        let goal = GoalVector::linear(vec![1.0, 0.5, -0.3]);

        let result = sim.run(&initial, &decision, &goal);
        assert!(!result.decision_available);
        assert!(result.final_states.is_empty());
    }

    #[test]
    fn outcome_sampling_respects_weights() {
        let schema = test_schema();
        let initial = DynamicState::from_vec(vec![1000.0, 100.0, 0.0], schema);
        let decision = Decision {
            id: "weighted".into(),
            label: "Weighted".into(),
            preconditions: vec![],
            cost: vec![],
            outcomes: vec![
                Outcome {
                    label: "".into(),
                    weight: 90.0,
                    condition: None,
                    transform: Transform::simple(vec![AttributeEffect::fixed(0, 1.0)]),
                },
                Outcome {
                    label: "".into(),
                    weight: 10.0,
                    condition: None,
                    transform: Transform::simple(vec![AttributeEffect::fixed(0, 1000.0)]),
                },
            ],
        };
        let mut sim = Simulation::new(1, 1000);
        let goal = GoalVector::linear(vec![1.0, 0.0, 0.0]);

        let result = sim.run(&initial, &decision, &goal);

        // The 10% outcome should appear roughly 10% of the time
        let count_rare = *result.outcome_counts.get(&(0, 1)).unwrap_or(&0);
        // With 1000 runs and p=0.1, 95% CI is roughly 81–119.
        // Use a generous tolerance for CI robustness in CI.
        assert!(count_rare >= 70, "rare outcome undersampled: {count_rare}");
        assert!(count_rare <= 130, "rare outcome oversampled: {count_rare}");
    }

    #[test]
    fn outcome_condition_gating() {
        let schema = test_schema();
        // high stress → only the "bad" outcome is eligible
        let initial = DynamicState::from_vec(vec![1000.0, 100.0, 90.0], schema);
        let decision = Decision {
            id: "gated".into(),
            label: "Gated".into(),
            preconditions: vec![],
            cost: vec![],
            outcomes: vec![
                Outcome {
                    label: "".into(),
                    weight: 1.0,
                    condition: Some(Condition {
                        attribute_index: 2,
                        operator: ComparisonOp::Lt,
                        value: 50.0,
                    }),
                    transform: Transform::simple(vec![AttributeEffect::fixed(0, 1000.0)]),
                },
                Outcome {
                    label: "".into(),
                    weight: 1.0,
                    condition: None,
                    transform: Transform::simple(vec![AttributeEffect::fixed(0, -500.0)]),
                },
            ],
        };
        let mut sim = Simulation::new(1, 100);
        let goal = GoalVector::linear(vec![1.0, 0.0, 0.0]);

        let result = sim.run(&initial, &decision, &goal);

        // Stress is 90, so the first outcome (condition: stress < 50) is excluded.
        // Only the second outcome (index 1) should ever be sampled.
        assert_eq!(
            result.outcome_counts.len(),
            1,
            "only one outcome should be sampled"
        );
        assert!(result.outcome_counts.contains_key(&(0, 1)));
        assert!(!result.outcome_counts.contains_key(&(0, 0)));
    }

    #[test]
    fn passives_tick_correctly() {
        let schema = test_schema();
        let initial = DynamicState::from_vec(vec![1000.0, 100.0, 0.0], schema);
        let decision = test_decision();
        let mut sim = Simulation::new(3, 1);
        sim.passives.push(PassiveEffect::every_step(
            "income",
            "Salary",
            vec![AttributeEffect::fixed(0, 50.0)],
        ));
        let goal = GoalVector::linear(vec![1.0, 0.0, 0.0]);

        let result = sim.run(&initial, &decision, &goal);

        let final_state = &result.final_states[0];
        // Initial: 1000 + 100 (decision) + 50×3 (passives × 3 steps) = 1250
        assert!((final_state[0] - 1250.0).abs() < f64::EPSILON);
    }

    #[test]
    fn passive_every_n_steps() {
        let schema = test_schema();
        let initial = DynamicState::from_vec(vec![0.0, 100.0, 0.0], schema);
        let decision = test_decision(); // +100 to wealth[0]
        let mut sim = Simulation::new(4, 1);
        sim.passives.push(PassiveEffect::every_n(
            "bonus",
            "Quarterly Bonus",
            2, // every 2 steps
            vec![AttributeEffect::fixed(0, 200.0)],
        ));
        let goal = GoalVector::linear(vec![1.0, 0.0, 0.0]);

        let result = sim.run(&initial, &decision, &goal);

        let final_state = &result.final_states[0];
        // Decision: +100
        // Steps: 1 (no bonus), 2 (bonus +200), 3 (no bonus), 4 (bonus +200)
        // = 100 + 200 + 200 = 500
        assert!((final_state[0] - 500.0).abs() < f64::EPSILON);
    }

    #[test]
    fn clamping_respects_bounds() {
        let schema = test_schema();
        // health at 95, we add 20 → should clamp at 100
        let initial = DynamicState::from_vec(vec![1000.0, 95.0, 0.0], schema);
        let decision = Decision {
            id: "heal".into(),
            label: "Heal".into(),
            preconditions: vec![],
            cost: vec![],
            outcomes: vec![Outcome {
                label: "".into(),
                weight: 1.0,
                condition: None,
                transform: Transform::simple(vec![AttributeEffect::fixed(1, 20.0)]),
            }],
        };
        let mut sim = Simulation::new(1, 1);
        let goal = GoalVector::linear(vec![0.0, 1.0, 0.0]);

        let result = sim.run(&initial, &decision, &goal);
        assert!((result.final_states[0][1] - 100.0).abs() < f64::EPSILON);
    }

    #[test]
    fn utility_trace_length() {
        let schema = test_schema();
        let initial = DynamicState::from_vec(vec![1000.0, 100.0, 0.0], schema);
        let decision = test_decision();
        let mut sim = Simulation::new(24, 1);
        let goal = GoalVector::linear(vec![1.0, 0.5, -0.3]);

        let result = sim.run(&initial, &decision, &goal);

        // Horizon 24: step 0 (post-decision) + 24 forward steps = 25 entries
        assert_eq!(result.utility_traces[0].len(), 25);
    }

    #[test]
    fn project_completes_after_duration() {
        let schema = test_schema();
        let initial = DynamicState::from_vec(vec![0.0, 100.0, 0.0], schema);
        let decision = test_decision(); // +100 to wealth[0]
        let mut sim = Simulation::new(3, 1);
        sim.projects.push(Project {
            id: "learn_rust".into(),
            label: "Learn Rust".into(),
            preconditions: vec![],
            cost: vec![],
            duration: 2,
            on_complete: Transform::simple(vec![AttributeEffect::fixed(2, 50.0)]), // +50 to stress[2]
            interrupt: None,
        });
        sim.active_projects.push(ActiveProject {
            project_id: 0,
            remaining: 2,
        });
        let goal = GoalVector::linear(vec![1.0, 0.0, 0.0]);

        let result = sim.run(&initial, &decision, &goal);
        let final_state = &result.final_states[0];

        // Wealth: 0 + 100 (decision outcome) = 100
        assert!((final_state[0] - 100.0).abs() < f64::EPSILON);
        // Stress: 0 + 50 (project completes at step 2) = 50
        assert!((final_state[2] - 50.0).abs() < f64::EPSILON);
    }

    #[test]
    fn project_interrupted_by_event() {
        use crate::events::ResolvedEvent;
        use crate::traits::All;

        let schema = test_schema();
        let initial = DynamicState::from_vec(vec![0.0, 100.0, 0.0], schema);
        let decision = test_decision();
        let mut sim = Simulation::new(3, 1);

        // Add an event that fires every step (precondition always true)
        sim.events.push(ResolvedEvent {
            id: 0,
            label: "disaster".into(),
            precondition: Box::new(All(vec![])),
            effects: vec![Box::new(AttributeEffect::fixed(1, -5.0))], // -5 health
            triggered_by: vec![],
            suppressed_by: vec![],
            priority: 0,
            delay: 0,
            duration: 0,
            cooldown: 0,
            triggers_event_id: None,
            triggers_on_resolve_id: None,
        });

        // Add a project that is interrupted by event 0
        sim.projects.push(Project {
            id: "fragile".into(),
            label: "Fragile Project".into(),
            preconditions: vec![],
            cost: vec![],
            duration: 3,
            on_complete: Transform::simple(vec![AttributeEffect::fixed(0, 1000.0)]),
            interrupt: Some(InterruptConfig {
                event_ids: vec![0],
                on_interrupt: Transform::simple(vec![AttributeEffect::fixed(2, 30.0)]), // +30 stress
            }),
        });
        sim.active_projects.push(ActiveProject {
            project_id: 0,
            remaining: 3,
        });
        let goal = GoalVector::linear(vec![1.0, 0.0, -1.0]);

        let result = sim.run(&initial, &decision, &goal);
        let final_state = &result.final_states[0];

        // Project should be interrupted (event 0 fires every step)
        // Stress should be +30 from interrupt effect
        assert!((final_state[2] - 30.0).abs() < f64::EPSILON);
        // Wealth should NOT get the +1000 completion bonus
        assert!((final_state[0] - 100.0).abs() < f64::EPSILON); // just the decision outcome
    }
}
