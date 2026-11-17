//! Cross-library semantic consistency (RNL-CI-007, RNL-CI-008).
//!
//! The same mathematical operation is performed through the canonical rational kernel, through
//! Symbolics, through Systems and through the language. The test compares exact values, signs,
//! precision classification and error semantics, not just "it evaluates".
//!
//! Precision boundaries are checked separately: a terminating decimal, an exact periodic decimal, a
//! projection beyond the stored budget, a truncated `Real` and an exact-to-inexact round trip each
//! report the fidelity they actually have.

use lovelace_int::Int;
use lovelace_lang::{Engine, LangError, Value};
use lovelace_nat::Nat;
use lovelace_rational::Rat;
use lovelace_real::Real;
use lovelace_symbolics::Expr;
use lovelace_systems::{NeverCancel, Q, SolverOptions, mm1};

fn rat(numerator: i64, denominator: u64) -> Rat {
    Rat::from_parts(Int::from_i64(numerator), Nat::from_u64(denominator))
        .expect("non-zero denominator")
}

fn eval(engine: &mut Engine, source: &str) -> Value {
    engine
        .eval(source)
        .unwrap_or_else(|error| panic!("`{source}` failed: {}", error.message))
}

fn typed(engine: &mut Engine, source: &str) -> String {
    let value = eval(engine, source);
    engine.format_typed(&value)
}

fn refusal(engine: &mut Engine, source: &str) -> LangError {
    engine
        .eval(source)
        .err()
        .unwrap_or_else(|| panic!("`{source}` unexpectedly succeeded"))
}

#[test]
fn signed_division_is_identical_in_the_kernel_symbolics_and_the_language() {
    // Canonical kernel.
    let kernel = rat(1, 2)
        .checked_div(&rat(-1, 4))
        .expect("non-zero divisor");
    assert_eq!(kernel, Rat::from_i64(-2));

    // Symbolics folds the same operation to the same exact constant.
    let symbolic = Expr::div(
        Expr::rational(&Int::from_i64(1), &Nat::from_u64(2)),
        Expr::rational(&Int::from_i64(-1), &Nat::from_u64(4)),
    );
    assert_eq!(symbolic, Expr::num(-2));

    // The language evaluates the expression as a Real and publishes the same value and sign.
    let mut engine = Engine::new();
    assert_eq!(typed(&mut engine, "(1/2) / (-1/4)"), "-2 (Real)");
    // The sign combinations agree on all three surfaces.
    assert_eq!(typed(&mut engine, "(-1/2) / (-1/4)"), "2 (Real)");
    assert_eq!(typed(&mut engine, "(-1/2) / (1/4)"), "-2 (Real)");
}

#[test]
fn systems_exact_values_agree_with_the_kernel_symbolics_and_the_language() {
    let result = mm1(
        &Q::from_i64(8),
        &Q::from_i64(10),
        &SolverOptions::default(),
        &NeverCancel,
    )
    .expect("stable M/M/1");
    let utilization = &result.stations[0].utilization;
    assert_eq!(utilization, &rat(4, 5));

    // Symbolics carries the same canonical constant.
    let symbolic = lovelace_symbolics::canonical_parse("(rat 4 5)").expect("parses");
    assert_eq!(
        symbolic,
        Expr::rational(&Int::from_i64(4), &Nat::from_u64(5))
    );

    // The language exposes the exact sidecar next to the decimal projection.
    let mut engine = Engine::new();
    assert_eq!(
        typed(&mut engine, "sys_mm1(8, 10).utilization"),
        "0.8 (Real)"
    );
    assert_eq!(
        typed(
            &mut engine,
            "sys_mm1(8, 10).exact_values.utilization.numerator"
        ),
        "4 (Integer)"
    );
    assert_eq!(
        typed(
            &mut engine,
            "sys_mm1(8, 10).exact_values.utilization.denominator"
        ),
        "5 (Natural)"
    );
    // The exact value survives a language round trip through a rational comparison.
    assert_eq!(
        typed(&mut engine, "sys_mm1(8, 10).utilization == 4/5"),
        "True (Boolean)"
    );
}

#[test]
fn exact_periodic_values_keep_their_period_on_every_surface() {
    let third = rat(1, 3);
    let exact = Real::from_rational(&third, 30);
    assert!(exact.is_lossless());
    assert!(exact.value.is_exact());
    assert_eq!(exact.value.period_length(), 1);
    assert_eq!(exact.value.to_string(), "0.(3)");

    let mut engine = Engine::new();
    let language = eval(&mut engine, "1/3");
    let Value::Real(language_real) = &language else {
        panic!("1/3 must be a Real, got {language:?}");
    };
    assert!(language_real.is_exact(), "1/3 is an exact periodic decimal");
    assert_eq!(engine.format_typed(&language), "0.(3) (Real)");

    // Systems reports the same exact rational, and its decimal projection keeps the period.
    let result = mm1(
        &Q::from_i64(1),
        &Q::from_i64(3),
        &SolverOptions::default(),
        &NeverCancel,
    )
    .expect("stable");
    assert_eq!(result.stations[0].utilization, third);
    assert_eq!(
        typed(&mut engine, "sys_mm1(1, 3).utilization"),
        "0.(3) (Real)"
    );
    assert_eq!(
        typed(
            &mut engine,
            "sys_mm1(1, 3).exact_values.utilization.denominator"
        ),
        "3 (Natural)"
    );
}

#[test]
fn a_bounded_projection_is_not_reported_as_an_exact_value() {
    // 1/7 has a six-digit period; three stored digits are a projection, not the value.
    let projected = Real::from_rational(&rat(1, 7), 3);
    assert!(!projected.is_lossless());
    assert!(!projected.value.is_exact());
    assert_eq!(projected.value.to_string(), "0.142");

    // Converting the stored digits back to a rational is representation-exact but input-truncated.
    let back = projected.value.to_rat();
    assert!(!back.is_lossless());
    assert!(!back.provenance.is_value_exact());

    // A language transcendental is inexact at the requested precision, unlike an exact periodic.
    let mut engine = Engine::new();
    let Value::Real(pi) = eval(&mut engine, "pi(30)") else {
        panic!("pi(30) must be a Real");
    };
    assert!(!pi.is_exact(), "pi(30) is a bounded computation");
    assert!(!pi.to_rat().is_lossless());

    // Systems refuses to promote that precision-limited Real to an exact model input.
    let refused = refusal(&mut engine, "sys_mm1(pi(30), 10)");
    assert_eq!(refused.code, "InvalidArgument");
    assert_eq!(refused.category, "TypeMismatch");
    assert!(
        refused.message.contains("precision-limited"),
        "{}",
        refused.message
    );
}

#[test]
fn error_categories_agree_across_the_surfaces() {
    // Kernel: explicit division-by-zero error.
    let kernel = rat(1, 2).checked_div(&Rat::zero()).expect_err("zero");
    assert_eq!(kernel.code(), "DivisionByZero");
    assert_eq!(kernel.category(), "DomainError");

    // Language: the same stable code and category.
    let mut engine = Engine::new();
    let language = refusal(&mut engine, "1/0");
    assert_eq!(language.code, "DivisionByZero");
    assert_eq!(language.category, "DomainError");

    // Systems: a partial sweep grid is a structured DomainError, not a clamped success.
    let sweep = refusal(
        &mut engine,
        "sys_sweep(sys_open(1, [sys_station(\"api\", 1/10, 1)]), 1, 6, 4)",
    );
    assert_eq!(sweep.code, "SweepEndpointNotOnGrid");
    assert_eq!(sweep.category, "DomainError");

    // An unstable station keeps its own stable code on the language surface.
    let unstable = refusal(&mut engine, "sys_mm1(10, 10)");
    assert_eq!(unstable.code, "UnstableStation");
    assert_eq!(unstable.category, "DomainError");
}
