//! The two models from the crate page, as a program that can be run.
//!
//! Run with `cargo run -p lovelace-systems --example mmc_and_mva`.

use lovelace_systems::{
    ClosedWorkload, NeverCancel, Q, QueueNetwork, ServiceDiscipline, SolverOptions, Station,
    Workload, analyze, mmc,
};

fn main() {
    // M/M/2: arrivals at 8 per second, each of two servers serves 5 per second. Every reported
    // number is an exact rational, so the printed values are exact, not rounded.
    let mm2 = mmc(
        &Q::from_i64(8),
        &Q::from_i64(5),
        2,
        &SolverOptions::default(),
        &NeverCancel,
    )
    .expect("stable M/M/2");
    let probability_waiting = mm2.stations[0]
        .probability_waiting
        .as_ref()
        .expect("a stable M/M/2 publishes the Erlang C probability");
    assert_eq!(probability_waiting.to_string(), "32/45");
    println!("mmc(8, 5, 2) probability_waiting={probability_waiting}");

    // Closed: two customers, 0.09 s of think time, 0.01 s of service at one FCFS station.
    let service = Q::from_i64(1).div(&Q::from_i64(100)).expect("1/100");
    let closed = QueueNetwork::new(
        "interactive",
        Workload::Closed(ClosedWorkload {
            population: 2,
            think_time: Q::from_i64(9).div(&Q::from_i64(100)).expect("9/100"),
        }),
        vec![
            Station::new(
                "cpu",
                ServiceDiscipline::FcfsSingleServer,
                service,
                Q::one(),
            )
            .expect("valid station"),
        ],
    )
    .expect("valid model");
    let mva = analyze(&closed, &SolverOptions::default(), &NeverCancel).expect("solved");
    assert_eq!(mva.aggregate.throughput.to_string(), "2000/101");
    println!("mva throughput={}", mva.aggregate.throughput);
}
