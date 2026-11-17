//! DEV-009 evidence: the desktop default precision is 30 digits.
//!
//! Three independent observations live here; each is cited by
//! `docs/goal-desktop-v4/round-38/precision-default.md`.
//!
//! 1. [`fresh_defaults_are_thirty_digits_and_inside_the_fixed_width_gate`] pins the new value,
//!    the `<= 37` gate the value is chosen to sit under, and the two default paths that must
//!    take it (a fresh settings document and a new document) while a persisted value survives.
//! 2. [`movingavg_is_precision_independent_after_the_fixed_width_emulation_was_removed`] records
//!    what replaced the old representation witness. `dsp_movingavg` used to substitute the
//!    reference implementation's fixed-width division rendering below 38 digits, which answered
//!    `0.(8)` (8/9) at 30 and `0.8(3)` (5/6) at 1000 for `movingavg([2.5], 3)`. That difference
//!    was a defect in the substituted value, not a representation worth preserving, and the
//!    emulation has been removed: `2.5 / 3` is `5/6` at every precision, so the 30-digit default
//!    costs no correctness. The surviving DEV-009 evidence is observation 1 (the constant and the
//!    default paths) plus observation 3 (the digit-budget timing).
//! 3. [`sqrt_two_at_30_is_materially_faster_than_at_1000_and_correct`] measures the whole
//!    `sqrt(2)` evaluation at 30 and at 1000 and verifies the 30-digit answer. This is a
//!    *digit-budget* measurement: in the Rust engine `sqrt(2)` is a Real, and unlike the legacy
//!    C# `NumericOps`/`DspPlugin` gates the Rust real `sqrt` has no fixed-width representation
//!    switch, so the speedup here comes from doing 33x less big-integer work. The representation
//!    switch is proven by observation 2, never by this timing.
#![cfg(all(test, feature = "real-engine"))]

use std::time::{Duration, Instant};

use crate::app::AppState;
use crate::engine::real::RealEngine;
use crate::engine::{DEFAULT_PRECISION, Evaluator};
use crate::settings::Settings;

/// Fractional digits in a plain decimal rendering (no parentheses in these fixtures).
fn fractional_digits(text: &str) -> usize {
    match text.split_once('.') {
        Some((_integer, fraction)) => fraction.chars().filter(char::is_ascii_digit).count(),
        None => 0,
    }
}

#[test]
fn fresh_defaults_are_thirty_digits_and_inside_the_fixed_width_gate() {
    // DEV-009: the decided value, written out so a regression to the old 1000 fails here even
    // if every other use moved with the constant.
    assert_eq!(
        DEFAULT_PRECISION, 30,
        "DEV-009: the desktop default must be 30 digits"
    );

    // The shared fixed-width gate the decision is built on: 30 is inside it, 1000 is not.
    assert!(
        DEFAULT_PRECISION <= lovelace_lang::complex::FAST_PATH_MAX_DIGITS,
        "DEV-009: the default {} must stay at or below the fixed-width gate {}",
        DEFAULT_PRECISION,
        lovelace_lang::complex::FAST_PATH_MAX_DIGITS
    );
    assert_eq!(
        lovelace_lang::complex::FAST_PATH_MAX_DIGITS,
        37,
        "the C#/Rust fixed-width gate is 37 computation digits"
    );
    assert!(
        1000 > lovelace_lang::complex::FAST_PATH_MAX_DIGITS,
        "the old default 1000 was outside the gate"
    );

    // A fresh application: default settings, one new document, engine already set to 30.
    let app = AppState::with_factory(|| Box::new(RealEngine::new()));
    assert_eq!(
        app.settings.active_precision, 30,
        "a fresh settings document defaults to 30"
    );
    assert_eq!(
        app.documents[0].precision, 30,
        "a new document defaults to 30"
    );
    assert_eq!(
        app.documents[0].engine().precision(),
        30,
        "the document forwards the default to its engine at construction"
    );

    // A persisted value is NOT migrated down: an install that stored 1000 keeps 1000.
    let mut persisted = Settings::new();
    persisted.active_precision = 1000;
    let app = AppState::with_settings(persisted);
    assert_eq!(
        app.documents[0].precision, 1000,
        "a stored active_precision keeps its value; only fresh defaults changed"
    );
    assert_eq!(
        app.settings.active_precision, 1000,
        "loading the stored precision must not rewrite it"
    );
}

#[test]
fn movingavg_is_precision_independent_after_the_fixed_width_emulation_was_removed() {
    let mut engine = RealEngine::new();

    // The same script at the new default, at the retired gate boundary, and at the old default.
    engine.set_precision(30);
    let at_30 = engine
        .eval("movingavg([2.5], 3)")
        .expect("movingavg evaluates at 30");
    engine.set_precision(37);
    let at_37 = engine
        .eval("movingavg([2.5], 3)")
        .expect("movingavg evaluates at 37");
    engine.set_precision(38);
    let at_38 = engine
        .eval("movingavg([2.5], 3)")
        .expect("movingavg evaluates at 38");
    engine.set_precision(1000);
    let at_1000 = engine
        .eval("movingavg([2.5], 3)")
        .expect("movingavg evaluates at 1000");

    println!(
        "DEV-009 movingavg([2.5], 3): precision 30 -> {at_30:?}; 37 -> {at_37:?}; \
         38 -> {at_38:?}; 1000 -> {at_1000:?}"
    );

    // This test was DEV-009's representation witness: the fixed-width DSP path answered `[0.(8)]`
    // (8/9) at 30/37 and the arbitrary-precision path `[0.8(3)]` (5/6) at 38/1000. That difference
    // was a defect, not a representation worth preserving, and the emulation has been removed.
    // `2.5 / 3` is `5/6` at every precision, so the result no longer depends on the digit budget:
    // the 30-digit default is a performance choice that costs no correctness.
    for (precision, value) in [(30, &at_30), (37, &at_37), (38, &at_38), (1000, &at_1000)] {
        assert_eq!(
            value, "[0.8(3)]",
            "movingavg([2.5], 3) is 5/6 at {precision} digits"
        );
    }
    assert_eq!(
        at_30, at_1000,
        "the result must not depend on the precision budget"
    );
}

#[test]
fn sqrt_two_at_30_is_materially_faster_than_at_1000_and_correct() {
    fn measure(precision: usize) -> (String, Duration) {
        let mut engine = RealEngine::new();
        engine.set_precision(precision);
        let started = Instant::now();
        let value = engine.eval("sqrt(2)").expect("sqrt(2) evaluates");
        (value, started.elapsed())
    }

    // Warm both paths so allocator and page-fault noise do not land on the first sample, then
    // take the best of five samples per precision.
    let _ = measure(30);
    let _ = measure(1000);
    let mut best_30 = Duration::MAX;
    let mut best_1000 = Duration::MAX;
    let mut value_30 = String::new();
    let mut value_1000 = String::new();
    for _ in 0..5 {
        let (value, elapsed) = measure(30);
        if elapsed < best_30 {
            best_30 = elapsed;
            value_30 = value;
        }
        let (value, elapsed) = measure(1000);
        if elapsed < best_1000 {
            best_1000 = elapsed;
            value_1000 = value;
        }
    }

    println!(
        "DEV-009 sqrt(2): precision 30 = {:.3} ms ({} fractional digits); \
         precision 1000 = {:.3} ms ({} fractional digits); ratio = {:.1}x",
        best_30.as_secs_f64() * 1000.0,
        fractional_digits(&value_30),
        best_1000.as_secs_f64() * 1000.0,
        fractional_digits(&value_1000),
        best_1000.as_secs_f64() / best_30.as_secs_f64()
    );

    // Numerically correct at the new default: the exact 30-fractional-digit truncation of
    // sqrt(2), with no rounding and no lost digit.
    assert_eq!(
        value_30, "1.414213562373095048801688724209",
        "the 30-digit default must compute sqrt(2) to 30 correct fractional digits"
    );
    assert_eq!(fractional_digits(&value_30), 30);
    // The 1000-digit path still renders every digit and agrees on the shared prefix.
    assert_eq!(fractional_digits(&value_1000), 1000);
    assert!(
        value_1000.starts_with("1.414213562373095048801688724209"),
        "the 1000-digit value must agree with the 30-digit value: {value_1000:.40}"
    );

    // Materially faster. The measured ratio is far above this bound (see round-38 evidence);
    // 2x keeps the assertion honest without making it a machine-speed lottery.
    assert!(
        best_30 * 2 <= best_1000,
        "sqrt(2) at 30 digits must be materially faster than at 1000: {best_30:?} vs {best_1000:?}"
    );
}
