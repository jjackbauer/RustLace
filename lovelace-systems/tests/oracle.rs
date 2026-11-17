//! The independent Python systems oracle (RNL-CI-006).
//!
//! `tools/systems-oracle.py` re-derives M/M/1, Erlang C, MVA and open-Jackson metrics from their
//! textbook definitions over `fractions.Fraction`. The committed fixture is replayed here against
//! the production models; no expected value is produced by the Rust formulas themselves.

use std::collections::BTreeMap;

use lovelace_int::Int;
use lovelace_nat::Nat;
use lovelace_systems::{
    ClosedWorkload, NeverCancel, OpenWorkload, Q, QueueNetwork, ServiceDiscipline, SolverOptions,
    Station, Workload, analyze_closed, analyze_open, mm1, mmc,
};

const FIXTURE: &str = include_str!("../../parity/systems/oracle.tsv");

fn parse_rat(text: &str) -> Q {
    let (numerator, denominator) = text.split_once('/').expect("numerator/denominator");
    Q::from_parts(
        Int::from_decimal(numerator).expect("a decimal numerator"),
        Nat::from_decimal(denominator).expect("a decimal denominator"),
    )
    .expect("the oracle never emits a zero denominator")
}

fn oracle() -> BTreeMap<(String, String), Q> {
    let mut out = BTreeMap::new();
    for (line_number, line) in FIXTURE.lines().enumerate() {
        if line.trim().is_empty() || line.starts_with('#') {
            continue;
        }
        let parts: Vec<&str> = line.split('\t').collect();
        assert_eq!(
            parts.len(),
            3,
            "line {}: expected case/metric/value, got {line:?}",
            line_number + 1
        );
        out.insert(
            (parts[0].to_string(), parts[1].to_string()),
            parse_rat(parts[2]),
        );
    }
    assert!(
        out.len() >= 20,
        "the systems oracle fixture is non-vacuous: only {} metrics",
        out.len()
    );
    out
}

fn ratio(numerator: i64, denominator: u64) -> Q {
    Q::from_parts(Int::from_i64(numerator), Nat::from_u64(denominator))
        .expect("non-zero denominator")
}

fn station(id: &str, discipline: ServiceDiscipline, service_time: Q, visits: Q) -> Station {
    Station::new(id, discipline, service_time, visits).expect("valid station")
}

fn assert_metric(oracle: &BTreeMap<(String, String), Q>, case: &str, metric: &str, actual: &Q) {
    let expected = oracle
        .get(&(case.to_string(), metric.to_string()))
        .unwrap_or_else(|| panic!("the oracle has no {case}/{metric}"));
    assert_eq!(actual, expected, "{case}/{metric}");
}

#[test]
fn mm1_and_erlang_c_match_the_independent_oracle() {
    let oracle = oracle();
    let options = SolverOptions::default();

    let single = mm1(&ratio(8, 1), &ratio(10, 1), &options, &NeverCancel).expect("stable");
    let station = &single.stations[0];
    assert_metric(&oracle, "mm1_8_10", "utilization", &station.utilization);
    assert_metric(&oracle, "mm1_8_10", "response_time", &station.response_time);
    assert_metric(&oracle, "mm1_8_10", "waiting_time", &station.waiting_time);
    assert_metric(&oracle, "mm1_8_10", "occupancy", &station.occupancy);
    assert_metric(&oracle, "mm1_8_10", "queue_length", &station.queue_length);
    assert_metric(&oracle, "mm1_8_10", "throughput", &station.throughput);

    let multi = mmc(&ratio(8, 1), &ratio(5, 1), 2, &options, &NeverCancel).expect("stable");
    let station = &multi.stations[0];
    assert_metric(
        &oracle,
        "mmc_8_5_2",
        "probability_waiting",
        station.probability_waiting.as_ref().expect("defined"),
    );
    assert_metric(&oracle, "mmc_8_5_2", "waiting_time", &station.waiting_time);
    assert_metric(
        &oracle,
        "mmc_8_5_2",
        "response_time",
        &station.response_time,
    );
    assert_metric(&oracle, "mmc_8_5_2", "utilization", &station.utilization);
    assert_metric(&oracle, "mmc_8_5_2", "occupancy", &station.occupancy);
    assert_metric(&oracle, "mmc_8_5_2", "queue_length", &station.queue_length);
}

#[test]
fn closed_mva_matches_the_independent_oracle() {
    let oracle = oracle();
    let network = QueueNetwork::new(
        "oracle-mva",
        Workload::Closed(ClosedWorkload {
            population: 4,
            think_time: ratio(1, 10),
        }),
        vec![
            station(
                "cpu",
                ServiceDiscipline::FcfsSingleServer,
                ratio(1, 100),
                Q::one(),
            ),
            station(
                "disk",
                ServiceDiscipline::FcfsSingleServer,
                ratio(1, 25),
                Q::one(),
            ),
            station("net", ServiceDiscipline::Delay, ratio(1, 500), Q::one()),
        ],
    )
    .expect("valid closed network");
    let result = analyze_closed(&network, &SolverOptions::default(), &NeverCancel).expect("solved");
    assert_metric(
        &oracle,
        "mva_closed",
        "throughput",
        &result.aggregate.throughput,
    );
    assert_metric(
        &oracle,
        "mva_closed",
        "response_time",
        &result.aggregate.response_time,
    );
    assert_metric(
        &oracle,
        "mva_closed",
        "occupancy",
        &result.aggregate.occupancy,
    );
    for (index, id) in ["cpu", "disk", "net"].iter().enumerate() {
        let station = result.station(id).expect("station");
        assert_metric(
            &oracle,
            "mva_closed",
            &format!("occupancy_station_{index}"),
            &station.occupancy,
        );
    }
    // The independent recurrence closes the closed-model identity too.
    let think = ratio(1, 10);
    assert_eq!(
        result.aggregate.occupancy,
        Q::from_i64(4).sub(&result.aggregate.throughput.mul(&think))
    );
}

#[test]
fn open_jackson_matches_the_independent_oracle() {
    let oracle = oracle();
    let network = QueueNetwork::new(
        "oracle-jackson",
        Workload::Open(OpenWorkload {
            arrival_rate: ratio(20, 1),
        }),
        vec![
            station(
                "api",
                ServiceDiscipline::FcfsSingleServer,
                ratio(1, 50),
                Q::one(),
            ),
            station(
                "db",
                ServiceDiscipline::FcfsSingleServer,
                ratio(1, 100),
                ratio(2, 1),
            ),
            station(
                "search",
                ServiceDiscipline::FcfsSingleServer,
                ratio(1, 200),
                ratio(3, 1),
            ),
            station("cache", ServiceDiscipline::Delay, ratio(1, 20), Q::one()),
        ],
    )
    .expect("valid open network");
    let result = analyze_open(&network, &SolverOptions::default(), &NeverCancel).expect("solved");
    assert_metric(
        &oracle,
        "jackson_open",
        "response_time",
        &result.aggregate.response_time,
    );
    assert_metric(
        &oracle,
        "jackson_open",
        "occupancy",
        &result.aggregate.occupancy,
    );
    assert_metric(
        &oracle,
        "jackson_open",
        "queue_length",
        &result.aggregate.queue_length,
    );
    for id in ["api", "db", "search"] {
        let station = result.station(id).expect("station");
        assert_metric(
            &oracle,
            "jackson_open",
            &format!("utilization_{id}"),
            &station.utilization,
        );
    }
}
