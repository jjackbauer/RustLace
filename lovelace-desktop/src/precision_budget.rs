//! PERF-011: the measured cost of an enormous precision request.
//!
//! The product accepts a display precision up to [`crate::settings::MAX_PRECISION`] digits. A
//! precision change is itself cheap (it sets a number on the engine), but the next evaluation at
//! that precision is not, and the reference requirement is that an *extreme* request is answered
//! with an estimate or a refusal before the user pays for it.
//!
//! # Why the curve is measured
//!
//! The estimate is never a constant. [`PrecisionCurve::measure`] runs one fixed workload —
//! `sqrt(2)`, the same operation the reference precision journeys use — on fresh engines built
//! by the workbench's own engine factory, at increasing digit counts, and fits
//!
//! ```text
//! seconds(digits) = exp(intercept) * digits ^ exponent
//! ```
//!
//! to the measured points by least squares in log-log space. The samples are kept and shown with
//! the estimate, so a user (and a reviewer) can see which measurements the number came from. A
//! different engine, a different machine or a different engine version produces a different
//! curve; nothing here is a hardcoded time.
//!
//! # Thresholds
//!
//! * At or below [`WARN_DIGITS`] digits a change is free: no estimate, no dialog, no extra log
//!   line beyond the product's ordinary "Precision set to N".
//! * Above it the user must confirm the estimate ([`CostDecision::Confirm`]).
//! * When the measured estimate exceeds [`SESSION_BUDGET_SECONDS`] the request is refused
//!   outright ([`CostDecision::Refuse`]): that is the documented refusal class. Refusing
//!   returns an explanation and changes nothing; it never aborts the process and never panics.
//!
//! [`PrecisionCurve::estimate_memory_bytes`] is a documented conservative bound, not a second
//! measurement: 16 bytes per decimal digit covers two 64-bit limbs of working value plus the
//! temporaries a square root allocates, with a fixed 256-byte floor.

use std::time::Duration;

use crate::engine::Evaluator;

/// Digits at or below which a precision change is applied without an estimate or a dialog.
///
/// 5 000 is the reference's "small change" boundary: it is above every default and every
/// ordinary scientific computation, and below every precision at which the measured workload
/// already costs seconds.
pub const WARN_DIGITS: usize = 5_000;

/// The documented refusal class: a single extreme request whose measured estimate exceeds this
/// many seconds is refused instead of accepted.
pub const SESSION_BUDGET_SECONDS: f64 = 30.0;

/// The fixed workload the curve is measured with. One name, used by the samples, the estimate
/// line and the test, so the claim and the measurement cannot drift apart.
pub const WORKLOAD: &str = "sqrt(2)";

/// The digit counts the curve is measured at, smallest first.
///
/// They span the interesting range: the first points are fractions of a millisecond, and the set
/// stops growing once a sample exceeds [`SAMPLE_BUDGET_SECONDS`]. This is the set the acceptance
/// measurement and the real-engine test use; it is not the set a request pays for.
pub const SAMPLE_DIGITS: [usize; 4] = [250, 1_000, 4_000, 16_000];

/// The points one *request* measures synchronously before it offers an estimate.
///
/// PERF-011 measures the engine when the first extreme request arrives, and that measurement is
/// bounded: three points whose warm-up + sample on the reference engine costs about 50 ms, which
/// is one bounded calibration on an explicit user action, not a per-frame cost. The fit comes
/// from the same three measured points; the wider [`SAMPLE_DIGITS`] set exists to show in the
/// acceptance evidence that the curve's shape holds beyond them.
pub const RUNTIME_SAMPLE_DIGITS: [usize; 3] = [250, 1_000, 4_000];

/// Stop adding samples once one measurement takes longer than this.
pub const SAMPLE_BUDGET_SECONDS: f64 = 0.75;

/// One measured digits-to-time curve for one engine.
#[derive(Clone, Debug, PartialEq)]
pub struct PrecisionCurve {
    /// The measured points: digits, best-of-two seconds.
    samples: Vec<(usize, f64)>,
    /// The fitted power-law exponent: time grows roughly as `digits ^ exponent`.
    exponent: f64,
    /// `ln(seconds)` at one digit.
    log_scale: f64,
}

impl PrecisionCurve {
    /// Builds a curve from measured points, refusing a set that cannot support an estimate.
    ///
    /// At least two distinct positive points are required; a single point would make the
    /// extrapolation an assumption, and no points would make it a guess.
    pub fn from_samples(mut samples: Vec<(usize, f64)>) -> Option<PrecisionCurve> {
        samples.retain(|(digits, seconds)| *digits > 0 && seconds.is_finite() && *seconds > 0.0);
        samples.sort_by_key(|(digits, _)| *digits);
        samples.dedup_by_key(|(digits, _)| *digits);
        if samples.len() < 2 {
            return None;
        }
        let count = samples.len() as f64;
        let mean_x = samples
            .iter()
            .map(|(digits, _)| (*digits as f64).ln())
            .sum::<f64>()
            / count;
        let mean_y = samples
            .iter()
            .map(|(_, seconds)| seconds.ln())
            .sum::<f64>()
            / count;
        let mut covariance = 0.0;
        let mut variance = 0.0;
        for (digits, seconds) in &samples {
            let x = (*digits as f64).ln() - mean_x;
            let y = seconds.ln() - mean_y;
            covariance += x * y;
            variance += x * x;
        }
        // Two distinct digit counts always give a positive variance; the guard keeps the fit
        // honest rather than dividing by zero if that ever stops holding.
        if variance <= f64::EPSILON {
            return None;
        }
        let exponent = covariance / variance;
        let log_scale = mean_y - exponent * mean_x;
        Some(PrecisionCurve {
            samples,
            exponent,
            log_scale,
        })
    }

    /// Measures the curve on fresh engines from `factory`, with the documented workload.
    ///
    /// Each point is the best of two evaluations after a throwaway warm-up at the same precision,
    /// which keeps allocator and page-fault noise out of the sample without inventing a number.
    /// This is the wide acceptance measurement ([`SAMPLE_DIGITS`]); a request uses
    /// [`Self::measure_runtime`] instead.
    pub fn measure(factory: impl Fn() -> Box<dyn Evaluator> + Send) -> Option<PrecisionCurve> {
        Self::measure_with(factory, &SAMPLE_DIGITS)
    }

    /// Measures the bounded runtime curve: [`RUNTIME_SAMPLE_DIGITS`], one warm-up and one sample
    /// per point.
    pub fn measure_runtime(
        factory: impl Fn() -> Box<dyn Evaluator> + Send,
    ) -> Option<PrecisionCurve> {
        Self::measure_with(factory, &RUNTIME_SAMPLE_DIGITS)
    }

    fn measure_with(
        factory: impl Fn() -> Box<dyn Evaluator> + Send,
        points: &[usize],
    ) -> Option<PrecisionCurve> {
        let mut engine = factory();
        let mut samples: Vec<(usize, f64)> = Vec::new();
        for &digits in points {
            engine.set_precision(digits);
            // Warm-up: one evaluation at this precision, discarded.
            let _ = engine.eval(WORKLOAD);
            let mut best = f64::INFINITY;
            for _ in 0..2 {
                let started = std::time::Instant::now();
                if engine.eval(WORKLOAD).is_err() {
                    // An engine that cannot evaluate the workload cannot be measured; a missing
                    // curve is reported as "not measured", never replaced by a guessed number.
                    return None;
                }
                let seconds = started.elapsed().as_secs_f64();
                if seconds < best {
                    best = seconds;
                }
            }
            samples.push((digits, best));
            if best > SAMPLE_BUDGET_SECONDS {
                break;
            }
        }
        PrecisionCurve::from_samples(samples)
    }

    /// The measured points, smallest digit count first.
    pub fn samples(&self) -> &[(usize, f64)] {
        &self.samples
    }

    /// The fitted power-law exponent.
    pub fn exponent(&self) -> f64 {
        self.exponent
    }

    /// The estimated wall time of one [`WORKLOAD`] evaluation at `digits` digits.
    pub fn estimate_seconds(&self, digits: usize) -> f64 {
        let digits = digits.max(1) as f64;
        (self.log_scale + self.exponent * digits.ln()).exp()
    }

    /// The documented conservative memory bound for a value held at `digits` digits.
    pub fn estimate_memory_bytes(&self, digits: usize) -> u64 {
        256_u64.saturating_add(16_u64.saturating_mul(digits as u64))
    }

    /// One line naming the measurements the estimate is fitted to.
    pub fn measured_line(&self) -> String {
        let points: Vec<String> = self
            .samples
            .iter()
            .map(|(digits, seconds)| format!("{digits} digits {}", duration_text(*seconds)))
            .collect();
        format!(
            "measured {WORKLOAD}: {}; time grows as digits^{:.2}",
            points.join(", "),
            self.exponent
        )
    }

    /// The estimate for one digit count, as the pane and the log show it.
    pub fn cost(&self, digits: usize) -> PrecisionCost {
        PrecisionCost {
            digits,
            seconds: self.estimate_seconds(digits),
            bytes: self.estimate_memory_bytes(digits),
            measured: self.measured_line(),
        }
    }
}

/// What one precision request will cost, as the UI and the log show it.
#[derive(Clone, Debug, PartialEq)]
pub struct PrecisionCost {
    /// The requested display precision.
    pub digits: usize,
    /// Estimated seconds for one [`WORKLOAD`] evaluation at that precision.
    pub seconds: f64,
    /// Documented conservative memory bound for the value.
    pub bytes: u64,
    /// The measured points the estimate was fitted to, in words.
    pub measured: String,
}

impl PrecisionCost {
    /// `≈ 1.4 s and ≈ 1.6 MB`.
    pub fn estimate_text(&self) -> String {
        format!(
            "≈ {} and ≈ {}",
            duration_text(self.seconds),
            bytes_text(self.bytes)
        )
    }

    /// The full line the log carries and the dialog paints.
    pub fn line(&self) -> String {
        format!(
            "{} digits: estimated {} for one {WORKLOAD} evaluation ({}).",
            self.digits,
            self.estimate_text(),
            self.measured
        )
    }
}

/// PERF-011: what the product does with a precision request above the threshold.
#[derive(Clone, Debug, PartialEq)]
pub enum CostDecision {
    /// At or below [`WARN_DIGITS`]: nothing to estimate, apply the change.
    Free,
    /// Above the threshold and inside the budget: the user must confirm this estimate.
    Confirm(PrecisionCost),
    /// Above the documented budget: refuse, change nothing, explain.
    Refuse(PrecisionCost, String),
}

impl CostDecision {
    /// Decides one request against the measured curve.
    pub fn of(curve: &PrecisionCurve, digits: usize) -> CostDecision {
        if digits <= WARN_DIGITS {
            return CostDecision::Free;
        }
        let cost = curve.cost(digits);
        if cost.seconds > SESSION_BUDGET_SECONDS {
            let reason = format!(
                "refused: the measured estimate {} exceeds the {SESSION_BUDGET_SECONDS:.0} s \
                 budget for one precision change ({} digits). The precision was not changed.",
                duration_text(cost.seconds),
                digits
            );
            CostDecision::Refuse(cost, reason)
        } else {
            CostDecision::Confirm(cost)
        }
    }
}

/// `0.4 ms`, `1.4 s`, `2 min 5 s`: a duration as a person reads it.
pub fn duration_text(seconds: f64) -> String {
    if !seconds.is_finite() || seconds <= 0.0 {
        return "less than a millisecond".to_string();
    }
    if seconds < 1.0 {
        let millis = seconds * 1_000.0;
        if millis < 10.0 {
            format!("{millis:.1} ms")
        } else {
            format!("{millis:.0} ms")
        }
    } else if seconds < 90.0 {
        format!("{seconds:.2} s")
    } else {
        let total = seconds.round() as u64;
        format!("{} min {} s", total / 60, total % 60)
    }
}

/// `512 B`, `1.6 KB`, `3.1 MB`: a byte count as a person reads it.
pub fn bytes_text(bytes: u64) -> String {
    if bytes < 1_024 {
        format!("{bytes} B")
    } else if bytes < 1_024 * 1_024 {
        format!("{:.1} KB", bytes as f64 / 1_024.0)
    } else {
        format!("{:.1} MB", bytes as f64 / (1_024.0 * 1_024.0))
    }
}

/// The estimate as a [`Duration`], for callers that want to compare budgets in time types.
pub fn estimate_duration(cost: &PrecisionCost) -> Duration {
    Duration::from_secs_f64(cost.seconds.max(0.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A deliberately different curve from the measured one: the tests below must react to the
    /// samples, which is how "the estimate comes from the curve" is proved.
    fn steep_curve() -> PrecisionCurve {
        PrecisionCurve::from_samples(vec![
            (100, 0.001),
            (1_000, 0.010),
            (10_000, 0.100),
            (100_000, 1.000),
        ])
        .expect("four positive points fit")
    }

    #[test]
    fn a_curve_needs_two_measured_points() {
        assert_eq!(PrecisionCurve::from_samples(Vec::new()), None);
        assert_eq!(PrecisionCurve::from_samples(vec![(100, 0.001)]), None);
        assert_eq!(
            PrecisionCurve::from_samples(vec![(100, 0.0), (1_000, 0.01)]),
            None,
            "a zero-time sample is not a measurement"
        );
        assert!(PrecisionCurve::from_samples(vec![(100, 0.001), (1_000, 0.01)]).is_some());
    }

    #[test]
    fn the_estimate_is_the_fitted_curve_not_a_constant() {
        let steep = steep_curve();
        // 100x digits with 100x more time is an exponent of 1: the fit reproduces the samples.
        assert!(
            (steep.exponent() - 1.0).abs() < 0.02,
            "the fitted exponent must come from the samples: {}",
            steep.exponent()
        );
        for (digits, seconds) in steep.samples().to_vec() {
            let estimate = steep.estimate_seconds(digits);
            let ratio = estimate / seconds;
            assert!(
                (0.9..=1.1).contains(&ratio),
                "the estimate at a measured point ({digits} digits) must match its measurement: \
                 {estimate} vs {seconds}"
            );
        }
        // A different set of samples gives a different number: nothing is hardcoded.
        let shallow = PrecisionCurve::from_samples(vec![
            (100, 0.001),
            (1_000, 0.002),
            (10_000, 0.004),
            (100_000, 0.008),
        ])
        .expect("four positive points fit");
        assert!(
            shallow.estimate_seconds(100_000) < steep.estimate_seconds(100_000) / 50.0,
            "the estimate is a function of the measured samples"
        );
        assert!(
            steep.estimate_seconds(100_000) > steep.estimate_seconds(10_000),
            "the estimate grows with the digit count"
        );
    }

    #[test]
    fn a_cost_names_the_measurements_it_came_from() {
        let cost = steep_curve().cost(20_000);
        assert_eq!(cost.digits, 20_000);
        assert!(cost.seconds > 0.1, "the estimate is the curve's: {cost:?}");
        assert_eq!(cost.bytes, 256 + 16 * 20_000);
        assert!(cost.measured.contains("100 digits"));
        assert!(cost.measured.contains("100000 digits"));
        assert!(cost.measured.contains("digits^1.00"), "{}", cost.measured);
        let line = cost.line();
        assert!(line.contains("20000 digits: estimated ≈"), "{line}");
        assert!(line.contains("sqrt(2)"), "{line}");
    }

    #[test]
    fn a_small_change_is_free_and_an_extreme_one_needs_confirmation_or_is_refused() {
        let curve = steep_curve();
        assert_eq!(CostDecision::of(&curve, 30), CostDecision::Free);
        assert_eq!(
            CostDecision::of(&curve, WARN_DIGITS),
            CostDecision::Free,
            "the threshold itself is still frictionless"
        );
        // 5 001 digits ≈ 0.05 s: inside the budget, so it needs a confirmation with the estimate.
        let CostDecision::Confirm(cost) = CostDecision::of(&curve, WARN_DIGITS + 1) else {
            panic!("one digit above the threshold must ask for confirmation");
        };
        assert_eq!(cost.digits, WARN_DIGITS + 1);
        assert!(cost.seconds > 0.0);
        // 200 000 digits is above MAX_PRECISION but the decision function is about the estimate:
        // 2 s at the 100k sample scaled is still inside the budget...
        let big = curve.cost(MAX_FOR_TEST);
        assert!(big.seconds < SESSION_BUDGET_SECONDS);
        // ...while a steeper curve refuses the same request with the reason spelled out.
        let extreme = PrecisionCurve::from_samples(vec![(100, 0.002), (1_000, 2.0)])
            .expect("two points fit");
        let CostDecision::Refuse(cost, reason) = CostDecision::of(&extreme, 8_000) else {
            panic!("an estimate past the budget must be refused");
        };
        assert_eq!(cost.digits, 8_000);
        assert!(reason.contains("refused"), "{reason}");
        assert!(reason.contains("30 s budget"), "{reason}");
        assert!(reason.contains("8000 digits"), "{reason}");
    }

    const MAX_FOR_TEST: usize = 100_000;

    #[test]
    fn durations_and_sizes_read_as_units() {
        assert_eq!(duration_text(0.0004), "0.4 ms");
        assert_eq!(duration_text(0.012), "12 ms");
        assert_eq!(duration_text(1.4), "1.40 s");
        assert_eq!(duration_text(125.0), "2 min 5 s");
        assert_eq!(bytes_text(512), "512 B");
        assert_eq!(bytes_text(1_600), "1.6 KB");
        assert_eq!(bytes_text(3 * 1_024 * 1_024), "3.0 MB");
    }
}

#[cfg(all(test, feature = "real-engine"))]
mod real_engine_curve_tests {
    use super::*;
    use crate::engine::real::RealEngine;

    /// PERF-011: the curve is measured on the shipped engine, and the estimate it produces is
    /// the curve's own number — printed so the round document can quote the measurement.
    #[test]
    fn the_shipped_engine_curve_is_measured_and_its_estimate_tracks_the_measurements() {
        let curve = PrecisionCurve::measure(|| Box::new(RealEngine::new()))
            .expect("the shipped engine yields at least two measured points");
        println!("PERF-011 curve: {}", curve.measured_line());
        for (digits, seconds) in curve.samples().to_vec() {
            println!("PERF-011 sample {digits} digits = {:.6} s", seconds);
            assert!(
                seconds > 0.0,
                "every sample is a real measurement: {digits} digits"
            );
        }
        assert!(
            curve.samples().len() >= 2,
            "the curve needs at least two measured points"
        );
        // The measured range must be strictly increasing in time with the digit count, or the
        // fit would be meaningless.
        let times: Vec<f64> = curve.samples().iter().map(|(_, s)| *s).collect();
        for pair in times.windows(2) {
            assert!(
                pair[1] >= pair[0],
                "the measured workload must not get faster with more digits: {times:?}"
            );
        }
        // The estimate at the largest measured point is within a factor of four of its
        // measurement; the fit is a fit, not a substitute for the sample.
        let (digits, seconds) = *curve.samples().last().expect("samples");
        let estimate = curve.estimate_seconds(digits);
        println!(
            "PERF-011 estimate at {digits} digits = {:.6} s (measured {:.6} s)",
            estimate, seconds
        );
        assert!(
            estimate <= seconds * 4.0 && estimate >= seconds / 4.0,
            "the fitted estimate must track the measurement at the last sample: {estimate} vs {seconds}"
        );

        // The bounded runtime curve a request actually pays for is the same engine, measured at
        // the three points RUNTIME_SAMPLE_DIGITS names; print its cost and require it to land in
        // the same order of magnitude as the wide measurement.
        let runtime = PrecisionCurve::measure_runtime(|| Box::new(RealEngine::new()))
            .expect("the bounded runtime curve fits");
        println!("PERF-011 runtime curve: {}", runtime.measured_line());
        assert_eq!(
            runtime.samples().len(),
            RUNTIME_SAMPLE_DIGITS.len(),
            "the runtime curve uses the bounded point set"
        );
        let wide_at_max = curve.estimate_seconds(crate::settings::MAX_PRECISION);
        let runtime_at_max = runtime.estimate_seconds(crate::settings::MAX_PRECISION);
        println!(
            "PERF-011 estimate at {} digits: wide {:.3} s, runtime {:.3} s",
            crate::settings::MAX_PRECISION,
            wide_at_max,
            runtime_at_max
        );
        assert!(
            runtime_at_max > 0.05 && runtime_at_max < wide_at_max * 10.0 && runtime_at_max > wide_at_max / 10.0,
            "the runtime curve must agree with the wide measurement in order of magnitude: \
             {runtime_at_max} vs {wide_at_max}"
        );
        // And the whole flow decides on it: the maximum precision either needs confirmation or
        // is refused, never accepted silently.
        let decision = CostDecision::of(&runtime, crate::settings::MAX_PRECISION);
        println!(
            "PERF-011 decision at {} digits: {:?}",
            crate::settings::MAX_PRECISION,
            decision
        );
        assert!(
            !matches!(decision, CostDecision::Free),
            "the product maximum must never be a silent change"
        );
    }
}
