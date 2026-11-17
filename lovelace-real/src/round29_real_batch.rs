//! Round-29: the exact-period add/sub property of Lovelace.Real.Tests.RealAddPeriodicExactTests.
//!
//! The C# property: for exact rationals a = p/q and b = r/s, a + b denotes EXACTLY
//! (p*s + r*q)/(q*s) and a - b denotes exactly (p*s - r*q)/(q*s), in lowest terms, reconstructed
//! from the value's PUBLIC state (magnitude, exponent, period start, period length) rather than
//! from any production conversion helper. A fixed expansion of each periodic operand truncated
//! the operand, which is how 1/19 + 1/17 (period 144) used to publish a wrong exact value.

use lovelace_int::Int;
use lovelace_nat::Nat;

use crate::Real;

fn pow10(places: i64) -> Nat {
    Nat::from_u64(10).pow(places as u64)
}

fn gcd(mut a: Nat, mut b: Nat) -> Nat {
    while !b.is_zero() {
        let remainder = a.div_rem(&b).expect("nonzero divisor").1;
        a = b;
        b = remainder;
    }
    a
}

fn reduce(num: Int, den: Nat) -> (Int, Nat) {
    if num.is_zero() {
        return (Int::from_i64(0), Nat::from_u64(1));
    }
    let divisor = gcd(num.magnitude().clone(), den.clone());
    if divisor.to_decimal() == "1" {
        return (num, den);
    }
    let (reduced_num, _) = num.div_rem(&divisor).expect("nonzero divisor");
    let (reduced_den, _) = den.div_rem(&divisor).expect("nonzero divisor");
    (reduced_num, reduced_den)
}

fn quotient(numerator: i64, denominator: i64) -> Real {
    Real::from_i64(numerator)
        .div(&Real::from_i64(denominator), 1000)
        .expect("nonzero denominator")
}

/// Reconstructs the exact rational a Real denotes from its public value state.
fn rational_of(value: &Real) -> (Int, Nat) {
    let magnitude = value.int.magnitude().clone();
    let (num, den) = if value.period_len == 0 {
        let digits = Int::from_decimal(&magnitude.to_decimal()).expect("stored digits");
        if value.exp >= 0 {
            let scale = Int::from_decimal(&pow10(value.exp).to_decimal()).expect("power of ten");
            (digits.mul(&scale), Nat::from_u64(1))
        } else {
            (digits, pow10(-value.exp))
        }
    } else {
        let start = value.period_start;
        let length = value.period_len;
        assert_eq!(
            value.exp,
            -(start + length),
            "periodic invariant: exponent == -(start+length)"
        );
        let ten_to_p = pow10(length);
        let block = magnitude.div_rem(&ten_to_p).expect("power of ten").1;
        let nine = ten_to_p.sub(&Nat::from_u64(1)).expect("ten^p - 1");
        let numerator = magnitude.mul(&nine).add(&block);
        let denominator = pow10(start + length).mul(&nine);
        (
            Int::from_decimal(&numerator.to_decimal()).expect("numerator"),
            denominator,
        )
    };
    let num = if value.int.is_negative() && !num.is_zero() {
        num.negate()
    } else {
        num
    };
    reduce(num, den)
}

fn expected(numerator: i64, denominator: i64) -> (Int, Nat) {
    reduce(Int::from_i64(numerator), Nat::from_u64(denominator as u64))
}

/// The corpus's periodic left denominators, and the right denominators (periodic and terminating).
const PERIODIC: [i64; 12] = [3, 6, 7, 9, 11, 12, 13, 17, 19, 21, 37, 99];
const RIGHT: [i64; 8] = [3, 7, 17, 99, 2, 4, 25, 1000];
const LEFT_NUMERATORS: [i64; 3] = [1, 2, -5];
const RIGHT_NUMERATORS: [i64; 3] = [1, -1, 5];

/// Add_ExactRationalPair_Property_DenotesTheExactRationalSum: the 9 deferred q=19/s=17 rows are
/// part of the corpus, and the whole corpus is asserted here.
#[test]
fn real_add_periodic_pairs_denote_the_exact_rational_sum() {
    for q in PERIODIC {
        for p in LEFT_NUMERATORS {
            for s in RIGHT {
                for r in RIGHT_NUMERATORS {
                    let left = quotient(p, q);
                    assert!(left.period_len > 0, "{p}/{q} is not periodic");
                    let sum = left.add(&quotient(r, s));
                    assert_eq!(
                        rational_of(&sum),
                        expected(p * s + r * q, q * s),
                        "({p}/{q}) + ({r}/{s})"
                    );
                }
            }
        }
    }
}

/// Subtract_ExactRationalPair_Property_DenotesTheExactRationalDifference: the 9 deferred rows.
#[test]
fn real_subtract_periodic_pairs_denote_the_exact_rational_difference() {
    for q in PERIODIC {
        for p in LEFT_NUMERATORS {
            for s in RIGHT {
                for r in RIGHT_NUMERATORS {
                    let left = quotient(p, q);
                    let difference = left.sub(&quotient(r, s));
                    assert_eq!(
                        rational_of(&difference),
                        expected(p * s - r * q, q * s),
                        "({p}/{q}) - ({r}/{s})"
                    );
                }
            }
        }
    }
}

/// Subtract_OneThirdMinusNThrees: the period START is the count of threes, and the exactness
/// provenance flips exactly at the computation cap (the reference marks 1000-threes inexact).
#[test]
fn real_period_start_and_exactness_flip_at_the_computation_cap() {
    for (threes, exact) in [(998usize, true), (999, true), (1000, false)] {
        let third = quotient(1, 3);
        let literal = Real::from_str(&format!("0.{}", "3".repeat(threes))).expect("literal");
        let difference = third.sub(&literal);
        assert_eq!(
            difference.period_start, threes as i64,
            "{threes} threes: period start"
        );
        assert_eq!(difference.period_len, 1, "{threes} threes: period length");
        assert_eq!(difference.is_exact(), exact, "{threes} threes: exactness");
        assert_eq!(
            rational_of(&difference),
            (
                Int::from_i64(1),
                Nat::from_u64(3).mul(&pow10(threes as i64))
            )
        );
    }
}

/// Subtract_PeriodicOperandMinusItsOwnTruncation_IsTheTruncationRemainder: the leading zeros a
/// cancellation produces are carried by the EXPONENT, not charged to the digit budget.
#[test]
fn real_periodic_minus_its_own_truncation_is_the_remainder() {
    let third = quotient(1, 3);
    assert!(third.period_len > 0);
    let truncated = Real::from_str(&format!("0.{}", "3".repeat(1000))).expect("literal");
    let difference = third.sub(&truncated);
    assert!(!difference.is_zero(), "1/3 - 0.333...3 must not be zero");
    assert_eq!(
        rational_of(&difference),
        (Int::from_i64(1), Nat::from_u64(3).mul(&pow10(1000))),
        "the difference is 1/(3*10^1000)"
    );
    // Canonical shape: the period begins after the 1000 zeros the subtraction cancels.
    assert_eq!(difference.period_start, 1000);
    assert_eq!(difference.period_len, 1);
    assert_eq!(difference.exp, -1001);
}

/// COR-004: a deterministic numeric-literal campaign with accepted and rejected censuses.
#[test]
fn numeric_literal_fuzz_campaign_never_panics() {
    fn next(state: &mut u64) -> u64 {
        let mut x = *state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        *state = x;
        x
    }
    const FRAGMENTS: &[&str] = &[
        "0", "1", "9", "3", ".", "-", "+", "(", ")", "e", "E", "a", " ", "0.(3)", "..", "--",
        "\u{0}", "é",
    ];
    let mut state = 0x0d15_ea5e_2026_1118u64;
    let mut accepted = 0usize;
    let mut rejected = 0usize;
    for step in 0..20_000 {
        let mut text = String::new();
        if step % 2 == 0 {
            // One definitely valid form: digits, optional fraction, optional periodic suffix.
            let integer = next(&mut state) % 10_000;
            text.push_str(&integer.to_string());
            if next(&mut state) % 2 == 0 {
                text.push('.');
                text.push_str(&(next(&mut state) % 1000).to_string());
            }
            if next(&mut state) % 7 == 0 {
                text.push_str(".(3)");
            }
        } else {
            let pieces = (next(&mut state) % 12) as usize;
            for _ in 0..pieces {
                text.push_str(FRAGMENTS[(next(&mut state) as usize) % FRAGMENTS.len()]);
            }
        }
        if Real::from_str(&text).is_some() {
            accepted += 1;
        } else {
            rejected += 1;
        }
    }
    assert!(
        accepted > 1_000,
        "only {accepted} generated literals parsed; campaign is not exercising the parser"
    );
    assert!(
        rejected > 100,
        "only {rejected} generated literals were rejected; campaign has no rejection signal"
    );
}

/// WL-01 (numeric kernel): randomized 12-digit factors promote 64 -> 128 exactly, and a
/// 20-digit x 20-digit product promotes 128 -> Real. Every fixed-width result is compared with the
/// arbitrary-precision `Real` oracle.
#[test]
fn lreal_tier_boundaries_promote_and_match_real() {
    use crate::lreal::{LReal64, LReal128, LRealError};

    fn next(state: &mut u64) -> u64 {
        let mut x = *state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        *state = x;
        x
    }

    let mut state = 0x0d15_ea5e_2026_1121u64;
    for _ in 0..200 {
        // Twelve significant digits each; the 24-digit product exceeds u64 but fits u128.
        let a = format!(
            "{:012}",
            100_000_000_000u64 + next(&mut state) % 900_000_000_000
        );
        let b = format!(
            "{:012}",
            100_000_000_000u64 + next(&mut state) % 900_000_000_000
        );
        let a64 = LReal64::parse(&a).expect("12 digits fit LReal64");
        let b64 = LReal64::parse(&b).expect("12 digits fit LReal64");
        assert_eq!(
            a64.mul(&b64),
            Err(LRealError::Promote),
            "{a} * {b} needs the wide tier"
        );
        let a128 = LReal128::parse(&a).expect("12 digits fit LReal128");
        let b128 = LReal128::parse(&b).expect("12 digits fit LReal128");
        let product = a128.mul(&b128).expect("24 digits fit LReal128");
        let oracle = Real::from_str(&a)
            .unwrap()
            .mul(&Real::from_str(&b).unwrap());
        assert_eq!(product.to_real(), oracle, "{a} * {b}");
    }

    let a = "12345678901234567890";
    let b = "98765432109876543210";
    let a128 = LReal128::parse(a).expect("20 digits fit LReal128");
    let b128 = LReal128::parse(b).expect("20 digits fit LReal128");
    assert_eq!(
        a128.mul(&b128),
        Err(LRealError::Promote),
        "the 40-digit product exceeds LReal128"
    );
    let oracle = Real::from_str(a).unwrap().mul(&Real::from_str(b).unwrap());
    assert_eq!(a128.to_real().mul(&b128.to_real()), oracle);
}
