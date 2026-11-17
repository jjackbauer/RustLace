//! Typed systems models.
//!
//! A model is a [`QueueNetwork`]: a name, exactly one [`Workload`] and an ordered list of
//! [`Station`]s. Every station has a stable [`StationId`], a [`ServiceDiscipline`], a service time
//! per visit and a visit ratio. A [`SolverOptions`] carries the rendering precision and the hard
//! budgets; nothing in this module rounds a value.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::Q;
use crate::errors::SystemsError;

/// A validated station or class identifier.
///
/// The grammar is 1 to 64 bytes of ASCII letters, digits, `_`, `.` or `-`; an identifier is unique
/// within a network. It is a newtype so an identifier cannot be confused with a free-text label.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StationId(String);

impl StationId {
    /// Validates and wraps an identifier.
    pub fn new(value: impl Into<String>) -> Result<StationId, SystemsError> {
        let value = value.into();
        let valid_length = !value.is_empty() && value.len() <= 64;
        let valid_chars = value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'.' | b'-'));
        if valid_length && valid_chars {
            Ok(StationId(value))
        } else {
            Err(SystemsError::invalid_identifier(&value))
        }
    }

    /// The identifier text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for StationId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// The scheduling discipline a station models.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ServiceDiscipline {
    /// One server, first-come first-served, load-independent service.
    FcfsSingleServer,
    /// Infinitely many servers (a delay center): no customer waits.
    Delay,
    /// `c` identical servers (`c >= 2`), M/M/c.
    MultiServer {
        /// The server count, at least 2.
        servers: usize,
    },
}

impl ServiceDiscipline {
    /// The finite server count, or `None` for an infinite-server delay center.
    pub fn servers(&self) -> Option<usize> {
        match self {
            ServiceDiscipline::FcfsSingleServer => Some(1),
            ServiceDiscipline::Delay => None,
            ServiceDiscipline::MultiServer { servers } => Some(*servers),
        }
    }

    /// The stable discipline label.
    pub fn as_str(&self) -> &'static str {
        match self {
            ServiceDiscipline::FcfsSingleServer => "fcfs",
            ServiceDiscipline::Delay => "delay",
            ServiceDiscipline::MultiServer { .. } => "mmc",
        }
    }
}

/// One service center.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Station {
    /// The stable identifier.
    pub id: StationId,
    /// The scheduling discipline.
    pub discipline: ServiceDiscipline,
    /// Service time per visit, in canonical seconds.
    pub service_time: Q,
    /// Visits per transaction, a non-negative (usually positive) ratio.
    pub visits: Q,
}

impl Station {
    /// A station with an explicit discipline, service time and visit ratio.
    pub fn new(
        id: impl Into<String>,
        discipline: ServiceDiscipline,
        service_time: Q,
        visits: Q,
    ) -> Result<Station, SystemsError> {
        Ok(Station {
            id: StationId::new(id)?,
            discipline,
            service_time,
            visits,
        })
    }

    /// The service demand `D = V * S` in canonical seconds per transaction.
    pub fn service_demand(&self) -> Q {
        self.visits.mul(&self.service_time)
    }

    /// The finite server count, or `None` for a delay center.
    pub fn servers(&self) -> Option<usize> {
        self.discipline.servers()
    }
}

/// An open workload: jobs arrive from outside at a rate and leave.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OpenWorkload {
    /// The external arrival rate in jobs per second.
    pub arrival_rate: Q,
}

/// A closed workload: a fixed population cycles between thinking and the network.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClosedWorkload {
    /// The number of customers, at least 0.
    pub population: u64,
    /// Mean think time outside the network, in canonical seconds.
    pub think_time: Q,
}

/// The one workload a model carries.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Workload {
    /// An open arrival stream.
    Open(OpenWorkload),
    /// A closed customer population.
    Closed(ClosedWorkload),
}

impl Workload {
    /// The stable model-class label (`open` or `closed`).
    pub fn class(&self) -> &'static str {
        match self {
            Workload::Open(_) => "open",
            Workload::Closed(_) => "closed",
        }
    }
}

/// A validated queueing network.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QueueNetwork {
    /// A human-readable model name (not an identifier; may be empty).
    pub name: String,
    /// The one workload.
    pub workload: Workload,
    /// The stations, in declaration order. Result order follows this order exactly.
    pub stations: Vec<Station>,
}

impl QueueNetwork {
    /// Builds and validates a network.
    pub fn new(
        name: impl Into<String>,
        workload: Workload,
        stations: Vec<Station>,
    ) -> Result<QueueNetwork, SystemsError> {
        let network = QueueNetwork {
            name: name.into(),
            workload,
            stations,
        };
        crate::validation::validate(&network)?;
        Ok(network)
    }

    /// The station with `id`, when present.
    pub fn station(&self, id: &str) -> Option<&Station> {
        self.stations
            .iter()
            .find(|station| station.id.as_str() == id)
    }

    /// The station with `id`, mutably, when present (scenario copies only).
    pub fn station_mut(&mut self, id: &str) -> Option<&mut Station> {
        self.stations
            .iter_mut()
            .find(|station| station.id.as_str() == id)
    }

    /// The total service demand `sum_i D_i` in seconds per transaction.
    pub fn total_demand(&self) -> Q {
        self.stations
            .iter()
            .fold(Q::zero(), |acc, station| acc.add(&station.service_demand()))
    }
}

/// Hard limits and the rendering precision for one analysis.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SolverOptions {
    /// Decimal places used when a `Q` is rendered as a `Real`; the math itself is exact.
    pub precision: usize,
    /// Maximum solver iterations (a closed-model population, or a sweep point count).
    pub max_iterations: u64,
    /// Maximum retained population entries when intermediate populations are collected.
    pub max_state_space: u64,
    /// Maximum decimal digits of any exact intermediate (numerator + denominator).
    pub max_rational_digits: usize,
    /// Whether to retain every intermediate population of an MVA run.
    pub collect_populations: bool,
}

impl Default for SolverOptions {
    fn default() -> Self {
        SolverOptions {
            precision: 30,
            max_iterations: 100_000,
            max_state_space: 1_000_000,
            max_rational_digits: 4_096,
            collect_populations: false,
        }
    }
}

impl SolverOptions {
    /// Records a rational intermediate and refuses it when it is over the digit budget.
    pub fn check_rational(&self, value: &Q) -> Result<(), SystemsError> {
        let digits = value.decimal_digits();
        if digits > self.max_rational_digits {
            return Err(SystemsError::rational_budget(
                self.max_rational_digits,
                digits,
            ));
        }
        Ok(())
    }

    /// Refuses a requested iteration count over the budget.
    pub fn check_iterations(&self, requested: u64) -> Result<(), SystemsError> {
        if requested > self.max_iterations {
            return Err(SystemsError::iteration_budget(
                self.max_iterations,
                requested,
            ));
        }
        Ok(())
    }
}

/// The cooperative cancellation check a long solver polls.
///
/// The interpreter adapter implements this over its own deadline/flag scope, so the systems crate
/// does not depend on the engine. A cancelled run returns [`SystemsError::cancelled`]; a partial
/// result is never published as successful.
pub trait Cancel {
    /// True when the caller has cancelled the computation.
    fn is_cancelled(&self) -> bool;
}

/// A cancellation check that never fires.
#[derive(Clone, Copy, Debug, Default)]
pub struct NeverCancel;

impl Cancel for NeverCancel {
    fn is_cancelled(&self) -> bool {
        false
    }
}

/// A cancellation check backed by a shared atomic flag (the desktop's Cancel button, a runner
/// deadline).
#[derive(Clone, Debug)]
pub struct FlagCancel(pub Arc<AtomicBool>);

impl Cancel for FlagCancel {
    fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lovelace_int::Int;
    use lovelace_nat::Nat;

    fn station(id: &str, discipline: ServiceDiscipline, time: Q, visits: Q) -> Station {
        Station::new(id, discipline, time, visits).expect("valid station")
    }

    #[test]
    fn identifiers_follow_the_documented_grammar() {
        assert_eq!(
            StationId::new("api_1.db").expect("valid").as_str(),
            "api_1.db"
        );
        for bad in ["", "has space", "slash/inside", "tab\there"] {
            let error = StationId::new(bad).expect_err("refused");
            assert_eq!(error.code(), "InvalidIdentifier");
            assert_eq!(error.category().as_str(), "DomainError");
        }
        assert!(StationId::new("x".repeat(65)).is_err());
        assert!(StationId::new("x".repeat(64)).is_ok());
    }

    #[test]
    fn service_demand_is_visits_times_service_time() {
        let s = station(
            "api",
            ServiceDiscipline::FcfsSingleServer,
            Q::from_parts(Int::from_i64(1), Nat::from_u64(20)).expect("1/20"),
            Q::from_u64(3),
        );
        assert_eq!(
            s.service_demand(),
            Q::from_parts(Int::from_i64(3), Nat::from_u64(20)).expect("3/20")
        );
        assert_eq!(s.servers(), Some(1));
        assert_eq!(ServiceDiscipline::Delay.servers(), None);
        assert_eq!(
            ServiceDiscipline::MultiServer { servers: 4 }.servers(),
            Some(4)
        );
    }

    #[test]
    fn closed_model_accepts_zero_population_and_validates_the_rest() {
        let network = QueueNetwork::new(
            "empty",
            Workload::Closed(ClosedWorkload {
                population: 0,
                think_time: Q::zero(),
            }),
            vec![station(
                "one",
                ServiceDiscipline::FcfsSingleServer,
                Q::one(),
                Q::one(),
            )],
        );
        assert!(network.is_ok());
        // A zero arrival rate is a defined, empty arrival stream, not a domain error.
        let zero = QueueNetwork::new(
            "zero",
            Workload::Open(OpenWorkload {
                arrival_rate: Q::zero(),
            }),
            vec![station(
                "one",
                ServiceDiscipline::FcfsSingleServer,
                Q::one(),
                Q::one(),
            )],
        )
        .expect("zero arrival rate is defined");
        assert_eq!(zero.stations.len(), 1);
        let error = QueueNetwork::new(
            "bad",
            Workload::Open(OpenWorkload {
                arrival_rate: Q::from_i64(-1),
            }),
            vec![station(
                "one",
                ServiceDiscipline::FcfsSingleServer,
                Q::one(),
                Q::one(),
            )],
        )
        .expect_err("a negative arrival rate is refused");
        assert_eq!(error.code(), "InvalidArrivalRate");
    }

    #[test]
    fn options_default_to_the_documented_values() {
        let options = SolverOptions::default();
        assert_eq!(options.precision, 30);
        assert_eq!(options.max_iterations, 100_000);
        assert_eq!(options.max_state_space, 1_000_000);
        assert_eq!(options.max_rational_digits, 4_096);
        assert!(!options.collect_populations);
        assert!(options.check_iterations(100_000).is_ok());
        assert_eq!(
            options.check_iterations(100_001).expect_err("over").code(),
            "IterationBudgetExceeded"
        );
    }
}
