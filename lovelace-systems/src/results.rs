//! Analysis results and solver provenance.
//!
//! Every number is an exact [`Q`]. The provenance says which model class and solver produced it,
//! which assumptions the solver applied, whether the arithmetic was exact, whether an algorithmic
//! approximation was used, the rendering precision, and any warning or resource limit. A result is
//! ordered by station declaration, never by a hash map, so two identical runs serialize identically.

use crate::Q;

/// Whether the reported numbers are exact rationals or a precision-limited rendering.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Exactness {
    /// Every reported number is an exact rational; the precision only affects decimal rendering.
    Exact,
    /// A number came from a bounded-digit computation; the precision names the digit budget.
    PrecisionLimited {
        /// The digit budget the computation used.
        precision: usize,
    },
}

impl Exactness {
    /// True for [`Exactness::Exact`].
    pub fn is_exact(self) -> bool {
        matches!(self, Exactness::Exact)
    }

    /// The stable label.
    pub fn as_str(self) -> &'static str {
        match self {
            Exactness::Exact => "exact",
            Exactness::PrecisionLimited { .. } => "precision-limited",
        }
    }
}

/// The solution status of a completed analysis.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SolverStatus {
    /// The stationary (or finite-horizon) solution was computed.
    Solved,
    /// The model is mathematically defined but empty (for example a closed population of 0).
    Degenerate,
}

impl SolverStatus {
    /// The stable label.
    pub fn as_str(self) -> &'static str {
        match self {
            SolverStatus::Solved => "solved",
            SolverStatus::Degenerate => "degenerate",
        }
    }
}

/// What produced a result, and on what terms.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Provenance {
    /// The model class (`open-jackson`, `closed-mva`, `station-mm1`, ...).
    pub model_class: &'static str,
    /// The solver name.
    pub solver_name: &'static str,
    /// The solver/algorithms version; results with different versions are not comparable.
    pub solver_version: &'static str,
    /// The assumptions the solver applied, as stable sentences.
    pub assumptions: Vec<&'static str>,
    /// Whether the arithmetic is exact or precision-limited.
    pub exactness: Exactness,
    /// The algorithmic approximation, when one was used (`None` means none).
    pub approximation: Option<&'static str>,
    /// The decimal rendering precision used for `Real` conversions.
    pub precision: usize,
    /// Non-fatal notes (a degenerate model, a capacity bound that is not achieved).
    pub warnings: Vec<String>,
    /// The solution status.
    pub status: SolverStatus,
    /// The resource limit that shaped the result, when one did.
    pub resource_limit: Option<String>,
}

impl Provenance {
    /// Builds provenance for an exact, non-approximate solve.
    pub fn exact(
        model_class: &'static str,
        solver_name: &'static str,
        assumptions: Vec<&'static str>,
        precision: usize,
    ) -> Provenance {
        Provenance {
            model_class,
            solver_name,
            solver_version: crate::SOLVER_VERSION,
            assumptions,
            exactness: Exactness::Exact,
            approximation: None,
            precision,
            warnings: Vec::new(),
            status: SolverStatus::Solved,
            resource_limit: None,
        }
    }
}

/// One station's solved metrics.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StationResult {
    /// The stable station identifier.
    pub id: String,
    /// The discipline label (`fcfs`, `delay`, `mmc`).
    pub discipline: &'static str,
    /// The finite server count, or `None` for a delay center.
    pub servers: Option<usize>,
    /// Visits per transaction.
    pub visits: Q,
    /// Service time per visit, in seconds.
    pub service_time: Q,
    /// Service demand `V * S`, in seconds per transaction.
    pub service_demand: Q,
    /// Station throughput, in jobs per second.
    pub throughput: Q,
    /// Utilization (dimensionless; a delay center reports its mean busy-server count here and in
    /// `busy_servers`).
    pub utilization: Q,
    /// Mean busy servers, `a = c * rho`; it can exceed 1 at a multi-server station or a delay
    /// center, where `utilization` is `a / c` or `a`.
    pub busy_servers: Q,
    /// Mean residence time in the station (service + wait), in seconds per visit.
    pub response_time: Q,
    /// Mean waiting time before service, in seconds per visit.
    pub waiting_time: Q,
    /// Mean number in the station (in service + waiting).
    pub occupancy: Q,
    /// Mean number waiting (excluding those in service).
    pub queue_length: Q,
    /// M/M/c probability of waiting (Erlang C); `None` when not defined for the discipline.
    pub probability_waiting: Option<Q>,
}

/// One exact conservation or operational-law check.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LawCheck {
    /// The law's stable name (`little`, `forced-flow`, `interactive-response`, `utilization`).
    pub name: &'static str,
    /// The left-hand side as computed.
    pub left: Q,
    /// The right-hand side as computed.
    pub right: Q,
    /// True when the two sides are exactly equal.
    pub holds: bool,
}

impl LawCheck {
    /// Records an exact check; `holds` is the exact comparison.
    pub fn exact(name: &'static str, left: Q, right: Q) -> LawCheck {
        let holds = left == right;
        LawCheck {
            name,
            left,
            right,
            holds,
        }
    }
}

/// Network-wide metrics.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AggregateResult {
    /// System throughput, in jobs per second.
    pub throughput: Q,
    /// Mean response time inside the network, in seconds per transaction (excludes think time).
    pub response_time: Q,
    /// Mean number in the network.
    pub occupancy: Q,
    /// Mean number waiting in queues across the network.
    pub queue_length: Q,
    /// The closed population, when the model is closed.
    pub population: Option<u64>,
    /// The closed think time, when the model is closed.
    pub think_time: Option<Q>,
    /// The exact law checks the solver performed.
    pub law_checks: Vec<LawCheck>,
}

/// One intermediate population of a closed-model solve (opt-in).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PopulationPoint {
    /// The population `n`.
    pub population: u64,
    /// Throughput `X(n)`.
    pub throughput: Q,
    /// Mean response time `R(n)` inside the network.
    pub response_time: Q,
    /// Mean number in the network `N(n)`.
    pub occupancy: Q,
    /// Per-station mean occupancy, in station declaration order.
    pub station_occupancy: Vec<Q>,
}

/// The complete result of one analysis.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AnalysisResult {
    /// What produced the result and on what terms.
    pub provenance: Provenance,
    /// Per-station metrics, in station declaration order.
    pub stations: Vec<StationResult>,
    /// Network-wide metrics.
    pub aggregate: AggregateResult,
    /// Every intermediate population, when `SolverOptions::collect_populations` was set.
    pub populations: Option<Vec<PopulationPoint>>,
}

impl AnalysisResult {
    /// The result for the station with `id`.
    pub fn station(&self, id: &str) -> Option<&StationResult> {
        self.stations.iter().find(|station| station.id == id)
    }

    /// True when every reported law check holds.
    pub fn laws_hold(&self) -> bool {
        self.aggregate.law_checks.iter().all(|check| check.holds)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exactness_and_status_labels_are_stable() {
        assert_eq!(Exactness::Exact.as_str(), "exact");
        assert!(Exactness::Exact.is_exact());
        assert_eq!(
            Exactness::PrecisionLimited { precision: 40 }.as_str(),
            "precision-limited"
        );
        assert_eq!(SolverStatus::Solved.as_str(), "solved");
        assert_eq!(SolverStatus::Degenerate.as_str(), "degenerate");
    }

    #[test]
    fn a_law_check_compares_exactly() {
        let ok = LawCheck::exact("little", Q::from_i64(4), Q::from_i64(4));
        assert!(ok.holds);
        let bad = LawCheck::exact("little", Q::from_i64(4), Q::from_i64(5));
        assert!(!bad.holds);
        let result = AnalysisResult {
            provenance: Provenance::exact("test", "test", Vec::new(), 30),
            stations: Vec::new(),
            aggregate: AggregateResult {
                throughput: Q::zero(),
                response_time: Q::zero(),
                occupancy: Q::zero(),
                queue_length: Q::zero(),
                population: None,
                think_time: None,
                law_checks: vec![ok, bad],
            },
            populations: None,
        };
        assert!(!result.laws_hold());
    }
}
