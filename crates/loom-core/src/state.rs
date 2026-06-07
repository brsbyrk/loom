//! State vector abstraction — the engine operates on f64 slices, not domain types.
//!
//! Users define their own attribute space (e.g., `PersonalState`) by implementing
//! [`StateVector`]. The simulation loop reads/writes raw `Vec<f64>` for speed;
//! the trait bridges to/from domain types.

use std::fmt::Debug;

/// Trait for any domain type that can be represented as a fixed-length float vector.
///
/// # Safety / correctness contract
///
/// - `dimension()` must return the same value for all calls.
/// - `labels().len()` must equal `dimension()`.
/// - `to_vec().len()` must equal `dimension()`.
/// - `from_vec(v)` must round-trip: `from_vec(to_vec(&self))` must reconstruct the value.
pub trait StateVector: Clone + Debug + Send + Sync + 'static {
    /// Number of attributes in this space.
    fn dimension() -> usize;

    /// Human-readable names for each attribute index.
    fn labels() -> &'static [&'static str];

    /// Serialize to a flat float vector.
    fn to_vec(&self) -> Vec<f64>;

    /// Deserialize from a flat float vector.
    fn from_vec(v: &[f64]) -> Self;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Debug)]
    struct TestState {
        wealth: f64,
        health: f64,
    }

    impl StateVector for TestState {
        fn dimension() -> usize { 2 }
        fn labels() -> &'static [&'static str] { &["wealth", "health"] }
        fn to_vec(&self) -> Vec<f64> { vec![self.wealth, self.health] }
        fn from_vec(v: &[f64]) -> Self { Self { wealth: v[0], health: v[1] } }
    }

    #[test]
    fn round_trip() {
        let s = TestState { wealth: 50_000.0, health: 75.0 };
        let v = s.to_vec();
        let s2 = TestState::from_vec(&v);
        assert!((s2.wealth - s.wealth).abs() < f64::EPSILON);
        assert!((s2.health - s.health).abs() < f64::EPSILON);
    }

    #[test]
    fn dimension_matches_labels() {
        assert_eq!(TestState::dimension(), TestState::labels().len());
    }
}
