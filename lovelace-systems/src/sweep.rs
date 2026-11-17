//! Population and arrival-rate sweeps.
//!
//! A sweep is a batch of ordinary analyses over an immutable baseline model: each point copies the
//! model, changes exactly one parameter and calls the same solver. The point count is bounded by
//! `SolverOptions::max_iterations` and by [`MAX_SWEEP_POINTS`] before anything is computed, so a
//! caller cannot ask for an unbounded allocation. Results keep their declaration order and each
//! point carries its own provenance.

use crate::errors::SystemsError;
use crate::model::{Cancel, QueueNetwork, SolverOptions, Workload};
use crate::numeric::Q;
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

/// Sweeps a closed model's population from `from` to `to` in `step` increments.
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
    let count = (to - from) / step + 1;
    check_count(count, options)?;
    let mut points = Vec::with_capacity(count as usize);
    let mut population = from;
    loop {
        if cancel.is_cancelled() {
            return Err(SystemsError::cancelled());
        }
        let mut copy = network.clone();
        if let Workload::Closed(closed) = &mut copy.workload {
            closed.population = population;
        }
        let result = crate::analyze(&copy, options, cancel)?;
        points.push(SweepPoint {
            parameter: Q::from_u64(population),
            result,
        });
        if population == to {
            break;
        }
        population = population.saturating_add(step).min(to);
    }
    Ok(SweepResult {
        axis: SweepAxis::Population,
        points,
        warnings: Vec::new(),
    })
}

/// Sweeps an open model's arrival rate from `from` to `to` in `step` increments.
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
    let span = to.sub(from);
    let steps = span.div(step).ok_or_else(|| {
        SystemsError::type_mismatch("An arrival-rate sweep needs a positive step")
    })?;
    let count = steps
        .to_i64()
        .and_then(|value| u64::try_from(value).ok())
        .map(|value| value + 1)
        .ok_or_else(|| {
            SystemsError::type_mismatch("A sweep count must be integral and non-negative")
        })?;
    check_count(count, options)?;
    let mut points = Vec::with_capacity(count as usize);
    let mut rate = from.clone();
    loop {
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
        if rate == *to {
            break;
        }
        let next = rate.add(step);
        rate = if next > *to { to.clone() } else { next };
    }
    Ok(SweepResult {
        axis: SweepAxis::ArrivalRate,
        points,
        warnings: Vec::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{ClosedWorkload, NeverCancel, OpenWorkload, ServiceDiscipline, Station};
    use crate::numeric::Q;

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
}
