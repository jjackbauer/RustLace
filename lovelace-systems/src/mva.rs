//! Exact single-class Mean Value Analysis for a closed product-form network.
//!
//! Supported nodes: FCFS load-independent single-server stations and infinite-server delay
//! centers, one customer class, visit ratios and a think time. The recurrence is the standard
//! arrival theorem:
//!
//! ```text
//! R_i(n) = D_i * (1 + Q_i(n-1))   FCFS single server
//! R_i(n) = D_i                     delay center
//! X(n)   = n / (Z + sum_i R_i(n))
//! Q_i(n) = X(n) * R_i(n)
//! U_i(n) = X(n) * D_i              single server; X*D is the mean occupied servers at a delay
//! ```
//!
//! The result is mathematically exact (every value is a rational); "exact" here is a statement
//! about the model's closed-form solution, not a claim that an arbitrary real-world system follows
//! the product-form assumptions. `O(N*K)` arithmetic and `O(K)` state are used unless intermediate
//! populations are requested, in which case `N*K` is bounded by `SolverOptions::max_state_space`.
//!
//! Multi-server FCFS, priorities, finite buffers, class switching and mixed workloads are refused
//! with the failing assumption named; a closed model is never approximated by an M/M/1 network.

use crate::Q;
use crate::errors::SystemsError;
use crate::model::{Cancel, QueueNetwork, ServiceDiscipline, SolverOptions, Workload};
use crate::operational_laws::{check_forced_flow, check_interactive, check_little};
use crate::results::{
    AggregateResult, AnalysisResult, LawCheck, PopulationPoint, Provenance, SolverStatus,
    StationResult,
};

/// The assumptions a closed MVA result rests on.
pub const MVA_ASSUMPTIONS: [&str; 5] = [
    "single customer class",
    "FCFS load-independent single-server stations and delay centers only",
    "product-form (BCMP) service discipline",
    "visit ratios and think time describe a closed population",
    "the arrival theorem holds for the supported nodes",
];

/// Solves a closed network with exact single-class MVA.
pub fn analyze_closed(
    network: &QueueNetwork,
    options: &SolverOptions,
    cancel: &dyn Cancel,
) -> Result<AnalysisResult, SystemsError> {
    let Workload::Closed(closed) = &network.workload else {
        return Err(SystemsError::unsupported_model(
            "closed MVA requires a closed workload",
        ));
    };
    for station in &network.stations {
        if matches!(station.discipline, ServiceDiscipline::MultiServer { .. }) {
            return Err(SystemsError::unsupported_model(&format!(
                "station '{}' is M/M/c FCFS, whose service distribution is not product-form for multiclass exact MVA and is not supported by this single-class solver",
                station.id
            )));
        }
    }
    let population = closed.population;
    options.check_iterations(population)?;
    let stations = &network.stations;
    let count = stations.len();

    let demands: Vec<Q> = stations
        .iter()
        .map(|station| station.service_demand())
        .collect();
    for demand in &demands {
        options.check_rational(demand)?;
    }
    let total_demand = demands
        .iter()
        .fold(Q::zero(), |acc, demand| acc.add(demand));

    let mut provenance = Provenance::exact(
        "closed-mva",
        "mva.single-class",
        MVA_ASSUMPTIONS.to_vec(),
        options.precision,
    );

    // N = 0: a well-defined empty model.
    let mut collect = options.collect_populations;
    if collect {
        let entries = population.saturating_mul(count as u64);
        if entries > options.max_state_space {
            return Err(SystemsError::state_space_budget(
                options.max_state_space,
                entries,
            ));
        }
        if population == 0 {
            collect = false;
        }
    }

    if population == 0 {
        provenance.status = SolverStatus::Degenerate;
        provenance
            .warnings
            .push("the closed population is zero: every metric is zero".to_string());
        let stations_out = stations
            .iter()
            .map(|station| {
                let demand = station.service_demand();
                StationResult {
                    id: station.id.to_string(),
                    discipline: station.discipline.as_str(),
                    servers: station.servers(),
                    visits: station.visits.clone(),
                    service_time: station.service_time.clone(),
                    service_demand: demand,
                    throughput: Q::zero(),
                    utilization: Q::zero(),
                    busy_servers: Q::zero(),
                    response_time: Q::zero(),
                    waiting_time: Q::zero(),
                    occupancy: Q::zero(),
                    queue_length: Q::zero(),
                    probability_waiting: None,
                }
            })
            .collect();
        return Ok(AnalysisResult {
            provenance,
            stations: stations_out,
            aggregate: AggregateResult {
                throughput: Q::zero(),
                response_time: Q::zero(),
                occupancy: Q::zero(),
                queue_length: Q::zero(),
                population: Some(0),
                think_time: Some(closed.think_time.clone()),
                law_checks: vec![check_interactive(
                    0,
                    &Q::zero(),
                    &Q::zero(),
                    &closed.think_time,
                )],
            },
            populations: None,
        });
    }

    // No service demand at all: with a positive think time the population thinks forever;
    // with no think time the cycle is 0/0 and there is no defined throughput.
    if total_demand.is_zero() {
        if closed.think_time.is_zero() {
            return Err(SystemsError::degenerate_model(
                "the network has zero total service demand and zero think time",
            ));
        }
        let throughput = Q::from_u64(population)
            .div(&closed.think_time)
            .ok_or_else(|| SystemsError::degenerate_model("the think time is zero"))?;
        options.check_rational(&throughput)?;
        provenance.status = SolverStatus::Degenerate;
        provenance.warnings.push(
            "no station carries service demand: the population never leaves the think state"
                .to_string(),
        );
        let stations_out = stations
            .iter()
            .map(|station| StationResult {
                id: station.id.to_string(),
                discipline: station.discipline.as_str(),
                servers: station.servers(),
                visits: station.visits.clone(),
                service_time: station.service_time.clone(),
                service_demand: station.service_demand(),
                throughput: Q::zero(),
                utilization: Q::zero(),
                busy_servers: Q::zero(),
                response_time: Q::zero(),
                waiting_time: Q::zero(),
                occupancy: Q::zero(),
                queue_length: Q::zero(),
                probability_waiting: None,
            })
            .collect();
        return Ok(AnalysisResult {
            provenance,
            stations: stations_out,
            aggregate: AggregateResult {
                throughput: throughput.clone(),
                response_time: Q::zero(),
                occupancy: Q::zero(),
                queue_length: Q::zero(),
                population: Some(population),
                think_time: Some(closed.think_time.clone()),
                law_checks: vec![check_interactive(
                    population,
                    &throughput,
                    &Q::zero(),
                    &closed.think_time,
                )],
            },
            populations: None,
        });
    }

    let mut previous = vec![Q::zero(); count];
    let mut current = vec![Q::zero(); count];
    let mut response = vec![Q::zero(); count];
    let mut population_points: Vec<PopulationPoint> = Vec::new();
    let mut throughput = Q::zero();
    let mut network_response = Q::zero();

    for n in 1..=population {
        if cancel.is_cancelled() {
            return Err(SystemsError::cancelled());
        }
        let mut sum_response = Q::zero();
        for index in 0..count {
            let demand = &demands[index];
            let value = match stations[index].discipline {
                ServiceDiscipline::Delay => demand.clone(),
                ServiceDiscipline::FcfsSingleServer => demand.mul(&Q::one().add(&previous[index])),
                ServiceDiscipline::MultiServer { .. } => {
                    return Err(SystemsError::unsupported_model(
                        "a multi-server FCFS station is not supported by exact single-class MVA",
                    ));
                }
            };
            options.check_rational(&value)?;
            sum_response = sum_response.add(&value);
            response[index] = value;
        }
        let cycle = closed.think_time.add(&sum_response);
        if cycle.is_zero() {
            return Err(SystemsError::degenerate_model(
                "the cycle time is zero, so the throughput is 0/0",
            ));
        }
        throughput = Q::from_u64(n)
            .div(&cycle)
            .ok_or_else(|| SystemsError::degenerate_model("the cycle time is zero"))?;
        options.check_rational(&throughput)?;

        for index in 0..count {
            current[index] = throughput.mul(&response[index]);
            options.check_rational(&current[index])?;
        }
        network_response = sum_response;
        // Occupancy excludes the customers outside the network in their think state:
        // Q_network(n) = n - X(n) * Z (RNL-MIG-006).
        let in_network = Q::from_u64(n).sub(&throughput.mul(&closed.think_time));
        if in_network.is_negative() {
            return Err(SystemsError::invariant(
                "the in-network occupancy is negative",
            ));
        }
        let station_sum = current.iter().fold(Q::zero(), |acc, value| acc.add(value));
        if station_sum != in_network {
            return Err(SystemsError::invariant(&format!(
                "at population {n} the station occupancies sum to {station_sum}, but n - X*Z is {in_network}"
            )));
        }
        if collect {
            population_points.push(PopulationPoint {
                population: n,
                throughput: throughput.clone(),
                response_time: network_response.clone(),
                occupancy: in_network,
                station_occupancy: current.clone(),
            });
        }
        std::mem::swap(&mut previous, &mut current);
    }

    let occupancy: Q = previous.iter().fold(Q::zero(), |acc, value| acc.add(value));
    let final_in_network = Q::from_u64(population).sub(&throughput.mul(&closed.think_time));
    if occupancy != final_in_network {
        return Err(SystemsError::invariant(&format!(
            "the aggregate occupancy {occupancy} differs from n - X*Z ({final_in_network})"
        )));
    }
    let mut stations_out: Vec<StationResult> = Vec::with_capacity(count);
    let mut queue_length = Q::zero();
    let mut law_checks: Vec<LawCheck> = Vec::new();
    for (index, station) in stations.iter().enumerate() {
        let station_throughput = throughput.mul(&station.visits);
        let utilization = throughput.mul(&station.service_demand());
        if matches!(station.discipline, ServiceDiscipline::FcfsSingleServer)
            && utilization > Q::one()
        {
            return Err(SystemsError::invariant(&format!(
                "station '{}' has utilization {} above 1",
                station.id, utilization
            )));
        }
        // `response[index]` is the per-transaction residence `R_i(n)` of the recurrence. The
        // published station metric is per VISIT, which is what a caller compares with the
        // service time; the network aggregate uses the per-transaction value.
        let visit_response = if station.visits.is_zero() {
            Q::zero()
        } else {
            response[index]
                .div(&station.visits)
                .ok_or_else(|| SystemsError::internal("the visit ratio divided by zero"))?
        };
        let waiting = if matches!(station.discipline, ServiceDiscipline::Delay) {
            Q::zero()
        } else {
            let raw = visit_response.sub(&station.service_time);
            if raw.is_negative() {
                return Err(SystemsError::invariant(&format!(
                    "station '{}' has a negative waiting time",
                    station.id
                )));
            }
            raw
        };
        let queue = previous[index].sub(&utilization);
        let queue = if queue.is_negative() {
            return Err(SystemsError::invariant(&format!(
                "station '{}' has a negative queue length",
                station.id
            )));
        } else {
            queue
        };
        law_checks.push(check_forced_flow(
            &station_throughput,
            &station.visits,
            &throughput,
        ));
        law_checks.push(check_little(
            &previous[index],
            &station_throughput,
            &visit_response,
        ));
        queue_length = queue_length.add(&queue);
        stations_out.push(StationResult {
            id: station.id.to_string(),
            discipline: station.discipline.as_str(),
            servers: station.servers(),
            visits: station.visits.clone(),
            service_time: station.service_time.clone(),
            service_demand: station.service_demand(),
            throughput: station_throughput,
            utilization: utilization.clone(),
            busy_servers: utilization,
            response_time: visit_response,
            waiting_time: waiting,
            occupancy: previous[index].clone(),
            queue_length: queue,
            probability_waiting: None,
        });
    }

    law_checks.push(check_interactive(
        population,
        &throughput,
        &network_response,
        &closed.think_time,
    ));
    law_checks.push(check_little(&occupancy, &throughput, &network_response));

    let result = AnalysisResult {
        provenance,
        stations: stations_out,
        aggregate: AggregateResult {
            throughput,
            response_time: network_response,
            occupancy,
            queue_length,
            population: Some(population),
            think_time: Some(closed.think_time.clone()),
            law_checks,
        },
        populations: collect.then_some(population_points),
    };
    if !result.laws_hold() {
        return Err(SystemsError::invariant(
            "a conservation law did not hold exactly",
        ));
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{ClosedWorkload, NeverCancel, Station};

    fn station(id: &str, discipline: ServiceDiscipline, service_time: Q, visits: Q) -> Station {
        Station::new(id, discipline, service_time, visits).expect("valid station")
    }

    fn closed(stations: Vec<Station>, population: u64, think: Q) -> QueueNetwork {
        QueueNetwork::new(
            "closed",
            Workload::Closed(ClosedWorkload {
                population,
                think_time: think,
            }),
            stations,
        )
        .expect("valid network")
    }

    fn hundredth() -> Q {
        Q::from_i64(1).div(&Q::from_i64(100)).unwrap()
    }

    #[test]
    fn golden_single_center_n1_and_n2() {
        let model = closed(
            vec![station(
                "cpu",
                ServiceDiscipline::FcfsSingleServer,
                hundredth(),
                Q::one(),
            )],
            1,
            Q::from_i64(9).div(&Q::from_i64(100)).unwrap(),
        );
        let one = analyze_closed(&model, &SolverOptions::default(), &NeverCancel).expect("N=1");
        assert_eq!(one.aggregate.throughput, Q::from_i64(10));
        assert_eq!(one.stations[0].response_time, hundredth());
        assert_eq!(
            one.stations[0].utilization,
            Q::from_i64(1).div(&Q::from_i64(10)).unwrap()
        );
        assert_eq!(
            one.stations[0].occupancy,
            Q::from_i64(1).div(&Q::from_i64(10)).unwrap()
        );
        assert!(one.laws_hold());

        let two = analyze_closed(
            &closed(
                vec![station(
                    "cpu",
                    ServiceDiscipline::FcfsSingleServer,
                    hundredth(),
                    Q::one(),
                )],
                2,
                Q::from_i64(9).div(&Q::from_i64(100)).unwrap(),
            ),
            &SolverOptions::default(),
            &NeverCancel,
        )
        .expect("N=2");
        // R(2) = D*(1+Q(1)) = (1/100)*(11/10) = 11/1000
        assert_eq!(
            two.stations[0].response_time,
            Q::from_i64(11).div(&Q::from_i64(1000)).unwrap()
        );
        // X(2) = 2 / (9/100 + 11/1000) = 2 / (101/1000) = 2000/101
        assert_eq!(
            two.aggregate.throughput,
            Q::from_i64(2000).div(&Q::from_i64(101)).unwrap()
        );
        // Q(2) = X*R = (2000/101)*(11/1000) = 22/101
        assert_eq!(
            two.stations[0].occupancy,
            Q::from_i64(22).div(&Q::from_i64(101)).unwrap()
        );
        assert!(two.laws_hold());
    }

    #[test]
    fn a_delay_center_has_no_wait_and_carries_its_occupancy() {
        let model = closed(
            vec![
                station(
                    "cpu",
                    ServiceDiscipline::FcfsSingleServer,
                    hundredth(),
                    Q::one(),
                ),
                station(
                    "io",
                    ServiceDiscipline::Delay,
                    Q::from_i64(1).div(&Q::from_i64(50)).unwrap(),
                    Q::one(),
                ),
            ],
            2,
            Q::zero(),
        );
        let result =
            analyze_closed(&model, &SolverOptions::default(), &NeverCancel).expect("solved");
        let io = result.station("io").expect("io");
        assert_eq!(io.waiting_time, Q::zero());
        assert_eq!(io.queue_length, Q::zero());
        assert!(io.occupancy > Q::one());
        assert_eq!(result.aggregate.occupancy, Q::from_u64(2));
        assert!(result.laws_hold());
    }

    #[test]
    fn zero_population_is_all_zero_and_n0_is_not_0_over_0() {
        let model = closed(
            vec![station(
                "cpu",
                ServiceDiscipline::FcfsSingleServer,
                hundredth(),
                Q::one(),
            )],
            0,
            Q::zero(),
        );
        let result = analyze_closed(&model, &SolverOptions::default(), &NeverCancel)
            .expect("N=0 is defined");
        assert_eq!(result.provenance.status, SolverStatus::Degenerate);
        assert_eq!(result.aggregate.throughput, Q::zero());
        assert_eq!(result.stations[0].occupancy, Q::zero());
        assert!(result.laws_hold());
    }

    #[test]
    fn zero_demand_and_zero_think_is_a_domain_diagnostic() {
        let model = closed(
            vec![station(
                "unused",
                ServiceDiscipline::Delay,
                Q::zero(),
                Q::zero(),
            )],
            3,
            Q::zero(),
        );
        let error =
            analyze_closed(&model, &SolverOptions::default(), &NeverCancel).expect_err("0/0");
        assert_eq!(error.code(), "DegenerateModel");
    }

    #[test]
    fn zero_demand_with_think_time_thinks_forever() {
        let model = closed(
            vec![station(
                "unused",
                ServiceDiscipline::Delay,
                Q::zero(),
                Q::zero(),
            )],
            3,
            Q::from_i64(2),
        );
        let result =
            analyze_closed(&model, &SolverOptions::default(), &NeverCancel).expect("defined");
        assert_eq!(
            result.aggregate.throughput,
            Q::from_i64(3).div(&Q::from_i64(2)).unwrap()
        );
        assert_eq!(result.aggregate.response_time, Q::zero());
    }

    #[test]
    fn multi_server_is_refused_with_the_assumption_named() {
        let model = closed(
            vec![station(
                "pool",
                ServiceDiscipline::MultiServer { servers: 2 },
                Q::one(),
                Q::one(),
            )],
            1,
            Q::zero(),
        );
        let error = analyze_closed(&model, &SolverOptions::default(), &NeverCancel)
            .expect_err("unsupported");
        assert_eq!(error.code(), "UnsupportedModel");
        assert!(error.message().contains("M/M/c"), "{}", error.message());
    }

    #[test]
    fn the_state_space_budget_is_checked_before_allocation() {
        let model = closed(
            vec![station(
                "cpu",
                ServiceDiscipline::FcfsSingleServer,
                hundredth(),
                Q::one(),
            )],
            2,
            Q::zero(),
        );
        let options = SolverOptions {
            collect_populations: true,
            max_state_space: 1,
            ..SolverOptions::default()
        };
        let error = analyze_closed(&model, &options, &NeverCancel).expect_err("budget");
        assert_eq!(error.code(), "StateSpaceBudgetExceeded");
    }

    /// The retained state is exactly `N*K` entries when collection is requested, and each point
    /// carries one occupancy per station in declaration order. This pins the complexity shape
    /// without a wall-clock assertion.
    #[test]
    fn the_collected_population_size_is_n_times_k() {
        let population = 6u64;
        let model = closed(
            vec![
                station(
                    "cpu",
                    ServiceDiscipline::FcfsSingleServer,
                    Q::from_i64(1).div(&Q::from_i64(10)).unwrap(),
                    Q::one(),
                ),
                station(
                    "io",
                    ServiceDiscipline::Delay,
                    Q::from_i64(1).div(&Q::from_i64(5)).unwrap(),
                    Q::one(),
                ),
            ],
            population,
            Q::zero(),
        );
        let options = SolverOptions {
            collect_populations: true,
            ..SolverOptions::default()
        };
        let result = analyze_closed(&model, &options, &NeverCancel).expect("solved");
        let points = result.populations.expect("populations were requested");
        assert_eq!(points.len() as u64, population);
        for (index, point) in points.iter().enumerate() {
            assert_eq!(point.population, index as u64 + 1);
            assert_eq!(point.station_occupancy.len(), 2);
        }
        assert_eq!(result.stations.len(), 2);
    }

    /// A deterministic randomized sweep over populations, station counts, visit ratios, service
    /// times and think times. Every solved model must satisfy the exact conservation identities
    /// `N = X(R+Z)`, `Q_i = X*R_i` and `U_i = X*D_i`, and publish no `holds: false` check.
    #[test]
    fn randomized_closed_models_satisfy_every_conservation_identity() {
        let mut state = 0x0d15_ea5e_u64;
        let mut next = || {
            state = state
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            (state >> 33) as i64
        };
        let mut solved = 0;
        for _ in 0..24 {
            let count = 1 + (next() % 4) as usize;
            let population = 1 + (next() % 8) as u64;
            let think = Q::from_u64((next() % 3) as u64)
                .div(&Q::from_u64(10))
                .expect("10");
            let mut stations = Vec::with_capacity(count);
            for index in 0..count {
                let service = Q::from_i64(1 + next() % 20)
                    .div(&Q::from_i64(100))
                    .expect("100");
                let visits = Q::from_u64(1 + (next() % 3) as u64);
                let discipline = if next() % 3 == 0 {
                    ServiceDiscipline::Delay
                } else {
                    ServiceDiscipline::FcfsSingleServer
                };
                stations.push(station(&format!("s{index}"), discipline, service, visits));
            }
            let model = closed(stations, population, think.clone());
            let result = analyze_closed(&model, &SolverOptions::default(), &NeverCancel)
                .expect("the generated model is stable by construction");
            let throughput = result.aggregate.throughput.clone();
            let response = result.aggregate.response_time.clone();
            assert_eq!(
                Q::from_u64(population),
                throughput.mul(&response.add(&think)),
                "N = X(R+Z)"
            );
            for (station_result, station) in result.stations.iter().zip(&model.stations) {
                let expected_occupancy =
                    throughput.mul(&station_result.response_time.mul(&station.visits));
                assert_eq!(
                    station_result.occupancy, expected_occupancy,
                    "Q_i = X * R_i (per transaction) at {}",
                    station.id
                );
                if matches!(station.discipline, ServiceDiscipline::FcfsSingleServer) {
                    assert_eq!(
                        station_result.utilization,
                        throughput.mul(&station.service_demand()),
                        "U_i = X * D_i at {}",
                        station.id
                    );
                }
            }
            assert!(result.laws_hold(), "every published law check holds");
            solved += 1;
        }
        assert!(solved >= 20, "the randomized sweep solved only {solved}");
    }

    /// RNL-MIG-006: a collected population point reports the in-network occupancy, not the whole
    /// closed population. With a positive think time some customers are always thinking, so
    /// `Q_network(n) = n - X(n) * Z`, and that value must equal the sum of station occupancies.
    #[test]
    fn collected_populations_exclude_customers_in_think_state() {
        let think = Q::from_i64(1).div(&Q::from_i64(2)).unwrap();
        let model = closed(
            vec![
                station(
                    "cpu",
                    ServiceDiscipline::FcfsSingleServer,
                    Q::from_i64(1).div(&Q::from_i64(10)).unwrap(),
                    Q::one(),
                ),
                station(
                    "io",
                    ServiceDiscipline::Delay,
                    Q::from_i64(1).div(&Q::from_i64(5)).unwrap(),
                    Q::one(),
                ),
            ],
            4,
            think.clone(),
        );
        let options = SolverOptions {
            collect_populations: true,
            ..SolverOptions::default()
        };
        let result = analyze_closed(&model, &options, &NeverCancel).expect("solved");
        let points = result
            .populations
            .as_ref()
            .expect("populations were requested");
        assert_eq!(points.len(), 4);
        for point in points {
            let expected = Q::from_u64(point.population).sub(&point.throughput.mul(&think));
            assert_eq!(point.occupancy, expected, "Q_network = n - X*Z");
            let station_sum = point
                .station_occupancy
                .iter()
                .fold(Q::zero(), |acc, value| acc.add(value));
            assert_eq!(point.occupancy, station_sum, "aggregate is the station sum");
            assert!(
                point.occupancy < Q::from_u64(point.population),
                "with positive think time a customer is in the think state at population {}",
                point.population
            );
        }
        // The final aggregate matches the last collected point and its station sum.
        let last = points.last().expect("non-empty");
        assert_eq!(result.aggregate.occupancy, last.occupancy);
        assert_eq!(
            result.aggregate.occupancy,
            Q::from_u64(4).sub(&result.aggregate.throughput.mul(&think))
        );
        let station_sum = result
            .stations
            .iter()
            .fold(Q::zero(), |acc, value| acc.add(&value.occupancy));
        assert_eq!(result.aggregate.occupancy, station_sum);
        assert!(result.laws_hold());
    }
}
