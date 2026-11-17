//! Deterministic, seeded property campaigns for the canonical rational kernel.
//!
//! These are property tests, not fuzz tests: the generator is a fixed-seed LCG in this file, so a
//! failure reproduces on every machine and the seed in the assertion message is the whole recipe.
//! The campaigns cover sign combinations, canonical reduction, ordering, algebraic identities, zero
//! handling, large operands beyond `u64`, conversion round trips and resource-limit refusals.
//!
//! RNL-CI-005: the seeds are constants; the campaign never depends on wall-clock time, the host or
//! an operating-system random source.

use lovelace_int::Int;
use lovelace_nat::Nat;
use lovelace_rational::{Cancel, Limits, NeverCancel, Rat, RationalError};

/// The campaign seed. Changing it is changing the campaign and must be justified in review.
const SEED: u64 = 0x5EED_2024_1117_0001;

/// A minimal deterministic 64-bit linear congruential generator.
struct Seeded {
    state: u64,
}

impl Seeded {
    fn new(seed: u64) -> Seeded {
        Seeded { state: seed }
    }

    fn next_u64(&mut self) -> u64 {
        // Numerical Recipes' LCG constants; the low bits are not used directly.
        self.state = self
            .state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.state
    }

    fn next_i64(&mut self, bound: i64) -> i64 {
        (self.next_u64() >> 33) as i64 % bound
    }

    fn next_u64_bounded(&mut self, bound: u64) -> u64 {
        (self.next_u64() >> 33) % bound
    }

    fn small_rat(&mut self) -> Rat {
        let numerator = self.next_i64(41) - 20;
        let denominator = 1 + self.next_u64_bounded(20);
        Rat::from_parts(Int::from_i64(numerator), Nat::from_u64(denominator))
            .expect("the generator never produces a zero denominator")
    }

    fn large_nat(&mut self) -> Nat {
        let base = Nat::from_u64(self.next_u64_bounded(1_000_000) + 1);
        let zeros = self.next_u64_bounded(90) as usize;
        base.mul_pow10(zeros)
    }

    fn large_rat(&mut self) -> Rat {
        let numerator = Int::from_nat(self.large_nat());
        let denominator = self.large_nat().add(&Nat::from_u64(1));
        let value = Rat::from_parts(numerator, denominator).expect("non-zero denominator");
        if self.next_u64() & 1 == 0 {
            value.neg()
        } else {
            value
        }
    }
}

fn assert_canonical(value: &Rat, context: &str) {
    assert!(
        !value.denominator().is_zero(),
        "{context}: denominator is zero"
    );
    assert!(
        !value.numerator().is_zero() || value.denominator().is_one(),
        "{context}: zero must be 0/1, got {value}"
    );
    let gcd = value.numerator().magnitude().gcd(value.denominator());
    assert!(
        gcd.is_one(),
        "{context}: {value} is not reduced (gcd {})",
        gcd.to_decimal()
    );
}

#[test]
fn seeded_sign_combinations_and_canonical_reduction() {
    let mut rng = Seeded::new(SEED);
    for step in 0..2_000 {
        let a = rng.small_rat();
        let b = rng.small_rat();
        for value in [a.add(&b), a.sub(&b), a.mul(&b)] {
            assert_canonical(&value, &format!("seed {SEED:#x} step {step}: a={a} b={b}"));
        }
        if !b.is_zero() {
            let quotient = a
                .checked_div(&b)
                .expect("a non-zero generated divisor is divisible");
            assert_canonical(
                &quotient,
                &format!("seed {SEED:#x} step {step}: a={a} b={b}"),
            );
            // Division is the inverse of multiplication for a non-zero divisor.
            assert_eq!(
                quotient.mul(&b),
                a,
                "seed {SEED:#x} step {step}: (a/b)*b must be a"
            );
        }
    }
}

#[test]
fn seeded_algebraic_identities_and_ordering() {
    let mut rng = Seeded::new(SEED ^ 0xA5A5_A5A5);
    for step in 0..1_000 {
        let a = rng.small_rat();
        let b = rng.small_rat();
        let c = rng.small_rat();
        let context = format!("seed {SEED:#x} step {step}: a={a} b={b} c={c}");

        assert_eq!(a.add(&b), b.add(&a), "addition commutes: {context}");
        assert_eq!(
            a.add(&b).add(&c),
            a.add(&b.add(&c)),
            "addition associates: {context}"
        );
        assert_eq!(
            a.mul(&b.add(&c)),
            a.mul(&b).add(&a.mul(&c)),
            "multiplication distributes: {context}"
        );
        assert_eq!(
            a.sub(&b),
            a.add(&b.neg()),
            "subtraction is negation: {context}"
        );
        assert_eq!(a.add(&b.neg()).add(&b), a, "additive inverse: {context}");

        match a.cmp(&b) {
            std::cmp::Ordering::Less => {
                assert_eq!(b.cmp(&a), std::cmp::Ordering::Greater, "{context}");
                assert!(
                    a.add(&c) < b.add(&c),
                    "order is translation invariant: {context}"
                );
            }
            std::cmp::Ordering::Equal => {
                assert_eq!(a.add(&c), b.add(&c), "equal values stay equal: {context}");
            }
            std::cmp::Ordering::Greater => {
                assert_eq!(b.cmp(&a), std::cmp::Ordering::Less, "{context}");
                assert!(
                    a.add(&c) > b.add(&c),
                    "order is translation invariant: {context}"
                );
            }
        }
    }
}

#[test]
fn seeded_zero_handling_is_canonical() {
    let mut rng = Seeded::new(SEED ^ 0x0F0F_0F0F);
    for step in 0..500 {
        let value = rng.small_rat();
        let zero = Rat::zero();
        assert_eq!(value.add(&zero), value, "x + 0 = x at step {step}");
        assert_eq!(value.mul(&zero), Rat::zero(), "x * 0 = 0 at step {step}");
        assert_eq!(zero.sub(&value), value.neg(), "0 - x = -x at step {step}");
        assert_eq!(zero.denominator(), &Nat::from_u64(1), "zero is 0/1");
        if !value.is_zero() {
            assert_eq!(
                value.pow_i64(0),
                Some(Rat::one()),
                "x^0 = 1 for x != 0 at step {step}"
            );
            assert_eq!(zero.checked_div(&value), Ok(Rat::zero()));
        }
    }
    assert_eq!(
        Rat::zero().checked_div(&Rat::zero()),
        Err(RationalError::DivisionByZero)
    );
    assert_eq!(
        Rat::zero().checked_pow(-1, &Limits::default(), &NeverCancel),
        Err(RationalError::NegativePowerOfZero)
    );
}

#[test]
fn seeded_large_operands_normalize_beyond_u64() {
    let mut rng = Seeded::new(SEED ^ 0xB16B00B5);
    for step in 0..200 {
        let a = rng.large_rat();
        let b = rng.large_rat();
        assert_canonical(&a, &format!("large seed {SEED:#x} step {step}"));
        assert_canonical(&b, &format!("large seed {SEED:#x} step {step}"));
        let product = a.mul(&b);
        assert_canonical(&product, &format!("large product step {step}"));
        let sum = a.add(&b);
        assert_canonical(&sum, &format!("large sum step {step}"));
        // Divide by b's reciprocal: (a/b) * (b/a) is 1 for non-zero values.
        if !a.is_zero() && !b.is_zero() {
            let round_trip = a
                .div(&b)
                .expect("non-zero")
                .mul(&b.div(&a).expect("non-zero"));
            assert_eq!(round_trip, Rat::one(), "large round trip at step {step}");
        }
    }
}

#[test]
fn seeded_signed_division_handles_every_sign_pair() {
    let mut rng = Seeded::new(SEED ^ 0xD1D1D3);
    for step in 0..500 {
        let a = rng.small_rat();
        let magnitude_numerator = 1 + rng.next_i64(20).abs();
        let magnitude_denominator = 1 + rng.next_u64_bounded(20);
        let positive_divisor = Rat::from_parts(
            Int::from_i64(magnitude_numerator),
            Nat::from_u64(magnitude_denominator),
        )
        .expect("non-zero denominator");
        let positive_quotient = a.checked_div(&positive_divisor).expect("non-zero divisor");
        for divisor_sign in [1i64, -1] {
            let divisor = Rat::from_parts(
                Int::from_i64(divisor_sign * magnitude_numerator),
                Nat::from_u64(magnitude_denominator),
            )
            .expect("non-zero denominator");
            let quotient = a.checked_div(&divisor).expect("non-zero divisor");
            // The canonical representation has a positive denominator; the divisor's sign lives
            // solely in its numerator. A negative divisor negates the quotient.
            let expected = if divisor_sign < 0 {
                positive_quotient.neg()
            } else {
                positive_quotient.clone()
            };
            assert_eq!(
                quotient, expected,
                "seed {SEED:#x} step {step}: divisor sign {divisor_sign}, dividend {a}"
            );
            // The sign is exactly the product of the operand signs.
            assert_eq!(
                quotient.is_negative(),
                a.is_negative() ^ (divisor_sign < 0) && !a.is_zero(),
                "seed {SEED:#x} step {step}: sign composition"
            );
        }
    }
}

#[test]
fn seeded_conversion_round_trips() {
    let mut rng = Seeded::new(SEED ^ 0xC0FFEE);
    for _ in 0..1_000 {
        let integer = rng.next_i64(1_000_000);
        let value = Rat::from_int(&Int::from_i64(integer));
        assert_eq!(value.to_i64(), Some(integer));
        assert_eq!(value.to_int_exact(), Some(Int::from_i64(integer)));

        let natural = rng.next_u64_bounded(u64::MAX / 2);
        assert_eq!(
            Rat::from_nat(&Nat::from_u64(natural)).to_u64(),
            Some(natural)
        );

        let numerator = rng.next_i64(1_000) - 500;
        let denominator = 2 + rng.next_u64_bounded(500);
        let non_integer = Rat::from_parts(Int::from_i64(numerator), Nat::from_u64(denominator))
            .expect("non-zero");
        // A non-integral rational never claims an integer conversion.
        if !non_integer.is_integer() {
            assert_eq!(non_integer.to_i64(), None);
            assert_eq!(non_integer.to_int_exact(), None);
        }
    }
}

#[test]
fn seeded_resource_limit_refusals_never_return_a_partial_value() {
    let mut rng = Seeded::new(SEED ^ 0xBADB0A5);
    for _ in 0..100 {
        let base = rng.small_rat();
        if base.is_zero() || base.abs() == Rat::one() {
            continue;
        }
        let exponent = 10 + rng.next_i64(60) as u32;
        let tight = Limits::digits(4);
        let error = base
            .checked_pow(exponent as i64, &tight, &NeverCancel)
            .expect_err("a 60-bit exponent of a non-unit value exceeds four digits");
        assert!(
            matches!(error, RationalError::DigitBudgetExceeded { limit: 4, .. }),
            "unexpected refusal: {error}"
        );

        let generous = Limits::digits(100_000);
        let value = base
            .checked_pow(exponent as i64, &generous, &NeverCancel)
            .expect("the same power fits the generous budget");
        assert_canonical(&value, "budgeted power");
    }
}

#[test]
fn the_same_seed_reproduces_the_campaign() {
    fn digest(seed: u64) -> String {
        let mut rng = Seeded::new(seed);
        let mut text = String::new();
        for _ in 0..200 {
            let a = rng.small_rat();
            let b = rng.small_rat();
            text.push_str(&a.add(&b).to_string());
            text.push('|');
            text.push_str(&a.mul(&b).to_string());
            text.push('|');
            if !b.is_zero() {
                text.push_str(&a.checked_div(&b).expect("non-zero").to_string());
            }
            text.push(';');
        }
        text
    }
    assert_eq!(digest(SEED), digest(SEED), "the campaign is reproducible");
    assert_ne!(
        digest(SEED),
        digest(SEED ^ 1),
        "the seed actually changes the campaign"
    );
}

#[test]
fn cancellation_is_polled_during_a_long_power() {
    struct CancelAfter {
        remaining: std::cell::Cell<u32>,
    }
    impl Cancel for CancelAfter {
        fn is_cancelled(&self) -> bool {
            let left = self.remaining.get();
            if left == 0 {
                return true;
            }
            self.remaining.set(left - 1);
            false
        }
    }
    let token = CancelAfter {
        remaining: std::cell::Cell::new(3),
    };
    let result = Rat::from_i64(3).checked_pow(1_000, &Limits::unlimited(), &token);
    assert_eq!(result, Err(RationalError::Cancelled));
}

/// A rational whose numerator and denominator each have 101 decimal digits and are coprime.
///
/// The two parts differ by two and both are odd, so their only common divisor is one: the value is
/// already canonical and its parts are genuinely large, not a disguised small fraction.
fn hundred_digit_rational(offset: u64) -> Rat {
    assert!(offset % 2 == 1, "an odd offset keeps both parts coprime");
    let ten100 = Nat::from_u64(10).pow(100);
    Rat::from_parts(
        Int::from_nat(ten100.add(&Nat::from_u64(offset))),
        ten100.add(&Nat::from_u64(offset + 2)),
    )
    .expect("the denominator is non-zero")
}

#[test]
fn preflight_refuses_multiplication_before_the_products_are_formed() {
    let limits = Limits::digits(50);
    let left = hundred_digit_rational(1);
    let right = hundred_digit_rational(5);
    assert!(
        left.decimal_digits() > 100,
        "the operands are genuinely large"
    );

    // The exact product needs on the order of 200 numerator digits and 200 denominator digits.
    let error = left
        .mul_with(&right, &limits)
        .expect_err("the product is far over 50 digits");
    assert_eq!(error.code(), "RationalDigitBudgetExceeded");
    assert_eq!(error.category(), "BudgetExceeded");
    match error {
        RationalError::DigitBudgetExceeded { limit, observed } => {
            assert_eq!(limit, 50);
            assert!(observed > 50, "the estimate must exceed the limit");
        }
        other => panic!("expected DigitBudgetExceeded, got {other}"),
    }

    // These exact results are one, zero and one: each fits the 50-digit budget comfortably. Only a
    // check performed before the product/sum is formed can refuse them, so these cases separate
    // "the result was checked" from "the operation was preflighted". The documented trade-off is
    // that the conservative estimate over-counts and refuses a few results that would have fit.
    let reciprocal = Rat::from_parts(
        Int::from_nat(left.denominator().clone()),
        left.numerator().magnitude().clone(),
    )
    .expect("non-zero denominator");
    assert_eq!(left.mul(&reciprocal), Rat::one());
    let error = left
        .mul_with(&reciprocal, &limits)
        .expect_err("the preflight must refuse before forming the ~200-digit product");
    assert!(matches!(
        error,
        RationalError::DigitBudgetExceeded { limit: 50, .. }
    ));

    let negative = left.neg();
    assert_eq!(left.add(&negative), Rat::zero());
    let error = left
        .add_with(&negative, &limits)
        .expect_err("the preflight must refuse before forming the common denominator");
    assert!(matches!(
        error,
        RationalError::DigitBudgetExceeded { limit: 50, .. }
    ));

    assert_eq!(left.checked_div(&left), Ok(Rat::one()));
    let error = left
        .checked_div_with(&left, &limits)
        .expect_err("the preflight must refuse before forming the division products");
    assert!(matches!(
        error,
        RationalError::DigitBudgetExceeded { limit: 50, .. }
    ));
}

#[test]
fn huge_power_is_refused_by_preflight() {
    let limits = Limits::digits(50);

    // 2^1_000_000 needs 301030 decimal digits. A preflight refusal can report a count derived from
    // the exponent; the squaring loop cannot, because it stops at the first intermediate over 50
    // digits (79 for this base). The observed size therefore distinguishes the two paths.
    let error = Rat::from_i64(2)
        .checked_pow(1_000_000, &limits, &NeverCancel)
        .expect_err("2^1_000_000 is far over 50 digits");
    match error {
        RationalError::DigitBudgetExceeded { limit, observed } => {
            assert_eq!(limit, 50);
            assert!(
                observed > 100_000,
                "the refusal must come from the preflight estimate, not the first over-budget \
                 squaring (observed {observed})"
            );
        }
        other => panic!("expected DigitBudgetExceeded, got {other}"),
    }

    // The preflight must never reject a power whose exact result fits: 2^100 is 31 digits.
    let value = Rat::from_i64(2)
        .checked_pow(100, &limits, &NeverCancel)
        .expect("2^100 fits fifty digits");
    assert_eq!(value.to_string(), "1267650600228229401496703205376");

    // With no digit budget the preflight has nothing to compare, so the loop's own cancellation
    // poll is still what stops a huge exponent.
    assert_eq!(
        Rat::from_i64(2).checked_pow(1_000_000, &Limits::unlimited(), &AlwaysCancel),
        Err(RationalError::Cancelled)
    );
}

/// A cancellation token that reports false for `remaining` polls, then true forever.
struct CancelOnNth {
    remaining: std::cell::Cell<u32>,
}

impl CancelOnNth {
    fn new(remaining: u32) -> CancelOnNth {
        CancelOnNth {
            remaining: std::cell::Cell::new(remaining),
        }
    }
}

impl Cancel for CancelOnNth {
    fn is_cancelled(&self) -> bool {
        let left = self.remaining.get();
        if left == 0 {
            return true;
        }
        self.remaining.set(left - 1);
        false
    }
}

/// A cancellation token that is already cancelled on the first poll.
struct AlwaysCancel;

impl Cancel for AlwaysCancel {
    fn is_cancelled(&self) -> bool {
        true
    }
}

#[test]
fn normalization_is_cancellable() {
    // 7*(10^80+1) / 11*(10^80+1): both parts are 81 and 82 digits and share the 81-digit factor,
    // so the GCD and the reduction are the expensive steps and the canonical result is 7/11.
    let shared = Nat::from_u64(10).pow(80).add(&Nat::from_u64(1));
    let numerator = Int::from_nat(shared.mul(&Nat::from_u64(7)));
    let denominator = shared.mul(&Nat::from_u64(11));
    assert!(
        numerator.magnitude().bit_length() > 200 && denominator.bit_length() > 200,
        "the parts are large enough for the GCD to be the dominant cost"
    );

    // Cancelled before any normalizing work: no partially normalized value is returned.
    assert_eq!(
        Rat::from_parts_with(
            numerator.clone(),
            denominator.clone(),
            &Limits::unlimited(),
            &AlwaysCancel,
        ),
        Err(RationalError::Cancelled)
    );

    // A token that reports false once and true on its next poll must still stop normalization,
    // which proves cancellation is polled more than once across the GCD, not only at the door.
    assert_eq!(
        Rat::from_parts_with(
            numerator.clone(),
            denominator.clone(),
            &Limits::unlimited(),
            &CancelOnNth::new(1),
        ),
        Err(RationalError::Cancelled)
    );

    // With no cancellation the same parts reduce exactly as `from_parts` does.
    let expected = Rat::from_parts(Int::from_i64(7), Nat::from_u64(11)).expect("non-zero");
    assert_eq!(
        Rat::from_parts_with(
            numerator.clone(),
            denominator.clone(),
            &Limits::unlimited(),
            &NeverCancel,
        ),
        Ok(expected)
    );

    // The digit growth of the unreduced parts is preflighted before the GCD runs, and the refusal
    // uses the existing digit-budget category.
    let error = Rat::from_parts_with(numerator, denominator, &Limits::digits(50), &NeverCancel)
        .expect_err("both unreduced parts exceed fifty digits");
    match error {
        RationalError::DigitBudgetExceeded { limit, observed } => {
            assert_eq!(limit, 50);
            assert!(observed > 50, "the estimate must exceed the limit");
        }
        other => panic!("expected DigitBudgetExceeded, got {other}"),
    }

    // The zero denominator keeps its domain error, and the unlimited form reduces exactly like
    // `from_parts`, which remains the infallible-no-cancel form.
    assert_eq!(
        Rat::from_parts_with(
            Int::from_i64(1),
            Nat::zero(),
            &Limits::unlimited(),
            &NeverCancel,
        ),
        Err(RationalError::ZeroDenominator)
    );
    assert_eq!(
        Rat::from_parts_with(
            Int::from_i64(2),
            Nat::from_u64(4),
            &Limits::unlimited(),
            &NeverCancel,
        ),
        Rat::from_parts(Int::from_i64(1), Nat::from_u64(2))
    );
}
