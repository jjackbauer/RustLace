//! Bottleneck analysis, capacity bounds and what-if comparison.
//!
//! The bound is stated with its own basis: `1 / max_i D_i` is the exact asymptotic throughput of a
//! closed single-server network as the population grows (with zero think time), while
//! `min_i c_i / D_i` is a capacity bound for multi-server stations. A delay center contributes no
//! finite bound. Neither is "achieved at finite N"; a result says so in a warning. A chosen
//! utilization or risk threshold is a different object and is never presented as a stability
//! threshold.

use crate::errors::SystemsError;
use crate::model::{
    Cancel, ClosedWorkload, OpenWorkload, QueueNetwork, ServiceDiscipline, SolverOptions, Workload,
};
use crate::numeric::Q;
use crate::results::AnalysisResult;

/// Why a throughput bound exists, and what it means.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BoundBasis {
    /// `1 / max D_i` over single-server stations: the exact asymptotic bound of a closed network
    /// with zero think time, and the stability threshold of an open one.
    AsymptoticSingleServer,
    /// `min c_i / D_i`: a capacity bound when at least one station has several servers.
    CapacityAdjusted,
    /// Every station is an infinite-server delay center: there is no finite throughput bound.
    Unbounded,
}

impl BoundBasis {
    /// The stable label.
    pub fn as_str(self) -> &'static str {
        match self {
            BoundBasis::AsymptoticSingleServer => "asymptotic-single-server",
            BoundBasis::CapacityAdjusted => "capacity-adjusted",
            BoundBasis::Unbounded => "unbounded",
        }
    }
}

/// One station in a bottleneck report.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BottleneckEntry {
    /// The station identifier.
    pub id: String,
    /// The service demand `V * S`.
    pub service_demand: Q,
    /// The finite server count, or `None` for a delay center.
    pub servers: Option<usize>,
    /// The station's own throughput ceiling (`c / D`), or `None` for a delay center with `D = 0`.
    pub ceiling: Option<Q>,
    /// True when this station sets the network bound.
    pub bottleneck: bool,
}

/// The bottleneck report for one model.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BottleneckAnalysis {
    /// Every station, in declaration order.
    pub stations: Vec<BottleneckEntry>,
    /// The largest service demand (zero when no station carries traffic).
    pub max_demand: Q,
    /// The network throughput bound, when one exists.
    pub throughput_bound: Option<Q>,
    /// What the bound means.
    pub basis: BoundBasis,
    /// Non-fatal notes: the bound is not achieved at finite N, or no station carries traffic.
    pub warnings: Vec<String>,
}

impl BottleneckAnalysis {
    /// The stations that set the bound.
    pub fn bottlenecks(&self) -> Vec<&BottleneckEntry> {
        self.stations
            .iter()
            .filter(|entry| entry.bottleneck)
            .collect()
    }
}

/// Computes the bottleneck and throughput bound for a network.
pub fn bottlenecks(network: &QueueNetwork) -> BottleneckAnalysis {
    let mut entries = Vec::with_capacity(network.stations.len());
    let mut max_demand = Q::zero();
    let mut bound: Option<Q> = None;
    let mut multi_server = false;

    for station in &network.stations {
        let demand = station.service_demand();
        if demand > max_demand {
            max_demand = demand.clone();
        }
        let ceiling = match station.servers() {
            Some(servers) if demand.is_positive() => Q::from_u64(servers as u64).div(&demand),
            _ => None,
        };
        if matches!(station.discipline, ServiceDiscipline::MultiServer { .. }) {
            multi_server = true;
        }
        if let Some(value) = &ceiling {
            bound = Some(match bound {
                Some(current) if current < *value => current,
                _ => value.clone(),
            });
        }
        entries.push(BottleneckEntry {
            id: station.id.to_string(),
            service_demand: demand,
            servers: station.servers(),
            ceiling,
            bottleneck: false,
        });
    }

    let basis = match (&bound, multi_server) {
        (None, _) => BoundBasis::Unbounded,
        (Some(_), true) => BoundBasis::CapacityAdjusted,
        (Some(_), false) => BoundBasis::AsymptoticSingleServer,
    };
    if let Some(limit) = &bound {
        for entry in &mut entries {
            entry.bottleneck = entry.ceiling.as_ref().is_some_and(|value| value == limit);
        }
    }
    let mut warnings = Vec::new();
    if bound.is_some() {
        warnings.push(
            "the bound is a ceiling, not a throughput achieved at finite population".to_string(),
        );
    }
    if entries.iter().all(|entry| entry.service_demand.is_zero()) {
        warnings.push("no station carries service demand".to_string());
    }
    BottleneckAnalysis {
        stations: entries,
        max_demand,
        throughput_bound: bound,
        basis,
        warnings,
    }
}

/// A what-if change applied to a copied model.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Scenario {
    /// Replace an open arrival rate.
    pub arrival_rate: Option<Q>,
    /// Replace a closed population.
    pub population: Option<u64>,
    /// Replace a closed think time.
    pub think_time: Option<Q>,
    /// Multiply each named station's service time (a demand change).
    pub demand_scale: Vec<(String, Q)>,
    /// Replace a named station's server count (multi-server only).
    pub server_counts: Vec<(String, usize)>,
}

/// The baseline and the scenario result, side by side.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Comparison {
    /// The unchanged model's result.
    pub baseline: AnalysisResult,
    /// The scenario model's result.
    pub scenario: AnalysisResult,
    /// The fields the scenario changed, as stable names.
    pub changed: Vec<&'static str>,
}

impl Scenario {
    /// Applies the scenario to a copy of the network, validating the result.
    pub fn apply(&self, network: &QueueNetwork) -> Result<QueueNetwork, SystemsError> {
        let mut copy = network.clone();
        match &mut copy.workload {
            Workload::Open(open) => {
                if let Some(rate) = &self.arrival_rate {
                    open.arrival_rate = rate.clone();
                }
            }
            Workload::Closed(closed) => {
                if let Some(population) = self.population {
                    closed.population = population;
                }
                if let Some(think) = &self.think_time {
                    closed.think_time = think.clone();
                }
            }
        }
        for (id, scale) in &self.demand_scale {
            let station = copy.station_mut(id).ok_or_else(|| {
                SystemsError::unsupported_model(&format!("scenario names unknown station '{id}'"))
            })?;
            station.service_time = station.service_time.mul(scale);
        }
        for (id, servers) in &self.server_counts {
            let station = copy.station_mut(id).ok_or_else(|| {
                SystemsError::unsupported_model(&format!("scenario names unknown station '{id}'"))
            })?;
            station.discipline = ServiceDiscipline::MultiServer { servers: *servers };
        }
        crate::validation::validate(&copy)?;
        Ok(copy)
    }
}

/// The stable names of the changes a scenario actually applies to this model.
pub fn changed_fields(network: &QueueNetwork, scenario: &Scenario) -> Vec<&'static str> {
    let mut changed = Vec::new();
    match &network.workload {
        Workload::Open(_) if scenario.arrival_rate.is_some() => changed.push("arrival_rate"),
        Workload::Closed(_) if scenario.population.is_some() => changed.push("population"),
        Workload::Closed(_) if scenario.think_time.is_some() => changed.push("think_time"),
        _ => {}
    }
    if !scenario.demand_scale.is_empty() {
        changed.push("service_demand");
    }
    if !scenario.server_counts.is_empty() {
        changed.push("server_count");
    }
    changed
}

/// Analyzes a scenario against its baseline with the same solver and options.
pub fn compare(
    network: &QueueNetwork,
    scenario: &Scenario,
    options: &SolverOptions,
    cancel: &dyn Cancel,
) -> Result<Comparison, SystemsError> {
    let baseline = crate::analyze(network, options, cancel)?;
    let changed = changed_fields(network, scenario);
    let model = scenario.apply(network)?;
    let result = crate::analyze(&model, options, cancel)?;
    Ok(Comparison {
        baseline,
        scenario: result,
        changed,
    })
}

/// Builds a closed workload change without touching the rest of the model (a helper for callers
/// that only need a population/think-time copy).
pub fn closed_with(
    network: &QueueNetwork,
    population: u64,
    think_time: Q,
) -> Result<QueueNetwork, SystemsError> {
    let mut copy = network.clone();
    copy.workload = Workload::Closed(ClosedWorkload {
        population,
        think_time,
    });
    crate::validation::validate(&copy)?;
    Ok(copy)
}

/// Builds an open workload change without touching the rest of the model.
pub fn open_with(network: &QueueNetwork, rate: Q) -> Result<QueueNetwork, SystemsError> {
    let mut copy = network.clone();
    copy.workload = Workload::Open(OpenWorkload { arrival_rate: rate });
    crate::validation::validate(&copy)?;
    Ok(copy)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{NeverCancel, Station};

    fn station(id: &str, discipline: ServiceDiscipline, service_time: Q, visits: Q) -> Station {
        Station::new(id, discipline, service_time, visits).expect("valid")
    }

    fn open(stations: Vec<Station>) -> QueueNetwork {
        QueueNetwork::new(
            "open",
            Workload::Open(OpenWorkload {
                arrival_rate: Q::from_i64(8),
            }),
            stations,
        )
        .expect("valid")
    }

    fn q(num: i64, den: u64) -> Q {
        Q::from_parts(
            lovelace_int::Int::from_i64(num),
            lovelace_nat::Nat::from_u64(den),
        )
        .unwrap()
    }

    #[test]
    fn the_largest_demand_is_the_bottleneck_and_sets_the_bound() {
        let model = open(vec![
            station(
                "api",
                ServiceDiscipline::FcfsSingleServer,
                q(1, 20),
                Q::one(),
            ),
            station(
                "db",
                ServiceDiscipline::FcfsSingleServer,
                q(1, 50),
                Q::from_u64(3),
            ),
            station("cache", ServiceDiscipline::Delay, q(1, 100), Q::one()),
        ]);
        let report = bottlenecks(&model);
        assert_eq!(report.basis, BoundBasis::AsymptoticSingleServer);
        assert_eq!(report.max_demand, q(3, 50));
        assert_eq!(
            report.throughput_bound,
            Some(Q::from_i64(50).div(&Q::from_i64(3)).unwrap())
        );
        let names: Vec<&str> = report
            .bottlenecks()
            .iter()
            .map(|entry| entry.id.as_str())
            .collect();
        assert_eq!(names, vec!["db"]);
        assert!(report.warnings[0].contains("ceiling"));
    }

    #[test]
    fn multi_server_capacity_is_adjusted_and_delay_is_unbounded() {
        let model = open(vec![
            station(
                "pool",
                ServiceDiscipline::MultiServer { servers: 4 },
                q(1, 10),
                Q::one(),
            ),
            station(
                "slow",
                ServiceDiscipline::FcfsSingleServer,
                q(1, 20),
                Q::one(),
            ),
        ]);
        let report = bottlenecks(&model);
        assert_eq!(report.basis, BoundBasis::CapacityAdjusted);
        // pool: 4 / 0.1 = 40; slow: 1 / 0.05 = 20 -> slow is the bound.
        assert_eq!(report.throughput_bound, Some(Q::from_i64(20)));
        assert_eq!(
            report.bottlenecks()[0].id,
            "slow",
            "the smallest ceiling sets the bound"
        );

        let delays = open(vec![station(
            "io",
            ServiceDiscipline::Delay,
            Q::one(),
            Q::one(),
        )]);
        let report = bottlenecks(&delays);
        assert_eq!(report.basis, BoundBasis::Unbounded);
        assert_eq!(report.throughput_bound, None);
    }

    #[test]
    fn a_scenario_copies_the_model_and_changes_only_the_named_fields() {
        let model = open(vec![
            station(
                "api",
                ServiceDiscipline::FcfsSingleServer,
                q(1, 20),
                Q::one(),
            ),
            station(
                "db",
                ServiceDiscipline::FcfsSingleServer,
                q(1, 50),
                Q::from_u64(2),
            ),
        ]);
        let scenario = Scenario {
            arrival_rate: Some(Q::from_i64(4)),
            ..Scenario::default()
        };
        let comparison =
            compare(&model, &scenario, &SolverOptions::default(), &NeverCancel).expect("compared");
        assert_eq!(comparison.changed, vec!["arrival_rate"]);
        assert_eq!(comparison.baseline.aggregate.throughput, Q::from_i64(8));
        assert_eq!(comparison.scenario.aggregate.throughput, Q::from_i64(4));
        assert_eq!(
            model.workload,
            Workload::Open(OpenWorkload {
                arrival_rate: Q::from_i64(8)
            })
        );
    }

    #[test]
    fn an_unknown_scenario_station_is_refused() {
        let model = open(vec![station(
            "api",
            ServiceDiscipline::FcfsSingleServer,
            Q::one(),
            Q::one(),
        )]);
        let scenario = Scenario {
            demand_scale: vec![("nope".to_string(), Q::from_i64(2))],
            ..Scenario::default()
        };
        let error = scenario.apply(&model).expect_err("unknown station");
        assert_eq!(error.code(), "UnsupportedModel");
        assert!(error.message().contains("nope"));
    }
}
