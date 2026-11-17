//! Stable, categorized refusals.
//!
//! The category vocabulary matches the interpreter's frozen seven (`ParseError` is the only one the
//! systems library cannot produce): the adapter maps a [`SystemsError`] to the same wire shape a
//! language builtin publishes, so a refusal is structured on both surfaces. A code is stable text;
//! a caller matches on the code (and optionally the category), never on prose.

use std::fmt;

/// The stable category of a systems refusal.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ErrorCategory {
    /// A value is outside the operation's mathematical domain (a rate <= 0, a population overflow).
    DomainError,
    /// The requested model or discipline is not supported by the chosen solver.
    UnsupportedOperation,
    /// A declared iteration, state-space or rational-size budget was exhausted.
    BudgetExceeded,
    /// A system of equations has no solution (an inconsistent routing matrix).
    NoSolution,
    /// A value has the wrong kind or shape for the declared parameter.
    TypeMismatch,
    /// The implementation's own invariant failed; never reachable by a valid model.
    InternalInvariantFailure,
}

impl ErrorCategory {
    /// The category's stable wire spelling.
    pub fn as_str(self) -> &'static str {
        match self {
            ErrorCategory::DomainError => "DomainError",
            ErrorCategory::UnsupportedOperation => "UnsupportedOperation",
            ErrorCategory::BudgetExceeded => "BudgetExceeded",
            ErrorCategory::NoSolution => "NoSolution",
            ErrorCategory::TypeMismatch => "TypeMismatch",
            ErrorCategory::InternalInvariantFailure => "InternalInvariantFailure",
        }
    }
}

/// One categorized systems refusal.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SystemsError {
    code: &'static str,
    category: ErrorCategory,
    message: String,
}

impl SystemsError {
    /// Builds a refusal with a stable code and a complete-sentence message.
    pub fn new(code: &'static str, category: ErrorCategory, message: impl Into<String>) -> Self {
        SystemsError {
            code,
            category,
            message: message.into(),
        }
    }

    /// The stable code (`InvalidArrivalRate`, `UnstableStation`, ...).
    pub fn code(&self) -> &'static str {
        self.code
    }

    /// The stable category.
    pub fn category(&self) -> ErrorCategory {
        self.category
    }

    /// The complete-sentence message.
    pub fn message(&self) -> &str {
        &self.message
    }

    /// An identifier that is empty, too long, or uses a character outside `[A-Za-z0-9_.-]`.
    pub fn invalid_identifier(id: &str) -> Self {
        Self::new(
            "InvalidIdentifier",
            ErrorCategory::DomainError,
            format!(
                "Station identifier '{id}' must be 1 to 64 characters of letters, digits, '_', '.' or '-'."
            ),
        )
    }

    /// Two stations declare the same identifier.
    pub fn duplicate_identifier(id: &str) -> Self {
        Self::new(
            "DuplicateStationId",
            ErrorCategory::DomainError,
            format!("Station identifier '{id}' is declared more than once."),
        )
    }

    /// A network with no stations.
    pub fn empty_network() -> Self {
        Self::new(
            "EmptyNetwork",
            ErrorCategory::DomainError,
            "A queue network needs at least one station.".to_string(),
        )
    }

    /// A population that does not fit the supported closed-model range.
    pub fn invalid_population(population: &str) -> Self {
        Self::new(
            "InvalidPopulation",
            ErrorCategory::DomainError,
            format!(
                "The closed population must be a non-negative integer, but got '{population}'."
            ),
        )
    }

    /// An arrival rate that is not a finite positive number for an open model.
    pub fn invalid_arrival_rate(rate: &str) -> Self {
        Self::new(
            "InvalidArrivalRate",
            ErrorCategory::DomainError,
            format!("The arrival rate must be positive, but got {rate}."),
        )
    }

    /// A service time that is not positive on a station carrying traffic.
    pub fn invalid_service_time(id: &str, service_time: &str) -> Self {
        Self::new(
            "InvalidServiceTime",
            ErrorCategory::DomainError,
            format!(
                "Station '{id}' must have a positive service time per visit when it carries traffic, but got {service_time}."
            ),
        )
    }

    /// A service rate that is not finite positive.
    pub fn invalid_service_rate(rate: &str) -> Self {
        Self::new(
            "InvalidServiceRate",
            ErrorCategory::DomainError,
            format!("The service rate must be positive, but got {rate}."),
        )
    }

    /// A negative visit count.
    pub fn invalid_visits(id: &str, visits: &str) -> Self {
        Self::new(
            "InvalidVisits",
            ErrorCategory::DomainError,
            format!("Station '{id}' must have a non-negative visit ratio, but got {visits}."),
        )
    }

    /// A discipline the chosen solver does not implement.
    pub fn unsupported_discipline(id: &str, discipline: &str) -> Self {
        Self::new(
            "UnsupportedDiscipline",
            ErrorCategory::UnsupportedOperation,
            format!(
                "Station '{id}' uses discipline {discipline}, which this solver does not support."
            ),
        )
    }

    /// A mixed open/closed network, which is a different solver.
    pub fn mixed_workload() -> Self {
        Self::new(
            "MixedWorkloadNotSupported",
            ErrorCategory::UnsupportedOperation,
            "A model has exactly one open or one closed workload; mixed networks are not supported."
                .to_string(),
        )
    }

    /// A finite-capacity station whose offered load is at or above capacity.
    pub fn unstable_station(id: &str, utilization: &str) -> Self {
        Self::new(
            "UnstableStation",
            ErrorCategory::DomainError,
            format!(
                "Station '{id}' is unstable: its offered utilization is {utilization}, which is not below 1."
            ),
        )
    }

    /// A model whose cycle time is 0/0 (no service demand and no think time).
    pub fn degenerate_model(detail: &str) -> Self {
        Self::new(
            "DegenerateModel",
            ErrorCategory::DomainError,
            format!("The model has no defined stationary throughput: {detail}."),
        )
    }

    /// A routing row that is not a probability distribution.
    pub fn routing_probability_row(row: usize, total: &str) -> Self {
        Self::new(
            "RoutingProbabilityRow",
            ErrorCategory::DomainError,
            format!(
                "Routing row {row} must be a probability distribution summing to 1, but sums to {total}."
            ),
        )
    }

    /// A routing matrix whose traffic equations have no finite solution.
    pub fn routing_not_reachable(detail: &str) -> Self {
        Self::new(
            "RoutingNotReachable",
            ErrorCategory::NoSolution,
            format!("The routing traffic equations are not solvable: {detail}."),
        )
    }

    /// The rational-size budget was exhausted; no truncated value is returned.
    pub fn rational_budget(limit: usize, observed: usize) -> Self {
        Self::new(
            "RationalBudgetExceeded",
            ErrorCategory::BudgetExceeded,
            format!(
                "The exact intermediate needs {observed} decimal digits, over the {limit}-digit budget."
            ),
        )
    }

    /// The iteration budget was exhausted.
    pub fn iteration_budget(limit: u64, requested: u64) -> Self {
        Self::new(
            "IterationBudgetExceeded",
            ErrorCategory::BudgetExceeded,
            format!("The model needs {requested} iterations, over the {limit}-iteration budget."),
        )
    }

    /// The state-space budget was exhausted.
    pub fn state_space_budget(limit: u64, requested: u64) -> Self {
        Self::new(
            "StateSpaceBudgetExceeded",
            ErrorCategory::BudgetExceeded,
            format!(
                "The requested result needs {requested} population entries, over the {limit}-entry budget."
            ),
        )
    }

    /// A sweep endpoint is not reachable from the start in exact steps.
    ///
    /// The grid contract accepts an inclusive endpoint only when the range is exactly divisible
    /// by the step; a partial final step is refused rather than clamped to the endpoint.
    pub fn sweep_endpoint_not_on_grid(from: &str, to: &str, step: &str) -> Self {
        Self::new(
            "SweepEndpointNotOnGrid",
            ErrorCategory::DomainError,
            format!(
                "The sweep endpoint {to} is not reachable from {from} in exact steps of {step}; adjust the endpoint or the step."
            ),
        )
    }

    /// The caller cancelled the computation; no partial result is returned.
    pub fn cancelled() -> Self {
        Self::new(
            "Cancelled",
            ErrorCategory::BudgetExceeded,
            "The computation was cancelled by the caller.".to_string(),
        )
    }

    /// A model the chosen solver does not implement, naming the failed assumption.
    pub fn unsupported_model(assumption: &str) -> Self {
        Self::new(
            "UnsupportedModel",
            ErrorCategory::UnsupportedOperation,
            format!("The model is not supported: {assumption}."),
        )
    }

    /// The solver's own conservation check failed.
    pub fn invariant(detail: &str) -> Self {
        Self::new(
            "InternalInvariantFailure",
            ErrorCategory::InternalInvariantFailure,
            format!("The solver's conservation check failed: {detail}."),
        )
    }

    /// An internal operation that must not fail (a divisor the model guarantees non-zero).
    pub fn internal(detail: &str) -> Self {
        Self::new(
            "InternalInvariantFailure",
            ErrorCategory::InternalInvariantFailure,
            format!("The solver hit an internal invariant: {detail}."),
        )
    }

    /// A value of the wrong kind or shape for a declared parameter.
    pub fn type_mismatch(detail: &str) -> Self {
        Self::new(
            "TypeMismatch",
            ErrorCategory::TypeMismatch,
            format!("{detail}."),
        )
    }
}

impl fmt::Display for SystemsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for SystemsError {}
