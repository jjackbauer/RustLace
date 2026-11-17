//! Model validation.
//!
//! Validation runs before any solver and returns the first stable refusal. The rules encode the P0
//! model vocabulary: identifiers are unique, a network is non-empty, a closed population is any
//! `u64`, an open arrival rate is non-negative (zero is a defined, empty arrival stream), a station
//! that carries traffic has a finite positive service time, and a multi-server discipline declares
//! at least two servers. Solver-specific assumptions (stability, discipline support) are checked by
//! the solver that relies on them and name the failing assumption.

use crate::errors::SystemsError;
use crate::model::{QueueNetwork, ServiceDiscipline, Workload};
use crate::numeric::Q;

/// The largest server count accepted for one station.
pub const MAX_SERVERS: usize = 1_000_000;

/// Validates the model vocabulary independent of the chosen solver.
pub fn validate(network: &QueueNetwork) -> Result<(), SystemsError> {
    if network.stations.is_empty() {
        return Err(SystemsError::empty_network());
    }
    for (index, station) in network.stations.iter().enumerate() {
        if network.stations[..index]
            .iter()
            .any(|earlier| earlier.id == station.id)
        {
            return Err(SystemsError::duplicate_identifier(station.id.as_str()));
        }
        if station.visits.is_negative() {
            return Err(SystemsError::invalid_visits(
                station.id.as_str(),
                &station.visits.to_string(),
            ));
        }
        if station.visits.is_positive() && !station.service_time.is_positive() {
            return Err(SystemsError::invalid_service_time(
                station.id.as_str(),
                &station.service_time.to_string(),
            ));
        }
        if station.service_time.is_negative() {
            return Err(SystemsError::invalid_service_time(
                station.id.as_str(),
                &station.service_time.to_string(),
            ));
        }
        match station.discipline {
            ServiceDiscipline::FcfsSingleServer | ServiceDiscipline::Delay => {}
            ServiceDiscipline::MultiServer { servers } => {
                if !(2..=MAX_SERVERS).contains(&servers) {
                    return Err(SystemsError::unsupported_discipline(
                        station.id.as_str(),
                        &format!("M/M/{servers} (the server count must be 2 to {MAX_SERVERS})"),
                    ));
                }
            }
        }
    }
    match &network.workload {
        Workload::Open(open) => {
            if open.arrival_rate.is_negative() {
                return Err(SystemsError::invalid_arrival_rate(
                    &open.arrival_rate.to_string(),
                ));
            }
        }
        Workload::Closed(closed) => {
            if closed.think_time.is_negative() {
                return Err(SystemsError::degenerate_model("the think time is negative"));
            }
        }
    }
    Ok(())
}

/// True when every station has zero visits (no traffic reaches any center).
pub fn is_empty_traffic(network: &QueueNetwork) -> bool {
    network
        .stations
        .iter()
        .all(|station| station.visits.is_zero())
}

/// The offered utilization `U = X * S / c` at one station, or `None` for a delay center.
pub fn station_utilization(service_time: &Q, throughput: &Q, servers: Option<usize>) -> Option<Q> {
    let servers = servers?;
    throughput
        .mul(service_time)
        .div(&Q::from_u64(servers as u64))
}

/// Refuses a finite-capacity station whose offered utilization is not below 1.
pub fn check_stable(
    id: &str,
    service_time: &Q,
    throughput: &Q,
    servers: Option<usize>,
) -> Result<(), SystemsError> {
    let Some(utilization) = station_utilization(service_time, throughput, servers) else {
        return Ok(());
    };
    if utilization < Q::one() {
        Ok(())
    } else {
        Err(SystemsError::unstable_station(id, &utilization.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{ClosedWorkload, OpenWorkload, Station};

    fn station(id: &str, discipline: ServiceDiscipline, service_time: Q, visits: Q) -> Station {
        Station::new(id, discipline, service_time, visits).expect("valid station")
    }

    #[test]
    fn duplicate_ids_are_refused() {
        let error = QueueNetwork::new(
            "duplicate",
            Workload::Closed(ClosedWorkload {
                population: 1,
                think_time: Q::zero(),
            }),
            vec![
                station(
                    "api",
                    ServiceDiscipline::FcfsSingleServer,
                    Q::one(),
                    Q::one(),
                ),
                station("api", ServiceDiscipline::Delay, Q::one(), Q::one()),
            ],
        )
        .expect_err("duplicate ids");
        assert_eq!(error.code(), "DuplicateStationId");
        assert_eq!(
            error.message(),
            "Station identifier 'api' is declared more than once."
        );
    }

    #[test]
    fn an_empty_network_and_negative_visits_are_refused() {
        let error = QueueNetwork::new(
            "empty",
            Workload::Open(OpenWorkload {
                arrival_rate: Q::one(),
            }),
            Vec::new(),
        )
        .expect_err("empty");
        assert_eq!(error.code(), "EmptyNetwork");

        let error = QueueNetwork::new(
            "negative",
            Workload::Open(OpenWorkload {
                arrival_rate: Q::one(),
            }),
            vec![station(
                "one",
                ServiceDiscipline::FcfsSingleServer,
                Q::one(),
                Q::from_i64(-1),
            )],
        )
        .expect_err("negative visits");
        assert_eq!(error.code(), "InvalidVisits");
    }

    #[test]
    fn a_traffic_carrying_station_needs_positive_service_time() {
        let error = QueueNetwork::new(
            "zero-service",
            Workload::Open(OpenWorkload {
                arrival_rate: Q::one(),
            }),
            vec![station(
                "one",
                ServiceDiscipline::FcfsSingleServer,
                Q::zero(),
                Q::one(),
            )],
        )
        .expect_err("zero service time");
        assert_eq!(error.code(), "InvalidServiceTime");

        // No visits means no traffic, so a zero service time is a defined, empty station.
        let network = QueueNetwork::new(
            "unused",
            Workload::Open(OpenWorkload {
                arrival_rate: Q::one(),
            }),
            vec![station(
                "unused",
                ServiceDiscipline::FcfsSingleServer,
                Q::zero(),
                Q::zero(),
            )],
        )
        .expect("a station with no visits is allowed");
        assert!(is_empty_traffic(&network));
    }

    #[test]
    fn one_server_multiserver_and_utilization_checks() {
        let error = QueueNetwork::new(
            "one-server",
            Workload::Open(OpenWorkload {
                arrival_rate: Q::one(),
            }),
            vec![station(
                "one",
                ServiceDiscipline::MultiServer { servers: 1 },
                Q::one(),
                Q::one(),
            )],
        )
        .expect_err("multi-server with one server");
        assert_eq!(error.code(), "UnsupportedDiscipline");

        assert!(check_stable("a", &Q::one(), &Q::from_i64(1), Some(1)).is_err());
        assert!(check_stable("a", &Q::one(), &Q::from_i64(1), Some(2)).is_ok());
        assert!(check_stable("d", &Q::one(), &Q::from_i64(100), None).is_ok());
        assert_eq!(
            station_utilization(&Q::one(), &Q::from_i64(4), Some(2)),
            Some(Q::from_i64(2))
        );
    }
}
