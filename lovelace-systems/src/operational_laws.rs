//! Operational laws.
//!
//! These are definitions and exact checks, not approximations: Little's law, the utilization law,
//! the forced-flow law and the interactive response-time law. Each `check_*` returns a
//! [`LawCheck`] whose `holds` is the exact rational comparison, so a solver can publish its own
//! conservation evidence instead of asserting it in prose.

use crate::errors::SystemsError;
use crate::numeric::Q;
use crate::results::LawCheck;

/// Little's law: `L = lambda * W`.
pub fn little_law(throughput: &Q, response_time: &Q) -> Q {
    throughput.mul(response_time)
}

/// The utilization law: `U = X * D` for a single-server center.
pub fn utilization_law(throughput: &Q, service_demand: &Q) -> Q {
    throughput.mul(service_demand)
}

/// The forced-flow law: `X_i = V_i * X`.
pub fn forced_flow(visits: &Q, throughput: &Q) -> Q {
    visits.mul(throughput)
}

/// The interactive response-time law inverted for the response time: `R = N / X - Z`.
///
/// Requires a positive throughput, so a completely idle closed model is a [`SystemsError`] rather
/// than a division by zero.
pub fn interactive_response(
    population: u64,
    throughput: &Q,
    think_time: &Q,
) -> Result<Q, SystemsError> {
    if !throughput.is_positive() {
        return Err(SystemsError::degenerate_model(
            "the closed model has zero throughput, so R = N/X - Z is undefined",
        ));
    }
    let cycle = Q::from_u64(population)
        .div(throughput)
        .ok_or_else(|| SystemsError::degenerate_model("the closed model has zero throughput"))?;
    let response = cycle.sub(think_time);
    if response.is_negative() {
        return Err(SystemsError::degenerate_model(
            "N/X is below the think time, so the implied response time is negative",
        ));
    }
    Ok(response)
}

/// Exact check of Little's law.
pub fn check_little(occupancy: &Q, throughput: &Q, response_time: &Q) -> LawCheck {
    LawCheck::exact(
        "little",
        occupancy.clone(),
        little_law(throughput, response_time),
    )
}

/// Exact check of the utilization law.
pub fn check_utilization(utilization: &Q, throughput: &Q, service_demand: &Q) -> LawCheck {
    LawCheck::exact(
        "utilization",
        utilization.clone(),
        utilization_law(throughput, service_demand),
    )
}

/// Exact check of the forced-flow law.
pub fn check_forced_flow(station_throughput: &Q, visits: &Q, network_throughput: &Q) -> LawCheck {
    LawCheck::exact(
        "forced-flow",
        station_throughput.clone(),
        forced_flow(visits, network_throughput),
    )
}

/// Exact check of the interactive response-time law, `N = X * (R + Z)`.
pub fn check_interactive(
    population: u64,
    throughput: &Q,
    response_time: &Q,
    think_time: &Q,
) -> LawCheck {
    LawCheck::exact(
        "interactive-response",
        Q::from_u64(population),
        throughput.mul(&response_time.add(think_time)),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn q(num: i64, den: u64) -> Q {
        Q::from_parts(
            lovelace_int::Int::from_i64(num),
            lovelace_nat::Nat::from_u64(den),
        )
        .expect("denominator")
    }

    #[test]
    fn little_and_utilization_laws_are_exact() {
        assert_eq!(little_law(&Q::from_i64(8), &q(1, 2)), Q::from_i64(4));
        assert_eq!(utilization_law(&Q::from_i64(8), &q(1, 20)), q(2, 5));
        assert!(check_little(&Q::from_i64(4), &Q::from_i64(8), &q(1, 2)).holds);
        assert!(!check_little(&Q::from_i64(5), &Q::from_i64(8), &q(1, 2)).holds);
    }

    #[test]
    fn forced_flow_multiplies_the_visit_ratio() {
        assert_eq!(
            forced_flow(&Q::from_u64(2), &Q::from_i64(8)),
            Q::from_i64(16)
        );
        assert!(check_forced_flow(&Q::from_i64(16), &Q::from_u64(2), &Q::from_i64(8)).holds);
    }

    #[test]
    fn interactive_response_inverts_the_law() {
        // N = 2, X = 60, Z = 0.02 -> R = 2/60 - 0.02 = 0.013333...
        let response = interactive_response(2, &Q::from_i64(60), &q(1, 50)).expect("defined");
        assert_eq!(response, q(1, 75));
        assert!(check_interactive(2, &Q::from_i64(60), &response, &q(1, 50)).holds);
        assert_eq!(
            interactive_response(2, &Q::zero(), &Q::zero())
                .expect_err("zero throughput")
                .code(),
            "DegenerateModel"
        );
    }
}
