//! Trait-based abstraction layer for the Loom engine.
//!
//! Defines three core traits — [`Predicate`], [`Action`], [`Valuation`] — that
//! allow the simulation engine to operate on any types implementing them,
//! plus compositor types for building complex logic from simple pieces.

use rand::Rng;
use std::fmt::Debug;

/// A boolean condition evaluated against a state slice.
pub trait Predicate: Debug {
    /// Return `true` when the condition holds for the given state.
    fn evaluate(&self, state: &[f64]) -> bool;
}

/// A state mutation applied to a mutable state slice.
pub trait Action: Debug {
    /// Apply this action, mutating the state in place.
    fn apply(&self, state: &mut [f64]);
}

/// A numeric score computed from a state slice.
pub trait Valuation: Debug {
    /// Compute the score/utility of the given state.
    fn score(&self, state: &[f64]) -> f64;
}

// ── Predicate compositors ─────────────────────────────────────────────────────────

/// Logical AND of multiple predicates. All must evaluate to `true`.
#[derive(Debug)]
pub struct All(pub Vec<Box<dyn Predicate>>);

impl Predicate for All {
    fn evaluate(&self, state: &[f64]) -> bool {
        self.0.iter().all(|p| p.evaluate(state))
    }
}

/// Logical OR of multiple predicates. At least one must evaluate to `true`.
#[derive(Debug)]
pub struct Any(pub Vec<Box<dyn Predicate>>);

impl Predicate for Any {
    fn evaluate(&self, state: &[f64]) -> bool {
        self.0.iter().any(|p| p.evaluate(state))
    }
}

// ── Action compositors ────────────────────────────────────────────────────────────

/// Sequential application of multiple actions in order.
#[derive(Debug)]
pub struct Sequence(pub Vec<Box<dyn Action>>);

impl Action for Sequence {
    fn apply(&self, state: &mut [f64]) {
        for action in &self.0 {
            action.apply(state);
        }
    }
}

/// Conditional action: only applies if the guard predicate holds.
#[derive(Debug)]
pub struct When {
    pub guard: Box<dyn Predicate>,
    pub action: Box<dyn Action>,
}

impl Action for When {
    fn apply(&self, state: &mut [f64]) {
        if self.guard.evaluate(state) {
            self.action.apply(state);
        }
    }
}

/// Weighted random choice among multiple actions with optional guards.
///
/// Does not implement [`Action`] directly because weighted sampling requires
/// an external RNG. Use [`OneOf::sample_and_apply`] instead.
#[derive(Debug)]
pub struct OneOf {
    /// Branches: `(weight, optional_guard, action)`.
    pub branches: Vec<(f64, Option<Box<dyn Predicate>>, Box<dyn Action>)>,
}

impl OneOf {
    /// Sample a branch using the provided RNG and apply it to the state.
    /// Returns the index of the chosen branch.
    ///
    /// Only branches whose optional guard evaluates to `true` are eligible.
    /// If no branches are eligible, applies nothing and returns 0.
    pub fn sample_and_apply<R: Rng + ?Sized>(&self, state: &mut [f64], rng: &mut R) -> usize {
        let pool: Vec<(usize, f64)> = self
            .branches
            .iter()
            .enumerate()
            .filter(|(_, (_, guard, _))| guard.as_ref().map_or(true, |g| g.evaluate(state)))
            .map(|(i, (w, _, _))| (i, *w))
            .collect();

        if pool.is_empty() {
            return 0;
        }

        let total_weight: f64 = pool.iter().map(|(_, w)| w).sum();

        if total_weight <= 0.0 {
            let idx = rng.gen_range(0..pool.len());
            let branch_idx = pool[idx].0;
            self.branches[branch_idx].2.apply(state);
            return branch_idx;
        }

        let roll: f64 = rng.r#gen::<f64>() * total_weight;
        let mut cumulative = 0.0;
        for (idx, weight) in &pool {
            cumulative += weight;
            if roll < cumulative {
                self.branches[*idx].2.apply(state);
                return *idx;
            }
        }

        let last = pool.last().map(|(i, _)| *i).unwrap_or(0);
        self.branches[last].2.apply(state);
        last
    }
}
