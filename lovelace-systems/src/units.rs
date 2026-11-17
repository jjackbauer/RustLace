//! Canonical units and dimension tags.
//!
//! The model stores canonical base units: **seconds** for time, **jobs per second** for a rate,
//! **seconds per job** for a service demand and a service time, **jobs** for a population, and
//! dimensionless rationals for utilization and probability. The typed wrappers below make an
//! incompatible sum a compile error in Rust; the language adapter documents that its arguments are
//! canonical seconds / jobs-per-second and applies no silent conversion.
//!
//! A conversion is explicit (`Time::minutes`) and exact: it multiplies or divides by 60 or 3600,
//! so a minute is `60` seconds, not a rounded decimal.

use crate::errors::SystemsError;
use crate::numeric::Q;

/// A time unit accepted by the conversion constructors.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TimeUnit {
    /// Seconds (the canonical unit).
    Second,
    /// Minutes (`60` seconds).
    Minute,
    /// Hours (`3600` seconds).
    Hour,
}

/// A rate unit accepted by the conversion constructors.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RateUnit {
    /// Jobs per second (the canonical unit).
    PerSecond,
    /// Jobs per minute (`1/60` per second).
    PerMinute,
    /// Jobs per hour (`1/3600` per second).
    PerHour,
}

/// A duration in canonical seconds.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Time(Q);

impl Time {
    /// A duration in seconds.
    pub fn seconds(value: Q) -> Time {
        Time(value)
    }

    /// A duration in the given unit, converted exactly to seconds.
    pub fn in_unit(value: Q, unit: TimeUnit) -> Time {
        let factor = match unit {
            TimeUnit::Second => Q::one(),
            TimeUnit::Minute => Q::from_u64(60),
            TimeUnit::Hour => Q::from_u64(3600),
        };
        Time(value.mul(&factor))
    }

    /// The canonical value in seconds.
    pub fn as_seconds(&self) -> &Q {
        &self.0
    }

    /// Consumes the wrapper, yielding canonical seconds.
    pub fn into_seconds(self) -> Q {
        self.0
    }
}

/// A throughput or arrival rate in canonical jobs per second.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rate(Q);

impl Rate {
    /// A rate in jobs per second.
    pub fn per_second(value: Q) -> Rate {
        Rate(value)
    }

    /// A rate in the given unit, converted exactly to jobs per second.
    pub fn in_unit(value: Q, unit: RateUnit) -> Rate {
        let divisor = match unit {
            RateUnit::PerSecond => Q::one(),
            RateUnit::PerMinute => Q::from_u64(60),
            RateUnit::PerHour => Q::from_u64(3600),
        };
        Rate(value.div(&divisor).unwrap_or_else(Q::zero))
    }

    /// The canonical value in jobs per second.
    pub fn as_per_second(&self) -> &Q {
        &self.0
    }

    /// Consumes the wrapper, yielding canonical jobs per second.
    pub fn into_per_second(self) -> Q {
        self.0
    }
}

/// A service demand (or service time per visit) in canonical seconds per job.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Demand(Q);

impl Demand {
    /// A demand in seconds per job.
    pub fn seconds_per_job(value: Q) -> Demand {
        Demand(value)
    }

    /// The canonical value.
    pub fn as_seconds_per_job(&self) -> &Q {
        &self.0
    }

    /// Consumes the wrapper.
    pub fn into_seconds_per_job(self) -> Q {
        self.0
    }
}

/// The dimension of a quantity, for documentation, diagnostics and the adapter's checks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dimension {
    /// Time (seconds).
    Time,
    /// A rate (jobs per second).
    Rate,
    /// A demand (seconds per job).
    Demand,
    /// A count of jobs.
    Jobs,
    /// A dimensionless ratio.
    Dimensionless,
    /// A dimensionless probability in `[0, 1]`.
    Probability,
}

impl Dimension {
    /// The dimension's stable label.
    pub fn as_str(self) -> &'static str {
        match self {
            Dimension::Time => "time",
            Dimension::Rate => "rate",
            Dimension::Demand => "demand",
            Dimension::Jobs => "jobs",
            Dimension::Dimensionless => "dimensionless",
            Dimension::Probability => "probability",
        }
    }
}

/// Checks that two dimensions can be summed; returns the stable refusal otherwise.
pub fn check_same_dimension(left: Dimension, right: Dimension) -> Result<(), SystemsError> {
    if left == right {
        Ok(())
    } else {
        Err(SystemsError::type_mismatch(&format!(
            "Cannot add a {} quantity to a {} quantity",
            right.as_str(),
            left.as_str()
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn minute_and_hour_conversions_are_exact() {
        assert_eq!(
            Time::in_unit(Q::from_u64(3), TimeUnit::Minute)
                .as_seconds()
                .to_i64(),
            Some(180)
        );
        assert_eq!(
            Time::in_unit(Q::from_u64(1), TimeUnit::Hour)
                .as_seconds()
                .to_i64(),
            Some(3600)
        );
        assert_eq!(
            Rate::in_unit(Q::from_u64(120), RateUnit::PerMinute)
                .as_per_second()
                .to_i64(),
            Some(2)
        );
        assert_eq!(
            Rate::in_unit(Q::from_u64(1), RateUnit::PerHour)
                .as_per_second()
                .to_decimal(6),
            "0.0002(7)"
        );
    }

    #[test]
    fn mixed_dimensions_are_refused_by_the_checker() {
        assert!(check_same_dimension(Dimension::Time, Dimension::Time).is_ok());
        let error = check_same_dimension(Dimension::Time, Dimension::Rate).expect_err("refused");
        assert_eq!(error.code(), "TypeMismatch");
        assert_eq!(error.category().as_str(), "TypeMismatch");
        assert_eq!(
            error.message(),
            "Cannot add a rate quantity to a time quantity."
        );
    }
}
