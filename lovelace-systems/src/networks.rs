//! Open Jackson network analysis.
//!
//! The supported P0 open model is a Jackson network with one customer class, explicit per-station
//! visit ratios and Poisson external arrivals. For each station the arrival rate is the forced-flow
//! value `X_i = V_i * lambda`; the station is then an independent M/M/c queue (Jackson's theorem),
//! so its metrics come from the same exact station kernel the single-station API uses.
//!
//! The network response time is the expected residence time of one transaction,
//! `R = sum_i V_i * R_i`; the network population is `L = lambda * R` (Little's law). Both are
//! published with their exact checks. Every finite-capacity station must be stable, or the analysis
//! is refused with the offending station named.

use crate::Q;
use crate::errors::SystemsError;
use crate::model::{Cancel, QueueNetwork, SolverOptions, Workload};
use crate::operational_laws::{check_forced_flow, check_little};
use crate::queueing::solve_station;
use crate::results::{
    AggregateResult, AnalysisResult, LawCheck, Provenance, SolverStatus, StationResult,
};

/// The assumptions an open Jackson result rests on.
pub const JACKSON_ASSUMPTIONS: [&str; 5] = [
    "Poisson external arrivals",
    "exponential service times",
    "independent stations with infinite waiting rooms",
    "single customer class",
    "visit ratios are supplied and consistent with the routing",
];

/// Solves an open network.
pub fn analyze_open(
    network: &QueueNetwork,
    options: &SolverOptions,
    cancel: &dyn Cancel,
) -> Result<AnalysisResult, SystemsError> {
    let Workload::Open(open) = &network.workload else {
        return Err(SystemsError::unsupported_model(
            "open Jackson analysis requires an open workload",
        ));
    };
    if open.arrival_rate.is_negative() {
        return Err(SystemsError::invalid_arrival_rate(
            &open.arrival_rate.to_string(),
        ));
    }
    options.check_rational(&open.arrival_rate)?;

    let mut stations: Vec<StationResult> = Vec::with_capacity(network.stations.len());
    let mut network_response = Q::zero();
    let mut queue_length = Q::zero();
    let mut law_checks: Vec<LawCheck> = Vec::new();
    let mut visited = Q::zero();

    for station in &network.stations {
        if cancel.is_cancelled() {
            return Err(SystemsError::cancelled());
        }
        let arrival = station.visits.mul(&open.arrival_rate);
        options.check_rational(&arrival)?;
        let solved = solve_station(
            station.id.as_str(),
            &arrival,
            &station.service_time,
            &station.visits,
            station.discipline,
            options,
            cancel,
        )?;
        law_checks.push(check_forced_flow(
            &solved.throughput,
            &station.visits,
            &open.arrival_rate,
        ));
        if station.visits.is_positive() {
            visited = visited.add(&station.visits);
        }
        network_response = network_response.add(&station.visits.mul(&solved.response_time));
        queue_length = queue_length.add(&solved.queue_length);
        stations.push(solved);
    }

    let empty_traffic = visited.is_zero();
    let throughput = if empty_traffic {
        Q::zero()
    } else {
        open.arrival_rate.clone()
    };
    let occupancy = throughput.mul(&network_response);
    law_checks.push(check_little(&occupancy, &throughput, &network_response));

    let mut provenance = Provenance::exact(
        "open-jackson",
        "jackson.open-mmc",
        JACKSON_ASSUMPTIONS.to_vec(),
        options.precision,
    );
    if empty_traffic {
        provenance.status = SolverStatus::Degenerate;
        provenance.warnings.push(
            "no station has a positive visit ratio, so the network carries no traffic".to_string(),
        );
    } else if open.arrival_rate.is_zero() {
        provenance.status = SolverStatus::Degenerate;
        provenance
            .warnings
            .push("the arrival rate is zero: the network is empty".to_string());
    }
    if stations.iter().any(|station| station.servers.is_none()) {
        provenance.warnings.push(
            "a delay center carries no capacity bound; its occupancy can exceed its server count"
                .to_string(),
        );
    }

    Ok(AnalysisResult {
        provenance,
        stations,
        aggregate: AggregateResult {
            throughput,
            response_time: network_response,
            occupancy,
            queue_length,
            population: None,
            think_time: None,
            law_checks,
        },
        populations: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{OpenWorkload, ServiceDiscipline, Station};

    fn station(id: &str, discipline: ServiceDiscipline, service_time: Q, visits: Q) -> Station {
        Station::new(id, discipline, service_time, visits).expect("valid station")
    }

    fn network(stations: Vec<Station>, rate: Q) -> QueueNetwork {
        QueueNetwork::new(
            "open",
            Workload::Open(OpenWorkload { arrival_rate: rate }),
            stations,
        )
        .expect("valid network")
    }

    #[test]
    fn a_two_station_jackson_network_is_exact() {
        let model = network(
            vec![
                station(
                    "api",
                    ServiceDiscipline::FcfsSingleServer,
                    Q::from_parts(
                        lovelace_int::Int::from_i64(1),
                        lovelace_nat::Nat::from_u64(20),
                    )
                    .unwrap(),
                    Q::one(),
                ),
                station(
                    "db",
                    ServiceDiscipline::FcfsSingleServer,
                    Q::from_parts(
                        lovelace_int::Int::from_i64(1),
                        lovelace_nat::Nat::from_u64(50),
                    )
                    .unwrap(),
                    Q::from_u64(2),
                ),
            ],
            Q::from_i64(8),
        );
        let result = analyze_open(&model, &SolverOptions::default(), &crate::NeverCancel)
            .expect("stable network");
        let api = result.station("api").expect("api");
        let db = result.station("db").expect("db");
        // api: X=8, S=1/20, U=2/5, W=S/(1-U)=1/12
        assert_eq!(api.throughput, Q::from_i64(8));
        assert_eq!(
            api.utilization,
            Q::from_i64(2).div(&Q::from_i64(5)).unwrap()
        );
        assert_eq!(
            api.response_time,
            Q::from_i64(1).div(&Q::from_i64(12)).unwrap()
        );
        // db: X=16, S=1/50, U=16/50=8/25, W=S/(1-U)= (1/50)/(17/25)=1/34
        assert_eq!(db.throughput, Q::from_i64(16));
        assert_eq!(
            db.utilization,
            Q::from_i64(8).div(&Q::from_i64(25)).unwrap()
        );
        assert_eq!(
            db.response_time,
            Q::from_i64(1).div(&Q::from_i64(34)).unwrap()
        );
        // R = 1*(1/12) + 2*(1/34) = 1/12 + 1/17 = 29/204
        assert_eq!(
            result.aggregate.response_time,
            Q::from_i64(29).div(&Q::from_i64(204)).unwrap()
        );
        // L = 8 * 29/204 = 58/51
        assert_eq!(
            result.aggregate.occupancy,
            Q::from_i64(58).div(&Q::from_i64(51)).unwrap()
        );
        assert!(result.laws_hold());
    }

    #[test]
    fn an_unstable_station_is_refused_and_named() {
        let model = network(
            vec![
                station(
                    "fast",
                    ServiceDiscipline::FcfsSingleServer,
                    Q::from_parts(
                        lovelace_int::Int::from_i64(1),
                        lovelace_nat::Nat::from_u64(10),
                    )
                    .unwrap(),
                    Q::one(),
                ),
                station(
                    "slow",
                    ServiceDiscipline::FcfsSingleServer,
                    Q::from_parts(
                        lovelace_int::Int::from_i64(1),
                        lovelace_nat::Nat::from_u64(4),
                    )
                    .unwrap(),
                    Q::from_u64(2),
                ),
            ],
            Q::from_i64(8),
        );
        let error = analyze_open(&model, &SolverOptions::default(), &crate::NeverCancel)
            .expect_err("slow station is unstable");
        assert_eq!(error.code(), "UnstableStation");
        assert!(error.message().contains("'slow'"), "{}", error.message());
    }

    #[test]
    fn zero_arrival_is_defined_with_zero_occupancy() {
        let model = network(
            vec![station(
                "api",
                ServiceDiscipline::FcfsSingleServer,
                Q::from_i64(1).div(&Q::from_i64(10)).unwrap(),
                Q::one(),
            )],
            Q::zero(),
        );
        let result =
            analyze_open(&model, &SolverOptions::default(), &crate::NeverCancel).expect("defined");
        assert_eq!(result.provenance.status, SolverStatus::Degenerate);
        assert_eq!(result.aggregate.throughput, Q::zero());
        assert_eq!(result.aggregate.occupancy, Q::zero());
        assert_eq!(result.stations[0].utilization, Q::zero());
    }

    #[test]
    fn all_zero_visits_is_a_degenerate_model_not_a_panic() {
        let model = network(
            vec![station(
                "unused",
                ServiceDiscipline::FcfsSingleServer,
                Q::one(),
                Q::zero(),
            )],
            Q::from_i64(4),
        );
        let result =
            analyze_open(&model, &SolverOptions::default(), &crate::NeverCancel).expect("defined");
        assert_eq!(result.provenance.status, SolverStatus::Degenerate);
        assert_eq!(result.aggregate.throughput, Q::zero());
        assert_eq!(result.aggregate.response_time, Q::zero());
    }
}
