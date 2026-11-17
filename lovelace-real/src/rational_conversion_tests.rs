//! Canonical `Rat` ↔ `Real` conversion contracts.
//!
//! These tests live beside `lovelace-real` because it is the crate that can see both types. They
//! pin the four-part exactness report: an exact terminating rational is lossless, a non-terminating
//! rational whose period closes inside the budget is still exact, a bounded projection is
//! represented as truncated, and a truncated `Real` converted back to a rational does not become
//! an exact input again.

use lovelace_int::Int;
use lovelace_nat::Nat;
use lovelace_rational::{InputFidelity, Rat, RepresentationExactness};

use crate::Real;

fn rat(num: i64, den: u64) -> Rat {
    Rat::from_parts(Int::from_i64(num), Nat::from_u64(den)).expect("non-zero denominator")
}

#[test]
fn terminating_and_periodic_rationals_convert_losslessly() {
    let eighth = Real::from_rational(&rat(1, 8), 30);
    assert!(eighth.is_lossless());
    assert!(eighth.provenance.is_value_exact());
    assert_eq!(eighth.value.to_string(), "0.125");
    assert_eq!(eighth.value.to_rat().value, rat(1, 8));

    // The period closes inside the budget, so the value is exact with period metadata.
    let third = Real::from_rational(&rat(1, 3), 30);
    assert!(third.is_lossless());
    assert_eq!(third.value.period_length(), 1);
    assert_eq!(third.value.to_string(), "0.(3)");
    assert_eq!(third.value.to_rat().value, rat(1, 3));

    let negative = Real::from_rational(&rat(-1, 3), 30);
    assert!(negative.is_lossless());
    assert_eq!(negative.value.to_string(), "-0.(3)");
    assert_eq!(negative.value.to_rat().value, rat(-1, 3));

    let integer = Real::from_rational(&rat(7, 1), 30);
    assert!(integer.is_lossless());
    assert_eq!(integer.value.to_string(), "7");
    assert_eq!(integer.value.to_rat().value, rat(7, 1));
}

#[test]
fn a_long_period_is_an_explicit_truncated_projection() {
    // 1/7 has a six-digit period; a three-digit budget closes no period and truncates.
    let projected = Real::from_rational(&rat(1, 7), 3);
    assert!(!projected.is_lossless());
    assert_eq!(
        projected.provenance.representation,
        RepresentationExactness::Truncated { precision: 3 }
    );
    assert!(!projected.value.is_exact());
    assert_eq!(projected.value.to_string(), "0.142");

    // The returned digits have an exact rational representation, but the conversion back does
    // not claim the original input was exact.
    let back = projected.value.to_rat();
    assert!(!back.is_lossless());
    assert_eq!(
        back.provenance.input,
        InputFidelity::Truncated { precision: 3 }
    );
    assert_ne!(back.value, rat(1, 7));
}

#[test]
fn zero_is_canonical_across_the_boundary() {
    let zero = Real::from_rational(&Rat::zero(), 30);
    assert!(zero.is_lossless());
    assert_eq!(zero.value.to_string(), "0");
    assert_eq!(zero.value.to_rat().value, Rat::zero());
    assert_eq!(Rat::zero().to_string(), "0");
}

#[test]
fn the_render_budget_is_separate_from_the_value_exactness() {
    let third = Real::from_rational(&rat(1, 3), 30);
    assert!(third.value.is_exact());
    // Printing fewer digits does not make the stored value inexact.
    assert_eq!(third.value.to_decimal_fixed(2), "0.33");
    assert!(third.value.is_exact());
}
