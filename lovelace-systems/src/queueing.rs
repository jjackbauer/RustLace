//! Single-station queueing models: M/M/1, M/M/c (Erlang C) and M/M/infinity.
//!
//! All arithmetic is exact rational; `precision` only affects a later decimal rendering. A stable
//! stationary result exists only when `lambda < c*mu`; a model at or above capacity is refused with
//! `UnstableStation` and publishes no finite metric. `lambda = 0` is a defined, empty arrival
//! stream and answers zero throughput/occupancy with the station's service time intact.

use crate::errors::SystemsError;
use crate::model::{Cancel, ServiceDiscipline, SolverOptions};
use crate::numeric::Q;
use crate::results::{AggregateResult, AnalysisResult, LawCheck, Provenance, StationResult};

/// The stable solver version published with every result.
pub const QUEUEING_ASSUMPTIONS: [&str; 4] = [
    "Poisson arrivals",
    "exponential service times",
    "infinite waiting room",
    "load-independent service",
];

/// M/M/1 from an arrival rate and a service rate.
pub fn mm1(
    lambda: &Q,
    mu: &Q,
    options: &SolverOptions,
    cancel: &dyn Cancel,
) -> Result<AnalysisResult, SystemsError> {
    let service_time = require_service_rate(mu)?;
    analyze_one(
        lambda,
        &service_time,
        ServiceDiscipline::FcfsSingleServer,
        options,
        cancel,
        "station-mm1",
        "mm1.closed-form",
        vec![
            "Poisson arrivals at rate lambda",
            "exponential service at rate mu",
            "one FCFS server with an infinite waiting room",
            "load-independent service",
        ],
    )
}

/// M/M/c (Erlang C) from an arrival rate, a per-server service rate and a server count.
pub fn mmc(
    lambda: &Q,
    mu: &Q,
    servers: usize,
    options: &SolverOptions,
    cancel: &dyn Cancel,
) -> Result<AnalysisResult, SystemsError> {
    if !(2..=crate::validation::MAX_SERVERS).contains(&servers) {
        return Err(SystemsError::unsupported_discipline(
            "station",
            &format!(
                "M/M/{servers} (the server count must be 2 to {})",
                crate::validation::MAX_SERVERS
            ),
        ));
    }
    let service_time = require_service_rate(mu)?;
    analyze_one(
        lambda,
        &service_time,
        ServiceDiscipline::MultiServer { servers },
        options,
        cancel,
        "station-mmc",
        "mmc.erlang-c",
        vec![
            "Poisson arrivals at rate lambda",
            "exponential service at rate mu per server",
            "c identical FCFS servers with an infinite waiting room",
            "load-independent service",
        ],
    )
}

/// M/M/infinity (a delay center) from an arrival rate and a per-server service rate.
pub fn mm_infinity(
    lambda: &Q,
    mu: &Q,
    options: &SolverOptions,
    cancel: &dyn Cancel,
) -> Result<AnalysisResult, SystemsError> {
    let service_time = require_service_rate(mu)?;
    analyze_one(
        lambda,
        &service_time,
        ServiceDiscipline::Delay,
        options,
        cancel,
        "station-mminf",
        "mminf.closed-form",
        vec![
            "Poisson arrivals at rate lambda",
            "exponential service at rate mu",
            "infinitely many servers, so no customer waits",
        ],
    )
}

/// Solves one station with a known arrival rate and service time.
///
/// This is the shared kernel of the single-station API and of the open-network solver: a Jackson
/// network's station is an M/M/c queue with Poisson arrivals at its visit rate.
pub fn solve_station(
    id: &str,
    arrival: &Q,
    service_time: &Q,
    visits: &Q,
    discipline: ServiceDiscipline,
    options: &SolverOptions,
    cancel: &dyn Cancel,
) -> Result<StationResult, SystemsError> {
    if cancel.is_cancelled() {
        return Err(SystemsError::cancelled());
    }
    if arrival.is_negative() {
        return Err(SystemsError::invalid_arrival_rate(&arrival.to_string()));
    }
    if visits.is_negative() {
        return Err(SystemsError::invalid_visits(id, &visits.to_string()));
    }
    if arrival.is_positive() && !service_time.is_positive() {
        return Err(SystemsError::invalid_service_time(
            id,
            &service_time.to_string(),
        ));
    }
    options.check_rational(arrival)?;
    options.check_rational(service_time)?;

    let demand = visits.mul(service_time);
    let servers = discipline.servers();
    let (
        utilization,
        busy_servers,
        response_time,
        waiting_time,
        occupancy,
        queue_length,
        probability_waiting,
    ) = match discipline {
        ServiceDiscipline::Delay => {
            // a = X*S is the mean number in service; it may exceed 1 because there is no queue.
            let occupied = arrival.mul(service_time);
            options.check_rational(&occupied)?;
            (
                occupied.clone(),
                occupied.clone(),
                service_time.clone(),
                Q::zero(),
                occupied,
                Q::zero(),
                None,
            )
        }
        ServiceDiscipline::FcfsSingleServer => {
            let rho = arrival.mul(service_time);
            options.check_rational(&rho)?;
            if arrival.is_positive() && rho >= Q::one() {
                return Err(SystemsError::unstable_station(id, &rho.to_string()));
            }
            let one_minus = Q::one().sub(&rho);
            let response = service_time.div(&one_minus).ok_or_else(|| {
                SystemsError::degenerate_model("the station's service capacity is zero")
            })?;
            let wait = rho.mul(&response);
            let occupied = arrival.mul(&response);
            let queued = arrival.mul(&wait);
            (
                rho,
                arrival.mul(service_time),
                response,
                wait,
                occupied,
                queued,
                None,
            )
        }
        ServiceDiscipline::MultiServer { servers } => {
            let traffic = arrival.mul(service_time);
            let rho = traffic.div(&Q::from_u64(servers as u64)).ok_or_else(|| {
                SystemsError::internal("a rational division by the server count failed")
            })?;
            options.check_rational(&traffic)?;
            options.check_rational(&rho)?;
            if arrival.is_positive() && rho >= Q::one() {
                return Err(SystemsError::unstable_station(id, &rho.to_string()));
            }
            let probability = erlang_c(&traffic, servers, options)?;
            let spare = Q::from_u64(servers as u64)
                .mul(&Q::one().div(service_time).unwrap_or_else(Q::zero))
                .sub(arrival);
            let wait = probability.div(&spare).ok_or_else(|| {
                SystemsError::degenerate_model("the multi-server station has no spare capacity")
            })?;
            let response = wait.add(service_time);
            let occupied = arrival.mul(&response);
            let queued = arrival.mul(&wait);
            (
                rho,
                traffic,
                response,
                wait,
                occupied,
                queued,
                Some(probability),
            )
        }
    };

    for value in [
        &utilization,
        &busy_servers,
        &response_time,
        &waiting_time,
        &occupancy,
        &queue_length,
    ] {
        options.check_rational(value)?;
    }
    if cancel.is_cancelled() {
        return Err(SystemsError::cancelled());
    }

    Ok(StationResult {
        id: id.to_string(),
        discipline: discipline.as_str(),
        servers,
        visits: visits.clone(),
        service_time: service_time.clone(),
        service_demand: demand,
        throughput: arrival.clone(),
        utilization,
        busy_servers,
        response_time,
        waiting_time,
        occupancy,
        queue_length,
        probability_waiting,
    })
}

/// The Erlang C probability that an arrival waits, computed by the stable recurrence
/// `p = tail / (partial_sum + tail)` with `tail = a^c / (c! (1-rho))`.
fn erlang_c(traffic: &Q, servers: usize, options: &SolverOptions) -> Result<Q, SystemsError> {
    let mut term = Q::one();
    let mut partial = Q::one();
    for k in 1..servers {
        term = term
            .mul(traffic)
            .div(&Q::from_u64(k as u64))
            .ok_or_else(|| SystemsError::internal("the Erlang recurrence divided by zero"))?;
        options.check_rational(&term)?;
        partial = partial.add(&term);
    }
    let last = term
        .mul(traffic)
        .div(&Q::from_u64(servers as u64))
        .ok_or_else(|| SystemsError::internal("the Erlang recurrence divided by zero"))?;
    options.check_rational(&last)?;
    let rho = traffic
        .div(&Q::from_u64(servers as u64))
        .ok_or_else(|| SystemsError::internal("the Erlang recurrence divided by zero"))?;
    let one_minus = Q::one().sub(&rho);
    if one_minus.is_zero() || one_minus.is_negative() {
        return Err(SystemsError::unstable_station("station", &rho.to_string()));
    }
    let tail = last.div(&one_minus).ok_or_else(|| {
        SystemsError::degenerate_model("the Erlang normalization has no spare capacity")
    })?;
    options.check_rational(&tail)?;
    tail.div(&partial.add(&tail))
        .ok_or_else(|| SystemsError::internal("the Erlang normalization was zero"))
}

fn require_service_rate(mu: &Q) -> Result<Q, SystemsError> {
    if !mu.is_positive() {
        return Err(SystemsError::invalid_service_rate(&mu.to_string()));
    }
    Q::one()
        .div(mu)
        .ok_or_else(|| SystemsError::invalid_service_rate(&mu.to_string()))
}

#[allow(clippy::too_many_arguments)]
fn analyze_one(
    lambda: &Q,
    service_time: &Q,
    discipline: ServiceDiscipline,
    options: &SolverOptions,
    cancel: &dyn Cancel,
    model_class: &'static str,
    solver_name: &'static str,
    assumptions: Vec<&'static str>,
) -> Result<AnalysisResult, SystemsError> {
    if lambda.is_negative() {
        return Err(SystemsError::invalid_arrival_rate(&lambda.to_string()));
    }
    let station = solve_station(
        "station",
        lambda,
        service_time,
        &Q::one(),
        discipline,
        options,
        cancel,
    )?;
    let throughput = station.throughput.clone();
    let response = station.response_time.clone();
    let occupancy = station.occupancy.clone();
    let queue = station.queue_length.clone();
    let little = LawCheck::exact("little", occupancy.clone(), throughput.mul(&response));
    let offered = throughput.mul(&station.service_time);
    let utilization_rhs = match station.servers {
        Some(servers) => offered
            .div(&Q::from_u64(servers as u64))
            .unwrap_or_else(Q::zero),
        None => offered,
    };
    let utilization = LawCheck::exact("utilization", station.utilization.clone(), utilization_rhs);
    let mut provenance =
        Provenance::exact(model_class, solver_name, assumptions, options.precision);
    provenance.status = if lambda.is_zero() {
        crate::results::SolverStatus::Degenerate
    } else {
        crate::results::SolverStatus::Solved
    };
    if lambda.is_zero() {
        provenance
            .warnings
            .push("the arrival rate is zero: the station is empty".to_string());
    }
    Ok(AnalysisResult {
        provenance,
        stations: vec![station],
        aggregate: AggregateResult {
            throughput,
            response_time: response,
            occupancy,
            queue_length: queue,
            population: None,
            think_time: None,
            law_checks: vec![little, utilization],
        },
        populations: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::NeverCancel;

    fn mm1_metrics(lambda: i64, mu: i64) -> AnalysisResult {
        mm1(
            &Q::from_i64(lambda),
            &Q::from_i64(mu),
            &SolverOptions::default(),
            &NeverCancel,
        )
        .expect("stable")
    }

    #[test]
    fn mm1_golden_metrics_are_exact() {
        let result = mm1_metrics(8, 10);
        let station = &result.stations[0];
        assert_eq!(
            station.utilization,
            Q::from_i64(4).div(&Q::from_i64(5)).unwrap()
        );
        assert_eq!(
            station.response_time,
            Q::from_i64(1).div(&Q::from_i64(2)).unwrap()
        );
        assert_eq!(
            station.waiting_time,
            Q::from_i64(2).div(&Q::from_i64(5)).unwrap()
        );
        assert_eq!(station.occupancy, Q::from_i64(4));
        assert_eq!(
            station.queue_length,
            Q::from_i64(16).div(&Q::from_i64(5)).unwrap()
        );
        assert_eq!(
            station.service_time,
            Q::from_i64(1).div(&Q::from_i64(10)).unwrap()
        );
        assert_eq!(station.service_demand, station.service_time);
        assert!(result.provenance.exactness.is_exact());
        assert!(result.laws_hold());
    }

    #[test]
    fn mm1_at_or_above_capacity_is_refused() {
        for (lambda, mu) in [(10, 10), (11, 10)] {
            let error = mm1(
                &Q::from_i64(lambda),
                &Q::from_i64(mu),
                &SolverOptions::default(),
                &NeverCancel,
            )
            .expect_err("unstable");
            assert_eq!(error.code(), "UnstableStation");
            assert_eq!(error.category().as_str(), "DomainError");
        }
    }

    #[test]
    fn mmc_golden_metrics_are_exact() {
        let result = mmc(
            &Q::from_i64(8),
            &Q::from_i64(5),
            2,
            &SolverOptions::default(),
            &NeverCancel,
        )
        .expect("stable");
        let station = &result.stations[0];
        assert_eq!(
            station.utilization,
            Q::from_i64(4).div(&Q::from_i64(5)).unwrap()
        );
        assert_eq!(
            station.probability_waiting,
            Some(Q::from_i64(32).div(&Q::from_i64(45)).unwrap())
        );
        assert_eq!(
            station.waiting_time,
            Q::from_i64(16).div(&Q::from_i64(45)).unwrap()
        );
        assert_eq!(
            station.response_time,
            Q::from_i64(5).div(&Q::from_i64(9)).unwrap()
        );
        assert_eq!(
            station.queue_length,
            Q::from_i64(128).div(&Q::from_i64(45)).unwrap()
        );
        assert_eq!(
            station.busy_servers,
            Q::from_i64(8).div(&Q::from_i64(5)).unwrap()
        );
    }

    #[test]
    fn mmc_at_capacity_is_refused() {
        let error = mmc(
            &Q::from_i64(10),
            &Q::from_i64(5),
            2,
            &SolverOptions::default(),
            &NeverCancel,
        )
        .expect_err("unstable");
        assert_eq!(error.code(), "UnstableStation");
    }

    #[test]
    fn infinite_server_has_no_wait() {
        let result = mm_infinity(
            &Q::from_i64(8),
            &Q::from_i64(10),
            &SolverOptions::default(),
            &NeverCancel,
        )
        .expect("stable");
        let station = &result.stations[0];
        assert_eq!(station.waiting_time, Q::zero());
        assert_eq!(station.queue_length, Q::zero());
        assert_eq!(
            station.occupancy,
            Q::from_i64(4).div(&Q::from_i64(5)).unwrap()
        );
        assert_eq!(
            station.response_time,
            Q::from_i64(1).div(&Q::from_i64(10)).unwrap()
        );
        assert_eq!(station.discipline, "delay");
    }

    #[test]
    fn zero_arrival_is_defined_and_degenerate() {
        let result = mm1(
            &Q::zero(),
            &Q::from_i64(10),
            &SolverOptions::default(),
            &NeverCancel,
        )
        .expect("zero arrival is defined");
        assert_eq!(
            result.provenance.status,
            crate::results::SolverStatus::Degenerate
        );
        assert_eq!(result.stations[0].throughput, Q::zero());
        assert_eq!(result.stations[0].occupancy, Q::zero());
        assert_eq!(
            result.stations[0].response_time,
            Q::from_i64(1).div(&Q::from_i64(10)).unwrap()
        );
    }

    #[test]
    fn invalid_rates_and_budgets_are_refused() {
        assert_eq!(
            mm1(
                &Q::from_i64(-1),
                &Q::from_i64(10),
                &SolverOptions::default(),
                &NeverCancel
            )
            .expect_err("negative arrival")
            .code(),
            "InvalidArrivalRate"
        );
        assert_eq!(
            mm1(
                &Q::from_i64(1),
                &Q::zero(),
                &SolverOptions::default(),
                &NeverCancel
            )
            .expect_err("zero service rate")
            .code(),
            "InvalidServiceRate"
        );
        let tiny = SolverOptions {
            max_rational_digits: 2,
            ..SolverOptions::default()
        };
        assert_eq!(
            mm1(&Q::from_i64(8), &Q::from_i64(10), &tiny, &NeverCancel)
                .expect_err("budget")
                .code(),
            "RationalBudgetExceeded"
        );
    }

    /// A deterministic randomized check of the M/M/c probability against the DIRECT Erlang C
    /// formula, computed independently here with a plain factorial sum. The solver uses the
    /// stable recurrence; this test is the oracle for it.
    #[test]
    fn randomized_erlang_c_matches_the_direct_formula() {
        let mut state = 0x5eed_1234_u64;
        let mut next = || {
            state = state
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            (state >> 33) as i64
        };
        let mut checked = 0;
        for servers in 2..=6usize {
            for _ in 0..8 {
                let lambda = (next() % 97) + 1;
                let mu = (next() % 23) + 1;
                if lambda >= (servers as i64) * mu {
                    continue;
                }
                let result = mmc(
                    &Q::from_i64(lambda),
                    &Q::from_i64(mu),
                    servers,
                    &SolverOptions::default(),
                    &NeverCancel,
                )
                .expect("stable by construction");
                let traffic = Q::from_i64(lambda)
                    .div(&Q::from_i64(mu))
                    .expect("positive service rate");
                let rho = traffic
                    .div(&Q::from_u64(servers as u64))
                    .expect("positive server count");
                let one = Q::one();
                let mut power = Q::one();
                let mut factorial = 1u64;
                let mut partial = Q::one();
                for k in 1..servers {
                    power = power.mul(&traffic);
                    factorial *= k as u64;
                    partial = partial.add(
                        &power
                            .div(&Q::from_u64(factorial))
                            .expect("positive factorial"),
                    );
                }
                let last_power = power.mul(&traffic);
                let last_factorial = factorial * servers as u64;
                let tail = last_power
                    .div(&Q::from_u64(last_factorial).mul(&one.sub(&rho)))
                    .expect("stable station has spare capacity");
                let direct = tail
                    .div(&partial.add(&tail))
                    .expect("positive normalization");
                assert_eq!(
                    result.stations[0].probability_waiting.as_ref(),
                    Some(&direct),
                    "c={servers} lambda={lambda} mu={mu}"
                );
                checked += 1;
            }
        }
        assert!(checked >= 20, "the randomized sweep checked only {checked}");
    }
}
