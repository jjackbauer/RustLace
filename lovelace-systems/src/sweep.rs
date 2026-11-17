//! Population and arrival-rate sweeps.
//!
//! A sweep is a batch of ordinary analyses over an immutable baseline model: each point copies the
//! model, changes exactly one parameter and calls the same solver. The point count is bounded by
//! `SolverOptions::max_iterations` and by [`MAX_SWEEP_POINTS`] before anything is computed, so a
//! caller cannot ask for an unbounded allocation. Results keep their declaration order and each
//! point carries its own provenance.

use crate::Q;
use crate::errors::SystemsError;
use crate::model::{Cancel, QueueNetwork, SolverOptions, Workload};
use crate::results::AnalysisResult;

/// The largest number of points one sweep may request.
pub const MAX_SWEEP_POINTS: u64 = 10_000;

/// The parameter a sweep varies.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SweepAxis {
    /// The closed population (integer points).
    Population,
    /// The open arrival rate (rational points).
    ArrivalRate,
}

impl SweepAxis {
    /// The stable label.
    pub fn as_str(self) -> &'static str {
        match self {
            SweepAxis::Population => "population",
            SweepAxis::ArrivalRate => "arrival_rate",
        }
    }
}

/// One swept point: the parameter value and the full analysis at it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SweepPoint {
    /// The parameter value (the population as a rational, or the arrival rate).
    pub parameter: Q,
    /// The analysis at that value.
    pub result: AnalysisResult,
}

/// A completed sweep.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SweepResult {
    /// What was varied.
    pub axis: SweepAxis,
    /// The points, in increasing parameter order.
    pub points: Vec<SweepPoint>,
    /// Non-fatal notes (a skipped point, a degenerate population).
    pub warnings: Vec<String>,
}

impl SweepResult {
    /// The largest station utilization in any point.
    pub fn max_utilization(&self) -> Q {
        self.points
            .iter()
            .flat_map(|point| point.result.stations.iter())
            .map(|station| station.utilization.clone())
            .fold(
                Q::zero(),
                |acc, value| if value > acc { value } else { acc },
            )
    }
}

fn check_count(count: u64, options: &SolverOptions) -> Result<(), SystemsError> {
    if count == 0 {
        return Err(SystemsError::type_mismatch(
            "A sweep needs at least one point (from <= to with a positive step)",
        ));
    }
    if count > MAX_SWEEP_POINTS {
        return Err(SystemsError::state_space_budget(MAX_SWEEP_POINTS, count));
    }
    options.check_iterations(count)
}

/// The one grid contract both axes share: an inclusive endpoint is legal only when the span is an
/// exact whole number of steps. The returned count includes both endpoints.
fn grid_point_count(from: &Q, to: &Q, step: &Q) -> Result<u64, SystemsError> {
    let span = to.sub(from);
    let steps = span
        .checked_div(step)
        .map_err(|_| SystemsError::type_mismatch("A sweep needs a positive step"))?;
    if !steps.is_integer() || steps.mul(step) != span || steps.is_negative() {
        return Err(SystemsError::sweep_endpoint_not_on_grid(
            &from.to_string(),
            &to.to_string(),
            &step.to_string(),
        ));
    }
    let steps = steps.to_u64().ok_or_else(|| {
        SystemsError::type_mismatch("A sweep count must be integral and non-negative")
    })?;
    steps
        .checked_add(1)
        .ok_or_else(|| SystemsError::type_mismatch("A sweep count overflowed"))
}

/// Sweeps a closed model's population from `from` to `to` in `step` increments.
///
/// The endpoint is inclusive only when `(to - from)` is exactly divisible by `step`; otherwise the
/// request is refused with `SweepEndpointNotOnGrid` rather than silently clamping a partial final
/// step onto the endpoint.
pub fn sweep_population(
    network: &QueueNetwork,
    from: u64,
    to: u64,
    step: u64,
    options: &SolverOptions,
    cancel: &dyn Cancel,
) -> Result<SweepResult, SystemsError> {
    if !matches!(network.workload, Workload::Closed(_)) {
        return Err(SystemsError::unsupported_model(
            "a population sweep requires a closed workload",
        ));
    }
    if step == 0 {
        return Err(SystemsError::type_mismatch(
            "A population sweep needs a positive step",
        ));
    }
    if from > to {
        return Err(SystemsError::type_mismatch("A sweep requires from <= to"));
    }
    let span = to - from;
    if span % step != 0 {
        return Err(SystemsError::sweep_endpoint_not_on_grid(
            &from.to_string(),
            &to.to_string(),
            &step.to_string(),
        ));
    }
    let count = (span / step)
        .checked_add(1)
        .ok_or_else(|| SystemsError::type_mismatch("A sweep count overflowed"))?;
    check_count(count, options)?;
    let mut points = Vec::with_capacity(count as usize);
    for index in 0..count {
        if cancel.is_cancelled() {
            return Err(SystemsError::cancelled());
        }
        // `index * step <= span`, so this addition cannot overflow.
        let population = from + index * step;
        let mut copy = network.clone();
        if let Workload::Closed(closed) = &mut copy.workload {
            closed.population = population;
        }
        let result = crate::analyze(&copy, options, cancel)?;
        points.push(SweepPoint {
            parameter: Q::from_u64(population),
            result,
        });
    }
    debug_assert_eq!(points.len() as u64, count);
    debug_assert_eq!(
        points.last().map(|point| point.parameter.to_u64()),
        Some(Some(to))
    );
    Ok(SweepResult {
        axis: SweepAxis::Population,
        points,
        warnings: Vec::new(),
    })
}

/// Sweeps an open model's arrival rate from `from` to `to` in `step` increments.
///
/// The same exact-grid contract as [`sweep_population`] applies: a partial final step is a
/// `SweepEndpointNotOnGrid` refusal, never a clamped extra point.
pub fn sweep_arrival(
    network: &QueueNetwork,
    from: &Q,
    to: &Q,
    step: &Q,
    options: &SolverOptions,
    cancel: &dyn Cancel,
) -> Result<SweepResult, SystemsError> {
    if !matches!(network.workload, Workload::Open(_)) {
        return Err(SystemsError::unsupported_model(
            "an arrival-rate sweep requires an open workload",
        ));
    }
    if !step.is_positive() {
        return Err(SystemsError::type_mismatch(
            "An arrival-rate sweep needs a positive step",
        ));
    }
    if from > to {
        return Err(SystemsError::type_mismatch("A sweep requires from <= to"));
    }
    let count = grid_point_count(from, to, step)?;
    check_count(count, options)?;
    let mut points = Vec::with_capacity(count as usize);
    let mut rate = from.clone();
    for _ in 0..count {
        if cancel.is_cancelled() {
            return Err(SystemsError::cancelled());
        }
        let mut copy = network.clone();
        if let Workload::Open(open) = &mut copy.workload {
            open.arrival_rate = rate.clone();
        }
        let result = crate::analyze(&copy, options, cancel)?;
        points.push(SweepPoint {
            parameter: rate.clone(),
            result,
        });
        rate = rate.add(step);
    }
    debug_assert_eq!(points.len() as u64, count);
    debug_assert_eq!(points.last().map(|point| &point.parameter), Some(to));
    Ok(SweepResult {
        axis: SweepAxis::ArrivalRate,
        points,
        warnings: Vec::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Q;
    use crate::model::{ClosedWorkload, NeverCancel, OpenWorkload, ServiceDiscipline, Station};

    fn station(id: &str) -> Station {
        Station::new(
            id,
            ServiceDiscipline::FcfsSingleServer,
            Q::from_i64(1).div(&Q::from_i64(10)).unwrap(),
            Q::one(),
        )
        .expect("valid")
    }

    fn closed() -> QueueNetwork {
        QueueNetwork::new(
            "closed",
            Workload::Closed(ClosedWorkload {
                population: 1,
                think_time: Q::zero(),
            }),
            vec![station("cpu")],
        )
        .expect("valid")
    }

    fn open() -> QueueNetwork {
        QueueNetwork::new(
            "open",
            Workload::Open(OpenWorkload {
                arrival_rate: Q::one(),
            }),
            vec![station("api")],
        )
        .expect("valid")
    }

    #[test]
    fn a_population_sweep_is_ordered_and_bounded() {
        let sweep = sweep_population(&closed(), 1, 4, 1, &SolverOptions::default(), &NeverCancel)
            .expect("swept");
        assert_eq!(sweep.axis, SweepAxis::Population);
        let parameters: Vec<u64> = sweep
            .points
            .iter()
            .map(|point| point.parameter.to_i64().unwrap() as u64)
            .collect();
        assert_eq!(parameters, vec![1, 2, 3, 4]);
        // With D=0.1, Z=0, one center: X(n) = 1/D for n>=1 (saturated at U=1).
        assert_eq!(sweep.points[0].result.aggregate.throughput, Q::from_i64(10));
        assert_eq!(sweep.max_utilization(), Q::one());
    }

    #[test]
    fn an_arrival_sweep_steps_exactly_and_refuses_instability() {
        let sweep = sweep_arrival(
            &open(),
            &Q::from_i64(1),
            &Q::from_i64(5),
            &Q::from_i64(2),
            &SolverOptions::default(),
            &NeverCancel,
        )
        .expect("swept");
        assert_eq!(sweep.points.len(), 3);
        assert_eq!(sweep.points[2].parameter, Q::from_i64(5));

        let error = sweep_arrival(
            &open(),
            &Q::from_i64(1),
            &Q::from_i64(20),
            &Q::from_i64(1),
            &SolverOptions::default(),
            &NeverCancel,
        )
        .expect_err("U=2 is unstable");
        assert_eq!(error.code(), "UnstableStation");
    }

    #[test]
    fn a_sweep_refuses_a_zero_step_and_a_workload_mismatch() {
        assert_eq!(
            sweep_population(&closed(), 1, 4, 0, &SolverOptions::default(), &NeverCancel)
                .expect_err("zero step")
                .code(),
            "TypeMismatch"
        );
        assert_eq!(
            sweep_population(&open(), 1, 4, 1, &SolverOptions::default(), &NeverCancel)
                .expect_err("open model")
                .code(),
            "UnsupportedModel"
        );
    }

    /// RNL-MIG-007: the inclusive endpoint is legal only when the span is exactly divisible by the
    /// step. A partial final step is a structured refusal, never a clamped extra point.
    #[test]
    fn a_partial_final_step_is_refused_on_both_axes() {
        let population =
            sweep_population(&closed(), 1, 6, 4, &SolverOptions::default(), &NeverCancel)
                .expect_err("6 is not on the 1, 5 grid");
        assert_eq!(population.code(), "SweepEndpointNotOnGrid");
        assert_eq!(population.category().as_str(), "DomainError");

        let arrival = sweep_arrival(
            &open(),
            &Q::from_i64(1),
            &Q::from_i64(6),
            &Q::from_i64(4),
            &SolverOptions::default(),
            &NeverCancel,
        )
        .expect_err("6 is not on the 1, 5 grid");
        assert_eq!(arrival.code(), "SweepEndpointNotOnGrid");
        assert_eq!(arrival.category().as_str(), "DomainError");
    }

    /// The precomputed count is exactly the number of executed, ordered points, and the last point
    /// is exactly the endpoint when the grid is valid.
    #[test]
    fn the_count_matches_the_executed_points_exactly() {
        let population =
            sweep_population(&closed(), 1, 7, 2, &SolverOptions::default(), &NeverCancel)
                .expect("1, 3, 5, 7 is an exact grid");
        assert_eq!(population.points.len(), 4);
        let parameters: Vec<u64> = population
            .points
            .iter()
            .map(|point| point.parameter.to_u64().expect("integral"))
            .collect();
        assert_eq!(parameters, vec![1, 3, 5, 7]);

        let arrival = sweep_arrival(
            &open(),
            &Q::from_i64(1),
            &Q::from_i64(7),
            &Q::from_i64(2),
            &SolverOptions::default(),
            &NeverCancel,
        )
        .expect("1, 3, 5, 7 is an exact grid");
        assert_eq!(arrival.points.len(), 4);
        assert_eq!(
            arrival
                .points
                .iter()
                .map(|point| point.parameter.to_u64().expect("integral"))
                .collect::<Vec<_>>(),
            vec![1, 3, 5, 7]
        );

        // A zero span is the single-point grid, not an empty sweep.
        let single = sweep_arrival(
            &open(),
            &Q::from_i64(3),
            &Q::from_i64(3),
            &Q::from_i64(2),
            &SolverOptions::default(),
            &NeverCancel,
        )
        .expect("a single point");
        assert_eq!(single.points.len(), 1);
        assert_eq!(single.points[0].parameter, Q::from_i64(3));
    }
}
