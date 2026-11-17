//! `lovelace-systems` — exact analytical systems modeling and performance analysis.
//!
//! The crate is independent of the interpreter: it depends only on the workspace's canonical
//! numeric kernel (`lovelace-nat`, `lovelace-int`, `lovelace-rational`), contains no floating point
//! and no `unsafe` code, and is usable from ordinary Rust. The Lovelace language reaches it through
//! a thin adapter that converts `Value` records to the typed models below; no formula lives in the
//! adapter.
//!
//! # What it computes
//!
//! - [`queueing`]: M/M/1, M/M/c (Erlang C) and M/M/infinity single-station metrics.
//! - [`operational_laws`]: Little's law, the utilization law, the forced-flow law and the
//!   interactive response-time law, each with an exact check.
//! - [`networks`]: open Jackson networks with explicit visit ratios.
//! - [`mva`]: exact single-class Mean Value Analysis for closed product-form networks with
//!   single-server FCFS stations and delay centers.
//! - [`capacity`]: bottleneck and throughput-bound analysis, plus what-if scenario comparison.
//! - [`sweep`]: bounded population and arrival-rate sweeps.
//!
//! # Exactness
//!
//! Every reported number is an exact [`Q`] rational when the input model is rational (which every
//! model is: [`Q`] is the only numeric type). "Exact" in a result is a claim about the model's
//! mathematical solution at the chosen precision, not about the world the model describes and not
//! a certified floating-point error bound. A result that used an algorithmic approximation says so
//! in its provenance; no P0 solver does. The rendering precision only controls decimal output at
//! the language and wire boundary through `Real::from_rational`, which preserves
//! `lovelace-real`'s period metadata and reports a truncated projection explicitly.
//!
//! # Shared rational kernel
//!
//! [`Q`] is a re-export of `lovelace_rational::Rat`, the workspace's one general-purpose rational
//! implementation. This crate owns queueing and operations-research formulas, not arithmetic: it
//! contains no numerator/denominator struct, no private GCD normalization, and no parallel scalar
//! promotion rules. The migration deleted `src/numeric.rs`; the alias exists only so the systems
//! API keeps its historical `Q` spelling (RNL-MIG-002). The alias is type-identical to `Rat`.
//!
//! # Precision, budgets and cancellation
//!
//! [`SolverOptions`] carries a decimal rendering precision and hard budgets (iterations, retained
//! population entries, exact-intermediate digits). A budget that trips returns a categorized
//! `BudgetExceeded` error; a partially computed result is never published as successful. A
//! [`Cancel`] token is polled in every long loop.
//!
//! # Example
//!
//! ```rust
//! use lovelace_systems::{mm1, NeverCancel, Q, SolverOptions};
//!
//! let result = mm1(
//!     &Q::from_i64(8),
//!     &Q::from_i64(10),
//!     &SolverOptions::default(),
//!     &NeverCancel,
//! )?;
//! let station = &result.stations[0];
//! assert_eq!(station.utilization.to_string(), "4/5");
//! assert_eq!(station.response_time.to_string(), "1/2");
//! assert_eq!(station.occupancy.to_string(), "4");
//! assert!(result.laws_hold());
//! # Ok::<(), lovelace_systems::SystemsError>(())
//! ```

#![forbid(unsafe_code)]
#![deny(missing_docs)]
#![allow(clippy::too_many_arguments)]

pub mod capacity;
pub mod errors;
pub mod model;
pub mod mva;
pub mod networks;
pub mod operational_laws;
pub mod queueing;
pub mod results;
pub mod sweep;
pub mod units;
pub mod validation;

pub use capacity::{
    BottleneckAnalysis, BottleneckEntry, BoundBasis, Comparison, Scenario, bottlenecks,
    changed_fields, closed_with, compare, open_with,
};
pub use errors::{ErrorCategory, SystemsError};
pub use lovelace_rational::Rat as Q;
pub use model::{
    Cancel, ClosedWorkload, FlagCancel, NeverCancel, OpenWorkload, QueueNetwork, ServiceDiscipline,
    SolverOptions, Station, StationId, Workload,
};
pub use mva::{MVA_ASSUMPTIONS, analyze_closed};
pub use networks::{JACKSON_ASSUMPTIONS, analyze_open};
pub use operational_laws::{
    check_forced_flow, check_interactive, check_little, check_utilization, forced_flow,
    interactive_response, little_law, utilization_law,
};
pub use queueing::{mm_infinity, mm1, mmc, solve_station};
pub use results::{
    AggregateResult, AnalysisResult, Exactness, LawCheck, PopulationPoint, Provenance,
    SolverStatus, StationResult,
};
pub use sweep::{
    MAX_SWEEP_POINTS, SweepAxis, SweepPoint, SweepResult, sweep_arrival, sweep_population,
};
pub use units::{Demand, Dimension, Rate, RateUnit, Time, TimeUnit};
pub use validation::{MAX_SERVERS, check_stable, station_utilization, validate};

/// The solver/algorithms version published in every result's provenance.
pub const SOLVER_VERSION: &str = "0.5.0";

/// The crate version, available without a Cargo environment at run time.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Analyzes a model with the solver that matches its workload class.
///
/// An open model uses [`analyze_open`]; a closed model uses [`analyze_closed`]. The wrong workload
/// is refused by the solver with `UnsupportedModel`; it is never coerced.
pub fn analyze(
    network: &QueueNetwork,
    options: &SolverOptions,
    cancel: &dyn Cancel,
) -> Result<AnalysisResult, SystemsError> {
    validate(network)?;
    match &network.workload {
        Workload::Open(_) => analyze_open(network, options, cancel),
        Workload::Closed(_) => analyze_closed(network, options, cancel),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Station;

    fn station(id: &str, discipline: ServiceDiscipline, service_time: Q, visits: Q) -> Station {
        Station::new(id, discipline, service_time, visits).expect("valid station")
    }

    #[test]
    fn the_dispatcher_chooses_by_workload_and_never_coerces() {
        let open = QueueNetwork::new(
            "open",
            Workload::Open(OpenWorkload {
                arrival_rate: Q::from_i64(2),
            }),
            vec![station(
                "api",
                ServiceDiscipline::FcfsSingleServer,
                Q::from_i64(1).div(&Q::from_i64(10)).unwrap(),
                Q::one(),
            )],
        )
        .expect("valid");
        let result = analyze(&open, &SolverOptions::default(), &NeverCancel).expect("solved");
        assert_eq!(result.provenance.model_class, "open-jackson");

        let closed = QueueNetwork::new(
            "closed",
            Workload::Closed(ClosedWorkload {
                population: 1,
                think_time: Q::zero(),
            }),
            vec![station(
                "api",
                ServiceDiscipline::FcfsSingleServer,
                Q::from_i64(1).div(&Q::from_i64(10)).unwrap(),
                Q::one(),
            )],
        )
        .expect("valid");
        let result = analyze(&closed, &SolverOptions::default(), &NeverCancel).expect("solved");
        assert_eq!(result.provenance.model_class, "closed-mva");
        assert_eq!(result.provenance.solver_version, SOLVER_VERSION);
    }

    // ---------------------------------------------------------------------------------------
    // The systems-facing rational contract. These moved here from the deleted `numeric.rs` and
    // exercise the canonical kernel through the `Q` alias the systems API publishes.
    // ---------------------------------------------------------------------------------------

    fn q(num: i64, den: u64) -> Q {
        Q::from_parts(
            lovelace_int::Int::from_i64(num),
            lovelace_nat::Nat::from_u64(den),
        )
        .expect("non-zero denominator")
    }

    #[test]
    fn the_q_alias_is_the_canonical_kernel_and_still_has_the_expected_semantics() {
        // The alias is type-identical, not a wrapper with parallel arithmetic.
        let canonical: lovelace_rational::Rat = q(1, 2);
        let through_alias: Q = canonical.clone();
        assert_eq!(through_alias, canonical);

        assert_eq!(q(1, 3).add(&q(1, 6)), q(1, 2));
        assert_eq!(q(2, 4), q(1, 2));
        assert_eq!(q(-2, 4), q(-1, 2));
        assert_eq!(q(1, 3).sub(&q(1, 3)), Q::zero());
        assert_eq!(q(3, 1).to_i64(), Some(3));
    }

    #[test]
    fn the_systems_surface_preserves_the_signed_division_regression() {
        let result = q(1, 2).checked_div(&q(-1, 4)).expect("non-zero divisor");
        assert_eq!(result, Q::from_i64(-2));
        assert_eq!(result.to_string(), "-2");
    }

    #[test]
    fn large_products_stay_reduced_and_division_by_zero_is_refused() {
        let left = q(99991, 99989);
        let right = q(99989, 99991);
        assert_eq!(left.mul(&right), Q::one());
        assert_eq!(left.mul(&right).denominator().to_decimal(), "1");
        let sum = left.add(&right);
        assert_eq!(sum.denominator().to_decimal(), "9998000099");
        assert!(sum.decimal_digits() < 40);
        assert!(q(1, 2).div(&Q::zero()).is_none());
        assert!(Q::from_parts(lovelace_int::Int::from_i64(1), lovelace_nat::Nat::zero()).is_err());
        assert!(Q::zero().pow_i64(-1).is_none());
    }

    #[test]
    fn ordering_is_numeric_not_lexical() {
        assert!(q(1, 3) < q(1, 2));
        assert!(q(-1, 2) < q(-1, 3));
        assert_eq!(q(2, 4).cmp(&q(1, 2)), std::cmp::Ordering::Equal);
    }
}
