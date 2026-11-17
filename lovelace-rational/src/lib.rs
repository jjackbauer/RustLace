//! The canonical exact rational type for the whole workspace.
//!
//! `lovelace-rational` is the single owner of general-purpose rational arithmetic. Every other
//! crate — `lovelace-real`, `lovelace-symbolics`, `lovelace-systems`, and through them
//! `lovelace-lang` — consumes [`Rat`] instead of carrying its own numerator/denominator type. The
//! architecture policy in `tools/architecture-policy.json` and the checker in
//! `tools/check-architecture.py` enforce that ownership.
//!
//! # Canonical form
//!
//! A [`Rat`] always satisfies:
//!
//! * the denominator is strictly positive and non-zero;
//! * the sign is carried only by the numerator (there is no negative zero);
//! * zero is exactly `0/1`;
//! * numerator and denominator are divided by their full arbitrary-precision GCD, so equal numbers
//!   have identical representations.
//!
//! Normalization uses [`Nat::gcd`] on the full magnitude. It is not limited to operands that fit
//! in `u64`, and it never converts through decimal text: `add`, `sub`, `mul`, `div`, `cmp` and
//! normalization use [`Nat`]/[`Int`] directly. Decimal text appears only at parsing and formatting
//! boundaries ([`Rat::from_decimal`], [`std::fmt::Display`]).
//!
//! # Failures are explicit
//!
//! [`Rat::from_parts`] refuses a zero denominator, [`Rat::checked_div`] refuses division by zero,
//! and [`Rat::checked_pow`] refuses `0` to a negative power. The infallible helpers that remain for
//! source compatibility ([`Rat::div`] returning `Option`, [`Rat::pow_i64`] returning `Option`)
//! propagate the same failures; they never substitute zero.
//!
//! # Resource limits
//!
//! [`Limits`] bounds decimal digit growth (and optionally bit growth) and returns
//! [`RationalError::DigitBudgetExceeded`] *instead of* a partially computed result. A limit is
//! enforced twice: a cheap conservative preflight refuses an operation whose operands already imply
//! the result is over budget, and the materialized result is checked again. The preflight estimate
//! deliberately over-counts, so an operation whose operands are large can be refused even when the
//! reduced result would have fit; each operation's documentation states its estimate.
//!
//! [`Cancel`] is polled between squaring steps of [`Rat::checked_pow`] and around the GCD
//! normalization in [`Rat::from_parts_with`]. [`Limits::unlimited`] is an explicit opt-out; the
//! default used by [`Rat::pow_i64`] is [`DEFAULT_MAX_DIGITS`].
//!
//! # Exactness provenance
//!
//! The [`provenance`] module keeps computation exactness, input fidelity, representation
//! exactness and render precision as four independent claims, and [`Conversion`] makes the loss
//! of a conversion explicit. `lovelace-real` builds the `Rat`↔`Real` conversions on top of it.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

use std::cmp::Ordering;
use std::fmt;

use lovelace_int::Int;
use lovelace_nat::Nat;

pub mod provenance;

pub use provenance::{
    ComputationExactness, Conversion, ExactnessProvenance, InputFidelity, RepresentationExactness,
};

/// The default digit budget used by the infallible [`Rat::pow_i64`] helper.
///
/// It exists so an accidental `2^i64::MAX` is a refusal rather than an allocation attempt; callers
/// that genuinely need more pass [`Limits::unlimited`] to [`Rat::checked_pow`].
pub const DEFAULT_MAX_DIGITS: usize = 1_048_576;

/// The stable, categorized reason a rational operation failed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RationalError {
    /// A constructor was given a zero denominator.
    ZeroDenominator,
    /// A division or a negative power was asked to divide by zero.
    DivisionByZero,
    /// Zero was raised to a negative exponent.
    NegativePowerOfZero,
    /// The exponent cannot be represented by the exact computation (notably `i64::MIN` for a
    /// non-unit base).
    ExponentOutOfRange {
        /// The exponent that was refused.
        exponent: i64,
    },
    /// An exact intermediate exceeded the declared decimal-digit budget.
    DigitBudgetExceeded {
        /// The declared limit.
        limit: usize,
        /// The observed (upper-bound) size.
        observed: usize,
    },
    /// An exact intermediate exceeded the declared bit budget.
    BitBudgetExceeded {
        /// The declared limit.
        limit: u64,
        /// The observed size.
        observed: u64,
    },
    /// The caller's cancellation token fired; no partial result is returned.
    Cancelled,
    /// An arithmetic step that the representation guarantees non-zero failed internally.
    InternalInvariant {
        /// The operation that failed.
        operation: &'static str,
    },
}

impl RationalError {
    /// The stable category, matching the interpreter's frozen vocabulary.
    pub fn category(&self) -> &'static str {
        match self {
            RationalError::ZeroDenominator
            | RationalError::DivisionByZero
            | RationalError::NegativePowerOfZero
            | RationalError::ExponentOutOfRange { .. } => "DomainError",
            RationalError::DigitBudgetExceeded { .. }
            | RationalError::BitBudgetExceeded { .. }
            | RationalError::Cancelled => "BudgetExceeded",
            RationalError::InternalInvariant { .. } => "InternalInvariantFailure",
        }
    }

    /// The stable code a host may match on.
    pub fn code(&self) -> &'static str {
        match self {
            RationalError::ZeroDenominator => "ZeroDenominator",
            RationalError::DivisionByZero => "DivisionByZero",
            RationalError::NegativePowerOfZero => "NegativePowerOfZero",
            RationalError::ExponentOutOfRange { .. } => "ExponentOutOfRange",
            RationalError::DigitBudgetExceeded { .. } => "RationalDigitBudgetExceeded",
            RationalError::BitBudgetExceeded { .. } => "RationalBitBudgetExceeded",
            RationalError::Cancelled => "Cancelled",
            RationalError::InternalInvariant { .. } => "InternalInvariantFailure",
        }
    }
}

impl fmt::Display for RationalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RationalError::ZeroDenominator => {
                f.write_str("A rational denominator must be non-zero.")
            }
            RationalError::DivisionByZero => f.write_str("Cannot divide by zero."),
            RationalError::NegativePowerOfZero => {
                f.write_str("Zero cannot be raised to a negative power.")
            }
            RationalError::ExponentOutOfRange { exponent } => write!(
                f,
                "The exponent {exponent} is outside the exact rational power range."
            ),
            RationalError::DigitBudgetExceeded { limit, observed } => write!(
                f,
                "The exact rational needs {observed} decimal digits, over the {limit}-digit budget."
            ),
            RationalError::BitBudgetExceeded { limit, observed } => write!(
                f,
                "The exact rational needs {observed} bits, over the {limit}-bit budget."
            ),
            RationalError::Cancelled => f.write_str("The computation was cancelled by the caller."),
            RationalError::InternalInvariant { operation } => write!(
                f,
                "The rational kernel hit an internal invariant in {operation}."
            ),
        }
    }
}

impl std::error::Error for RationalError {}

/// A cooperative cancellation check.
///
/// The trait lives here so the kernel can poll long exact computations without depending on a
/// domain crate. The interpreter adapter, the solver and the desktop implement it over their own
/// deadline or flag. It is polled between squaring steps of [`Rat::checked_pow`] and before and
/// after the GCD normalization in [`Rat::from_parts_with`].
pub trait Cancel {
    /// True when the caller has cancelled the computation.
    fn is_cancelled(&self) -> bool;
}

/// A cancellation check that never fires.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NeverCancel;

impl Cancel for NeverCancel {
    fn is_cancelled(&self) -> bool {
        false
    }
}

/// An explicit decimal-digit and bit growth budget for exact intermediates.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    max_digits: Option<usize>,
    max_bits: Option<u64>,
}

impl Limits {
    /// No growth bound. A caller that chooses this accepts unbounded memory growth.
    pub const fn unlimited() -> Limits {
        Limits {
            max_digits: None,
            max_bits: None,
        }
    }

    /// Bounds the numerator-plus-denominator decimal digits.
    pub const fn digits(max_digits: usize) -> Limits {
        Limits {
            max_digits: Some(max_digits),
            max_bits: None,
        }
    }

    /// Bounds the numerator-plus-denominator bit length.
    pub const fn bits(max_bits: u64) -> Limits {
        Limits {
            max_digits: None,
            max_bits: Some(max_bits),
        }
    }

    /// Adds a decimal-digit bound to a limits value.
    pub const fn with_digits(mut self, max_digits: usize) -> Limits {
        self.max_digits = Some(max_digits);
        self
    }

    /// The configured decimal-digit bound, when any.
    pub const fn max_digits(&self) -> Option<usize> {
        self.max_digits
    }

    /// Refuses an estimated numerator-plus-denominator digit count.
    ///
    /// This is the cheap preflight counterpart of [`Limits::check`]: the caller derives an estimate
    /// from operand sizes instead of materializing a [`Rat`]. The estimate is expected to be an
    /// upper bound of the result's digit count; it may over-count, which refuses an operation
    /// slightly early but never lets an over-budget result through. A bit-only budget never
    /// refuses here, because a digit count says nothing sound about a bit count.
    ///
    /// Returns [`RationalError::DigitBudgetExceeded`] with the estimate as `observed`.
    pub fn check_digits(&self, observed: usize) -> Result<(), RationalError> {
        if let Some(limit) = self.max_digits {
            if observed > limit {
                return Err(RationalError::DigitBudgetExceeded { limit, observed });
            }
        }
        Ok(())
    }

    /// Refuses `value` when it is over either configured bound.
    pub fn check(&self, value: &Rat) -> Result<(), RationalError> {
        self.check_digits(value.decimal_digits())?;
        if let Some(limit) = self.max_bits {
            let observed = value.bit_length();
            if observed > limit {
                return Err(RationalError::BitBudgetExceeded { limit, observed });
            }
        }
        Ok(())
    }
}

impl Default for Limits {
    fn default() -> Self {
        Limits::digits(DEFAULT_MAX_DIGITS)
    }
}

/// An exact rational number in canonical form.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Rat {
    num: Int,
    den: Nat,
}

impl Rat {
    /// Zero, canonically `0/1`.
    pub fn zero() -> Rat {
        Rat {
            num: Int::zero(),
            den: Nat::from_u64(1),
        }
    }

    /// One, canonically `1/1`.
    pub fn one() -> Rat {
        Rat::from_i64(1)
    }

    /// An `i64` as a rational.
    pub fn from_i64(value: i64) -> Rat {
        Rat {
            num: Int::from_i64(value),
            den: Nat::from_u64(1),
        }
    }

    /// A `u64` as a rational.
    pub fn from_u64(value: u64) -> Rat {
        Rat {
            num: Int::from_nat(Nat::from_u64(value)),
            den: Nat::from_u64(1),
        }
    }

    /// A natural as a rational; lossless.
    pub fn from_nat(value: &Nat) -> Rat {
        Rat {
            num: Int::from_nat(value.clone()),
            den: Nat::from_u64(1),
        }
    }

    /// An integer as a rational; lossless.
    pub fn from_int(value: &Int) -> Rat {
        Rat {
            num: value.clone(),
            den: Nat::from_u64(1),
        }
    }

    /// Builds a rational from a numerator and a denominator.
    ///
    /// The result is fully reduced; zero is `0/1`. A zero denominator is
    /// [`RationalError::ZeroDenominator`]. This is the infallible-no-cancel form: it takes no
    /// [`Limits`] and polls no [`Cancel`] token, so it cannot refuse an over-budget normalization
    /// and cannot be stopped while the GCD runs. [`Rat::from_parts_with`] is the budgeted and
    /// cancellable form; there is no public way to build a [`Rat`] with a zero or negative
    /// denominator.
    pub fn from_parts(num: Int, den: Nat) -> Result<Rat, RationalError> {
        Rat::canonicalize(num, den)
    }

    /// Builds a rational from a numerator and a denominator under an explicit growth budget and
    /// cancellation check.
    ///
    /// The unreduced numerator-plus-denominator digit count is preflighted **before** the GCD runs;
    /// the caller's [`Cancel`] token is polled **before and after** the GCD normalization; and the
    /// normalized value is then checked against **both** configured bounds with [`Limits::check`],
    /// so a bit-only budget is enforced on the value that is returned. A cancelled or over-budget
    /// call returns [`RationalError::Cancelled`], [`RationalError::DigitBudgetExceeded`] or
    /// [`RationalError::BitBudgetExceeded`] and no value: a partially normalized rational is never
    /// returned. [`Nat::gcd`] itself has no cancellation hook, so the poll granularity is the
    /// normalization as a whole rather than one GCD iteration.
    ///
    /// The preflight over-counts by construction: it measures the unreduced parts. A fraction whose
    /// parts exceed the digit budget is refused even when its reduced value would fit (for example
    /// `10^60 / 10^60` under a sixty-digit budget), which is the price of refusing before the GCD.
    /// The final check measures the reduced value, so a bit-only budget is refused there and an
    /// already-reduced input is still checked. When a call is both cancelled and over budget,
    /// [`RationalError::Cancelled`] is reported: the caller has already abandoned the result. A zero
    /// numerator is `0/1` and is returned after the first cancellation poll without running the
    /// GCD, but it is still checked against both bounds: `0/1` has bit length one and two
    /// upper-bound decimal digits. A zero denominator is [`RationalError::ZeroDenominator`], which
    /// is reported before the budget preflight and before the cancellation poll.
    pub fn from_parts_with(
        num: Int,
        den: Nat,
        limits: &Limits,
        cancel: &dyn Cancel,
    ) -> Result<Rat, RationalError> {
        if den.is_zero() {
            return Err(RationalError::ZeroDenominator);
        }
        if cancel.is_cancelled() {
            return Err(RationalError::Cancelled);
        }
        if num.is_zero() {
            let value = Rat::zero();
            limits.check(&value)?;
            return Ok(value);
        }
        limits.check_digits(
            decimal_digits_upper_bound(num.magnitude())
                .saturating_add(decimal_digits_upper_bound(&den)),
        )?;
        let divisor = num.magnitude().gcd(&den);
        if cancel.is_cancelled() {
            return Err(RationalError::Cancelled);
        }
        let value = Rat::reduce_by(num, den, &divisor)?;
        limits.check(&value)?;
        Ok(value)
    }

    /// Compatibility spelling of [`Rat::from_parts`].
    pub fn new(num: Int, den: Nat) -> Result<Rat, RationalError> {
        Rat::from_parts(num, den)
    }

    /// Parses decimal text such as `3`, `-1.25`, `0.125`, or a `numerator/denominator` pair.
    ///
    /// Parsing is an explicit text boundary; arithmetic never goes through this path.
    pub fn from_decimal(text: &str) -> Option<Rat> {
        let text = text.trim();
        if text.is_empty() {
            return None;
        }
        if let Some((numerator, denominator)) = text.split_once('/') {
            let num = Int::from_decimal(numerator.trim())?;
            let den_text = denominator.trim();
            let negative_den = den_text.starts_with('-');
            let den = Nat::from_decimal(den_text.trim_start_matches(['-', '+']))?;
            if den.is_zero() {
                return None;
            }
            let num = if negative_den { num.negate() } else { num };
            return Rat::from_parts(num, den).ok();
        }

        let (negative, unsigned) = match text.strip_prefix('-') {
            Some(rest) => (true, rest),
            None => (false, text.strip_prefix('+').unwrap_or(text)),
        };
        let (whole, fraction) = match unsigned.split_once('.') {
            Some((whole, fraction)) => (whole, fraction),
            None => (unsigned, ""),
        };
        if whole.is_empty() && fraction.is_empty() {
            return None;
        }
        if !whole.bytes().all(|byte| byte.is_ascii_digit())
            || !fraction.bytes().all(|byte| byte.is_ascii_digit())
        {
            return None;
        }
        let mut digits = String::with_capacity(whole.len() + fraction.len());
        digits.push_str(whole);
        digits.push_str(fraction);
        if digits.is_empty() {
            return None;
        }
        let magnitude = Nat::from_decimal(&digits)?;
        let denominator = Nat::from_u64(10).pow(fraction.len() as u64);
        let numerator = if negative {
            Int::from_nat(magnitude).negate()
        } else {
            Int::from_nat(magnitude)
        };
        Rat::from_parts(numerator, denominator).ok()
    }

    fn canonicalize(num: Int, den: Nat) -> Result<Rat, RationalError> {
        if den.is_zero() {
            return Err(RationalError::ZeroDenominator);
        }
        if num.is_zero() {
            return Ok(Rat::zero());
        }
        let divisor = num.magnitude().gcd(&den);
        Rat::reduce_by(num, den, &divisor)
    }

    /// Divides both parts by the already-computed `divisor` and returns the canonical value.
    ///
    /// The caller has established the non-zero denominator and the non-zero numerator, so the only
    /// possible failures are the representation invariants of the divider.
    fn reduce_by(num: Int, den: Nat, divisor: &Nat) -> Result<Rat, RationalError> {
        if divisor.is_one() {
            return Ok(Rat { num, den });
        }
        let (reduced_num, _) = num
            .div_rem(divisor)
            .ok_or(RationalError::InternalInvariant {
                operation: "normalize numerator",
            })?;
        let (reduced_den, _) = den
            .div_rem(divisor)
            .ok_or(RationalError::InternalInvariant {
                operation: "normalize denominator",
            })?;
        Ok(Rat {
            num: reduced_num,
            den: reduced_den,
        })
    }

    /// Builds a value that the caller has already cross-cancelled into canonical form.
    ///
    /// Used by multiplication and division after their cross-cancellation step; the debug
    /// assertions are the proof obligation, and they disappear from the release build.
    fn from_reduced(num: Int, den: Nat) -> Rat {
        debug_assert!(
            !den.is_zero(),
            "a reduced rational has a non-zero denominator"
        );
        debug_assert!(!num.is_zero() || den.is_one(), "zero is canonically 0/1");
        debug_assert!(
            num.is_zero() || num.magnitude().gcd(&den).is_one(),
            "the cross-cancelled parts are coprime"
        );
        Rat { num, den }
    }

    /// The numerator; the sign lives here.
    pub fn numerator(&self) -> &Int {
        &self.num
    }

    /// The denominator; strictly positive.
    pub fn denominator(&self) -> &Nat {
        &self.den
    }

    /// True when the value is zero.
    pub fn is_zero(&self) -> bool {
        self.num.is_zero()
    }

    /// True when the value is one.
    pub fn is_one(&self) -> bool {
        self.den.is_one() && !self.num.is_negative() && self.num.magnitude().is_one()
    }

    /// True when the value is negative.
    pub fn is_negative(&self) -> bool {
        self.num.is_negative()
    }

    /// True when the value is strictly positive.
    pub fn is_positive(&self) -> bool {
        !self.num.is_negative() && !self.num.is_zero()
    }

    /// True when the denominator is one.
    pub fn is_integer(&self) -> bool {
        self.den.is_one()
    }

    /// The integer value when the rational is integral and fits `i64`.
    pub fn to_i64(&self) -> Option<i64> {
        if !self.is_integer() {
            return None;
        }
        let magnitude = self.num.magnitude().to_u64()?;
        if self.num.is_negative() {
            if magnitude == 1u64 << 63 {
                return Some(i64::MIN);
            }
            i64::try_from(magnitude).ok().map(|value| -value)
        } else {
            i64::try_from(magnitude).ok()
        }
    }

    /// The integer value when the rational is integral and non-negative and fits `u64`.
    pub fn to_u64(&self) -> Option<u64> {
        if !self.is_integer() || self.num.is_negative() {
            return None;
        }
        self.num.magnitude().to_u64()
    }

    /// The exact integer value, when the rational is integral.
    pub fn to_int_exact(&self) -> Option<Int> {
        if self.is_integer() {
            Some(self.num.clone())
        } else {
            None
        }
    }

    /// The numerator plus denominator decimal-digit count, as a cheap upper bound.
    ///
    /// The bound is derived from the binary length (`bits * 30103 / 100000 + 1` per part), so a
    /// budget check never renders decimal text. It may over-count by a digit or two, which makes
    /// a resource limit refuse slightly early rather than late.
    pub fn decimal_digits(&self) -> usize {
        decimal_digits_upper_bound(self.num.magnitude())
            .saturating_add(decimal_digits_upper_bound(&self.den))
    }

    /// The numerator-plus-denominator bit length.
    pub fn bit_length(&self) -> u64 {
        self.num
            .magnitude()
            .bit_length()
            .saturating_add(self.den.bit_length())
    }

    /// Negation.
    pub fn neg(&self) -> Rat {
        Rat {
            num: self.num.negate(),
            den: self.den.clone(),
        }
    }

    /// Absolute value.
    pub fn abs(&self) -> Rat {
        if self.is_negative() {
            self.neg()
        } else {
            self.clone()
        }
    }

    /// Addition.
    pub fn add(&self, other: &Rat) -> Rat {
        if self.is_zero() {
            return other.clone();
        }
        if other.is_zero() {
            return self.clone();
        }
        // Use the least common multiple of the denominators rather than their product.
        let common = self.den.gcd(&other.den);
        let left_scale = self
            .den
            .div_rem(&common)
            .map(|(quotient, _)| quotient)
            .expect("a non-zero gcd divides its dividend");
        let right_scale = other
            .den
            .div_rem(&common)
            .map(|(quotient, _)| quotient)
            .expect("a non-zero gcd divides its dividend");
        let left = self.num.mul(&Int::from_nat(right_scale.clone()));
        let right = other.num.mul(&Int::from_nat(left_scale));
        let den = self.den.mul(&right_scale);
        Rat::canonicalize(left.add(&right), den)
            .expect("the sum of two rationals has a non-zero denominator")
    }

    /// Addition with an explicit growth budget.
    ///
    /// The budget is enforced twice: a conservative preflight runs before [`Rat::add`] forms
    /// `a*d + c*b` over the common denominator, then the materialized result is checked. The
    /// preflight estimate is
    /// `max(digits(a) + digits(d), digits(c) + digits(b)) + 1 + digits(b) + digits(d)` for
    /// `a/b + c/d`, which bounds the carry and the unreduced denominator. It over-counts when the
    /// denominators share a large factor (the real sum uses their least common multiple), so a
    /// sum that would have fit can be refused when its operands are large.
    pub fn add_with(&self, other: &Rat, limits: &Limits) -> Result<Rat, RationalError> {
        limits.check_digits(addition_digits_upper_bound(self, other))?;
        let value = self.add(other);
        limits.check(&value)?;
        Ok(value)
    }

    /// Subtraction.
    pub fn sub(&self, other: &Rat) -> Rat {
        self.add(&other.neg())
    }

    /// Subtraction with an explicit growth budget.
    ///
    /// Subtraction is addition of the negation, so it uses the same conservative preflight as
    /// [`Rat::add_with`] before the numerators are combined.
    pub fn sub_with(&self, other: &Rat, limits: &Limits) -> Result<Rat, RationalError> {
        limits.check_digits(addition_digits_upper_bound(self, other))?;
        let value = self.sub(other);
        limits.check(&value)?;
        Ok(value)
    }

    /// Multiplication, with cross-cancellation before the products are formed.
    pub fn mul(&self, other: &Rat) -> Rat {
        if self.is_zero() || other.is_zero() {
            return Rat::zero();
        }
        // `(a/b) * (c/d)`: cancel `gcd(a, d)` and `gcd(c, b)` first so intermediate operands do
        // not grow by a factor that the result immediately removes.
        let first = self.num.magnitude().gcd(&other.den);
        let second = other.num.magnitude().gcd(&self.den);
        let (left_num, right_den) = cancel_pair(&self.num, &other.den, &first);
        let (right_num, left_den) = cancel_pair(&other.num, &self.den, &second);
        Rat::from_reduced(left_num.mul(&right_num), left_den.mul(&right_den))
    }

    /// Multiplication with an explicit growth budget.
    ///
    /// The budget is enforced twice: the preflight runs before [`Rat::mul`] computes the two
    /// numerator and denominator products, then the materialized result is checked. The estimate
    /// is `self.decimal_digits() + other.decimal_digits()`: each result part is at most the product
    /// of one numerator with one denominator, so its digit count is at most the sum of the factors'
    /// counts, and the cross-cancellation only shrinks the parts. It over-counts by up to the two
    /// cancelled factors, so a product that would have fit can be refused when its operands are
    /// large (a value multiplied by its own reciprocal is refused under a tight budget even though
    /// the exact result is one). A zero operand is `0/1`, which needs two digits, and forms no
    /// product.
    pub fn mul_with(&self, other: &Rat, limits: &Limits) -> Result<Rat, RationalError> {
        limits.check_digits(multiplication_digits_upper_bound(self, other))?;
        let value = self.mul(other);
        limits.check(&value)?;
        Ok(value)
    }

    /// Division; `None` when the divisor is zero.
    pub fn div(&self, other: &Rat) -> Option<Rat> {
        self.checked_div(other).ok()
    }

    /// Division, refusing a zero divisor explicitly.
    pub fn checked_div(&self, other: &Rat) -> Result<Rat, RationalError> {
        if other.is_zero() {
            return Err(RationalError::DivisionByZero);
        }
        if self.is_zero() {
            return Ok(Rat::zero());
        }
        // `(a/b) / (c/d) = (a*d) / (b*c)`: cancel `gcd(a, c)` and `gcd(d, b)` first, and carry
        // the divisor's sign onto the result numerator.
        let first = self.num.magnitude().gcd(other.num.magnitude());
        let second = other.den.gcd(&self.den);
        let (left_num, right_den) = cancel_pair(&self.num, other.num.magnitude(), &first);
        let (right_num, left_den) =
            cancel_pair(&Int::from_nat(other.den.clone()), &self.den, &second);
        let numerator = if other.is_negative() {
            left_num.mul(&right_num).negate()
        } else {
            left_num.mul(&right_num)
        };
        Ok(Rat::from_reduced(numerator, left_den.mul(&right_den)))
    }

    /// Division with an explicit growth budget.
    ///
    /// A zero divisor is still [`RationalError::DivisionByZero`], reported by
    /// [`Rat::checked_div`] before any product exists. Otherwise the budget is enforced twice: the
    /// preflight runs before the `a*d` and `b*c` products are formed, then the materialized result
    /// is checked. The estimate is the same `self.decimal_digits() + other.decimal_digits()` bound
    /// as [`Rat::mul_with`], because `(a/b) / (c/d) = (a*d) / (b*c)` pairs one numerator with one
    /// denominator. It over-counts for the same reason: the cross-cancellation is not accounted
    /// for, so a quotient that would have fit can be refused when its operands are large.
    pub fn checked_div_with(&self, other: &Rat, limits: &Limits) -> Result<Rat, RationalError> {
        if !other.is_zero() {
            limits.check_digits(multiplication_digits_upper_bound(self, other))?;
        }
        let value = self.checked_div(other)?;
        limits.check(&value)?;
        Ok(value)
    }

    /// Exact integer power with the default [`Limits`], refusing rather than substituting zero.
    pub fn pow_i64(&self, exponent: i64) -> Option<Rat> {
        self.checked_pow(exponent, &Limits::default(), &NeverCancel)
            .ok()
    }

    /// Exact integer power with explicit limits and cancellation.
    ///
    /// Positive exponents use exponentiation by squaring, after a preflight that refuses a huge
    /// exponent whose result provably exceeds the digit or bit budget (see [`Rat::pow_u64`]). A
    /// negative exponent computes the reciprocal of the positive power. `0^0` is `1` by the
    /// conventional empty-product rule; `0` to a negative power is
    /// [`RationalError::NegativePowerOfZero`]. `i64::MIN` is handled without overflowing: unit bases
    /// return their exact value, every other base is refused with
    /// [`RationalError::ExponentOutOfRange`] because the exact result cannot be represented by the
    /// supported computation.
    pub fn checked_pow(
        &self,
        exponent: i64,
        limits: &Limits,
        cancel: &dyn Cancel,
    ) -> Result<Rat, RationalError> {
        if exponent == 0 {
            return Ok(Rat::one());
        }
        if exponent > 0 {
            return self.pow_u64(exponent as u64, limits, cancel);
        }
        if self.is_zero() {
            return Err(RationalError::NegativePowerOfZero);
        }
        let magnitude = exponent.unsigned_abs();
        if self.is_one() {
            return Ok(Rat::one());
        }
        let negative_one = Rat::from_i64(-1);
        if self == &negative_one {
            return Ok(if magnitude % 2 == 0 {
                Rat::one()
            } else {
                negative_one
            });
        }
        if exponent == i64::MIN {
            // |i64::MIN| is 2^63; for any other base the exact result cannot be materialized.
            return Err(RationalError::ExponentOutOfRange { exponent });
        }
        let positive = self.pow_u64(magnitude, limits, cancel)?;
        Rat::one().checked_div(&positive)
    }

    /// Exact non-negative integer power with explicit limits and cancellation.
    ///
    /// Before the first squaring, a digit budget is preflighted from the exponent alone: the bit
    /// length of each part gives a sound lower bound on the result's digits, and a result that
    /// provably exceeds the budget is refused immediately instead of after the first over-budget
    /// intermediate. The preflight only refuses when the lower bound itself is over budget, so a
    /// small legitimate power whose exact result fits is never rejected. A bit budget is preflighted
    /// the same way. A zero result (`0^positive`) is returned without any preflight.
    pub fn pow_u64(
        &self,
        mut exponent: u64,
        limits: &Limits,
        cancel: &dyn Cancel,
    ) -> Result<Rat, RationalError> {
        if exponent == 0 {
            return Ok(Rat::one());
        }
        if self.is_zero() {
            return Ok(Rat::zero());
        }
        check_power_preflight(self, exponent, limits)?;
        let mut base = self.clone();
        let mut accumulator = Rat::one();
        loop {
            if cancel.is_cancelled() {
                return Err(RationalError::Cancelled);
            }
            if exponent & 1 == 1 {
                accumulator = accumulator.mul(&base);
                limits.check(&accumulator)?;
            }
            exponent >>= 1;
            if exponent == 0 {
                break;
            }
            base = base.mul(&base);
            limits.check(&base)?;
        }
        Ok(accumulator)
    }
}

/// An upper bound on the decimal digits of `left + right`.
///
/// The unreduced sum is `(a*d + c*b) / (b*d)`: each product's digit count is at most the sum of its
/// factors' counts, the addition can carry one digit, and the reduction only shrinks both parts.
/// The denominator GCD is deliberately not computed — the preflight must stay cheap — so the bound
/// over-counts when the denominators share a factor. Zero operands make [`Rat::add`] return the
/// other value unchanged, so their exact digit count is used instead.
fn addition_digits_upper_bound(left: &Rat, right: &Rat) -> usize {
    if left.is_zero() {
        return right.decimal_digits();
    }
    if right.is_zero() {
        return left.decimal_digits();
    }
    let left_num = decimal_digits_upper_bound(left.num.magnitude());
    let left_den = decimal_digits_upper_bound(&left.den);
    let right_num = decimal_digits_upper_bound(right.num.magnitude());
    let right_den = decimal_digits_upper_bound(&right.den);
    // `digits(a*d)` and `digits(c*b)`, plus one digit for the carry.
    let numerator = left_num
        .saturating_add(right_den)
        .max(right_num.saturating_add(left_den))
        .saturating_add(1);
    let denominator = left_den.saturating_add(right_den);
    numerator.saturating_add(denominator)
}

/// An upper bound on the decimal digits of `left * right` and of `left / right`.
///
/// Both operations pair one numerator with one denominator, so each result part is at most the
/// product of two operand parts and its digit count is at most the sum of their counts. The
/// cross-cancellation is not accounted for, which is where the over-count comes from. A zero
/// operand is the canonical `0/1`, two digits, and forms no product.
fn multiplication_digits_upper_bound(left: &Rat, right: &Rat) -> usize {
    if left.is_zero() || right.is_zero() {
        return 2;
    }
    left.decimal_digits().saturating_add(right.decimal_digits())
}

/// Refuses a power whose exponent alone proves it over budget, before the first squaring.
///
/// For a base with bit length `B`, `base >= 2^(B-1)`, so `base^exponent >= 2^((B-1)*exponent)`;
/// that gives a sound lower bound on the result's digits and bits. A refusal needs the lower bound
/// to exceed the limit, which proves the exact result is over budget and keeps a fitting power
/// accepted. The reported `observed` is the matching upper bound, so
/// [`RationalError::DigitBudgetExceeded::observed`] stays an upper bound as documented.
fn check_power_preflight(base: &Rat, exponent: u64, limits: &Limits) -> Result<(), RationalError> {
    let num_bits = base.num.magnitude().bit_length();
    let den_bits = base.den.bit_length();
    if let Some(limit) = limits.max_digits() {
        let lower = pow_decimal_digits_lower_bound(num_bits, exponent)
            .saturating_add(pow_decimal_digits_lower_bound(den_bits, exponent));
        if lower > limit {
            let observed = pow_decimal_digits_upper_bound(num_bits, exponent)
                .saturating_add(pow_decimal_digits_upper_bound(den_bits, exponent));
            return Err(RationalError::DigitBudgetExceeded { limit, observed });
        }
    }
    if let Some(limit) = limits.max_bits {
        let lower = pow_bits_lower_bound(num_bits, exponent)
            .saturating_add(pow_bits_lower_bound(den_bits, exponent));
        if lower > u128::from(limit) {
            let observed = pow_bits_upper_bound(num_bits, exponent)
                .saturating_add(pow_bits_upper_bound(den_bits, exponent));
            return Err(RationalError::BitBudgetExceeded {
                limit,
                observed: u64::try_from(observed).unwrap_or(u64::MAX),
            });
        }
    }
    Ok(())
}

/// `30103 / 100000` is strictly greater than `log10(2)`, so a bit count scaled by it over-estimates
/// the decimal digits.
const LOG10_2_UPPER_NUMERATOR: u128 = 30_103;

/// `30102 / 100000` is strictly less than `log10(2)`, so a bit count scaled by it under-estimates
/// the decimal digits.
const LOG10_2_LOWER_NUMERATOR: u128 = 30_102;

/// A sound lower bound on the decimal digits of `value^exponent`, from `value`'s bit length.
fn pow_decimal_digits_lower_bound(bits: u64, exponent: u64) -> usize {
    let bit_floor = (bits.saturating_sub(1) as u128).saturating_mul(u128::from(exponent));
    let digits = bit_floor
        .checked_mul(LOG10_2_LOWER_NUMERATOR)
        .map(|scaled| scaled / 100_000)
        .unwrap_or(u128::MAX)
        .saturating_add(1);
    usize::try_from(digits).unwrap_or(usize::MAX)
}

/// A sound upper bound on the decimal digits of `value^exponent`, from `value`'s bit length.
fn pow_decimal_digits_upper_bound(bits: u64, exponent: u64) -> usize {
    if bits <= 1 {
        // A bit length of one means the value is exactly one, whose every power is one digit.
        return 1;
    }
    let bit_ceiling = (bits as u128).saturating_mul(u128::from(exponent));
    let digits = bit_ceiling
        .checked_mul(LOG10_2_UPPER_NUMERATOR)
        .map(|scaled| scaled / 100_000)
        .unwrap_or(u128::MAX)
        .saturating_add(1);
    usize::try_from(digits).unwrap_or(usize::MAX)
}

/// A sound lower bound on the bit length of `value^exponent`, from `value`'s bit length.
fn pow_bits_lower_bound(bits: u64, exponent: u64) -> u128 {
    if bits == 0 || exponent == 0 {
        return 0;
    }
    (bits.saturating_sub(1) as u128)
        .saturating_mul(u128::from(exponent))
        .saturating_add(1)
}

/// A sound upper bound on the bit length of `value^exponent`, from `value`'s bit length.
fn pow_bits_upper_bound(bits: u64, exponent: u64) -> u128 {
    if bits <= 1 {
        // A bit length of one means the value is exactly one, whose every power has bit length one.
        return 1;
    }
    (bits as u128).saturating_mul(u128::from(exponent))
}

/// Divides both parts of a numerator/denominator pair by a common divisor when it is not one.
fn cancel_pair(num: &Int, den: &Nat, divisor: &Nat) -> (Int, Nat) {
    if divisor.is_one() {
        return (num.clone(), den.clone());
    }
    let reduced_num = num
        .div_rem(divisor)
        .map(|(quotient, _)| quotient)
        .expect("a non-zero gcd divides its dividend");
    let reduced_den = den
        .div_rem(divisor)
        .map(|(quotient, _)| quotient)
        .expect("a non-zero gcd divides its divisor");
    (reduced_num, reduced_den)
}

/// `ceil(bits * log10(2)) + 1` as a cheap upper bound on the decimal digits of a natural.
fn decimal_digits_upper_bound(value: &Nat) -> usize {
    let bits = value.bit_length();
    if bits == 0 {
        return 1;
    }
    ((bits as u128 * LOG10_2_UPPER_NUMERATOR) / 100_000) as usize + 1
}

impl Default for Rat {
    fn default() -> Self {
        Rat::zero()
    }
}

impl Ord for Rat {
    fn cmp(&self, other: &Self) -> Ordering {
        if self.den == other.den {
            return self.num.cmp(&other.num);
        }
        if self.is_integer() && other.is_integer() {
            return self.num.cmp(&other.num);
        }
        // Cross-multiplication uses Int/Nat directly; no decimal text is involved.
        let left = self.num.mul(&Int::from_nat(other.den.clone()));
        let right = other.num.mul(&Int::from_nat(self.den.clone()));
        left.cmp(&right)
    }
}

impl PartialOrd for Rat {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl fmt::Display for Rat {
    /// Renders an integral value without a denominator and every other value as `n/d`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_integer() {
            return f.write_str(&self.num.to_decimal());
        }
        write!(f, "{}/{}", self.num.to_decimal(), self.den.to_decimal())
    }
}

impl From<i64> for Rat {
    fn from(value: i64) -> Rat {
        Rat::from_i64(value)
    }
}

impl From<u64> for Rat {
    fn from(value: u64) -> Rat {
        Rat::from_u64(value)
    }
}

impl From<&Int> for Rat {
    fn from(value: &Int) -> Rat {
        Rat::from_int(value)
    }
}

impl From<&Nat> for Rat {
    fn from(value: &Nat) -> Rat {
        Rat::from_nat(value)
    }
}

impl std::str::FromStr for Rat {
    type Err = RationalError;

    /// Parses decimal text or a `numerator/denominator` pair.
    fn from_str(text: &str) -> Result<Self, Self::Err> {
        Rat::from_decimal(text).ok_or(RationalError::ZeroDenominator)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rat(num: i64, den: u64) -> Rat {
        Rat::from_parts(Int::from_i64(num), Nat::from_u64(den)).expect("non-zero denominator")
    }

    fn assert_canonical(value: &Rat) {
        assert!(!value.denominator().is_zero(), "denominator is non-zero");
        assert!(!value.numerator().is_zero() || value.denominator().is_one());
        assert_eq!(
            value.numerator().magnitude().gcd(value.denominator()),
            Nat::from_u64(1),
            "numerator and denominator are coprime: {value}"
        );
    }

    #[test]
    fn canonical_form_is_enforced() {
        assert_eq!(rat(2, 4), rat(1, 2));
        assert_eq!(rat(-2, 4), rat(-1, 2));
        assert_eq!(rat(0, 9), Rat::zero());
        assert_eq!(Rat::zero().denominator(), &Nat::from_u64(1));
        assert_canonical(&rat(-6, 15));
        assert_eq!(rat(-6, 15).to_string(), "-2/5");
        assert_eq!(rat(6, 3).to_string(), "2");
    }

    #[test]
    fn signed_division_regression() {
        // The mandatory regression: (1/2) / (-1/4) = -2, exactly.
        let result = rat(1, 2)
            .checked_div(&rat(-1, 4))
            .expect("non-zero divisor");
        assert_eq!(result, Rat::from_i64(-2));
        assert_eq!(result.to_string(), "-2");
        assert_canonical(&result);
    }

    #[test]
    fn signed_division_does_not_lose_the_sign() {
        assert_eq!(rat(1, 2).div(&rat(-1, 4)), Some(rat(-2, 1)));
        assert_eq!(rat(-1, 2).div(&rat(-1, 4)), Some(rat(2, 1)));
        assert_eq!(rat(-1, 2).div(&rat(1, 4)), Some(rat(-2, 1)));
        assert_eq!(rat(1, 2).div(&rat(1, 4)), Some(rat(2, 1)));
    }

    #[test]
    fn large_normalization_is_not_limited_to_u64() {
        // 10^30 is far outside u64; both parts share a factor of 2, and the denominator also
        // carries factors that cancel against the numerator.
        let ten30 = Nat::from_u64(1).mul_pow10(30);
        let numerator = Int::from_nat(ten30.add(&Nat::from_u64(4)));
        let denominator = ten30.add(&Nat::from_u64(6));
        let value = Rat::from_parts(numerator, denominator).expect("non-zero");
        assert_eq!(
            value.numerator(),
            &Int::from_i64(1).mul(&Int::from_nat(
                ten30
                    .add(&Nat::from_u64(4))
                    .div_rem(&Nat::from_u64(2))
                    .expect("2")
                    .0
            ))
        );
        assert_eq!(
            value.denominator(),
            &ten30
                .add(&Nat::from_u64(6))
                .div_rem(&Nat::from_u64(2))
                .expect("2")
                .0
        );
        assert_canonical(&value);

        // A GCD that neither operand can express in u64: (2^200 + 2^100) / (2^200 - 2^100).
        // The shared factor is 2^100 (the two odd cofactors are consecutive-ish odd numbers and
        // coprime), and the reduced denominator still exceeds u64.
        let big = Nat::from_u64(2).pow(200);
        let shared = Nat::from_u64(2).pow(100);
        let num = Int::from_nat(big.add(&shared));
        let den = big.sub(&shared).expect("bigger");
        let value = Rat::from_parts(num, den).expect("non-zero");
        assert!(value.denominator().to_u64().is_none());
        assert_canonical(&value);
        // The two parts share 2^100, so the fully reduced result must have divided it out.
        assert_eq!(
            value.denominator(),
            &Nat::from_u64(2)
                .pow(100)
                .sub(&Nat::from_u64(1))
                .expect("bigger")
        );
    }

    #[test]
    fn arithmetic_is_exact_and_reduced() {
        assert_eq!(rat(1, 3).add(&rat(1, 6)), rat(1, 2));
        assert_eq!(rat(1, 2).add(&rat(1, 2)), Rat::one());
        assert_eq!(rat(1, 3).sub(&rat(1, 3)), Rat::zero());
        assert_eq!(rat(99991, 99989).mul(&rat(99989, 99991)), Rat::one());
        assert_eq!(rat(1, 3).add(&rat(1, 6)).denominator(), &Nat::from_u64(2));
        // Cross-cancellation keeps the intermediate small: a/b * b/a is 1, not b*b / a*a.
        let product = rat(123456789, 987654321).mul(&rat(987654321, 123456789));
        assert_eq!(product, Rat::one());
        assert_eq!(product.decimal_digits(), 2);
    }

    #[test]
    fn division_and_constructor_safety() {
        assert_eq!(
            Rat::from_parts(Int::from_i64(1), Nat::zero()),
            Err(RationalError::ZeroDenominator)
        );
        assert_eq!(
            rat(1, 2).checked_div(&Rat::zero()),
            Err(RationalError::DivisionByZero)
        );
        assert_eq!(rat(1, 2).div(&Rat::zero()), None);
        assert_eq!(
            Rat::zero().checked_pow(-1, &Limits::default(), &NeverCancel),
            Err(RationalError::NegativePowerOfZero)
        );
        assert_eq!(Rat::zero().pow_i64(-1), None);
    }

    #[test]
    fn exponentiation_handles_the_boundaries() {
        assert_eq!(rat(2, 3).pow_i64(0), Some(Rat::one()));
        assert_eq!(rat(2, 3).pow_i64(3), Some(rat(8, 27)));
        assert_eq!(rat(2, 3).pow_i64(-3), Some(rat(27, 8)));
        assert_eq!(rat(1, 1).pow_i64(i64::MIN), Some(Rat::one()));
        assert_eq!(rat(-1, 1).pow_i64(i64::MIN), Some(Rat::one()));
        assert_eq!(rat(-1, 1).pow_i64(i64::MIN + 1), Some(rat(-1, 1)));
        assert_eq!(rat(2, 1).pow_i64(i64::MIN), None);
        // (1/2)^i64::MIN = 2^(2^63), which is exact but not representable here either.
        assert_eq!(rat(1, 2).pow_i64(i64::MIN), None);
        assert_eq!(
            rat(2, 1).checked_pow(i64::MIN, &Limits::unlimited(), &NeverCancel),
            Err(RationalError::ExponentOutOfRange { exponent: i64::MIN })
        );
    }

    #[test]
    fn exponentiation_is_by_squaring_and_budgeted() {
        let limits = Limits::digits(10);
        let error = rat(2, 1)
            .checked_pow(1_000_000, &limits, &NeverCancel)
            .expect_err("the result is far over ten digits");
        assert_eq!(error.category(), "BudgetExceeded");
        assert!(matches!(
            error,
            RationalError::DigitBudgetExceeded { limit: 10, .. }
        ));

        // With a generous budget the squared result is exact: (3/7)^20 has the expected shape.
        let value = rat(3, 7)
            .checked_pow(20, &Limits::unlimited(), &NeverCancel)
            .expect("fits");
        assert_eq!(value, rat(3i64.pow(20), 7u64.pow(20)));
        assert_canonical(&value);
    }

    #[test]
    fn cancellation_is_observed() {
        struct Always;
        impl Cancel for Always {
            fn is_cancelled(&self) -> bool {
                true
            }
        }
        assert_eq!(
            rat(2, 1).checked_pow(10, &Limits::unlimited(), &Always),
            Err(RationalError::Cancelled)
        );
    }

    #[test]
    fn ordering_is_numeric_not_lexical() {
        assert!(rat(1, 3) < rat(1, 2));
        assert!(rat(-1, 2) < rat(-1, 3));
        assert_eq!(rat(2, 4).cmp(&rat(1, 2)), Ordering::Equal);
        assert!(rat(-5, 2) < rat(-2, 1));
        assert_eq!(rat(3, 1).to_i64(), Some(3));
        assert_eq!(rat(1, 2).to_i64(), None);
        assert_eq!(rat(-2, 1).to_i64(), Some(-2));
        assert_eq!(
            Rat::from_parts(
                Int::from_decimal("-9223372036854775808").expect("i64::MIN"),
                Nat::from_u64(1)
            )
            .expect("valid")
            .to_i64(),
            Some(i64::MIN)
        );
    }

    #[test]
    fn decimal_parsing_is_a_boundary_only() {
        assert_eq!(Rat::from_decimal("0.125"), Some(rat(1, 8)));
        assert_eq!(Rat::from_decimal("-2.5"), Some(rat(-5, 2)));
        assert_eq!(Rat::from_decimal("3"), Some(rat(3, 1)));
        assert_eq!(Rat::from_decimal("3/4"), Some(rat(3, 4)));
        assert_eq!(Rat::from_decimal("-3/-4"), Some(rat(3, 4)));
        assert_eq!(Rat::from_decimal("1/0"), None);
        assert_eq!(Rat::from_decimal("nope"), None);
        assert_eq!("1.5".parse::<Rat>(), Ok(rat(3, 2)));
    }

    #[test]
    fn conversion_helpers_are_lossless_when_they_return_a_value() {
        let nat = Nat::from_u64(7);
        assert_eq!(Rat::from_nat(&nat).to_u64(), Some(7));
        let int = Int::from_i64(-11);
        assert_eq!(Rat::from_int(&int).to_i64(), Some(-11));
        assert_eq!(rat(4, 2).to_int_exact(), Some(Int::from_i64(2)));
        assert_eq!(rat(3, 2).to_int_exact(), None);
        assert_eq!(rat(3, 2).to_u64(), None);
    }

    #[test]
    fn limits_bound_both_representations() {
        let value = rat(1, 3);
        assert!(Limits::digits(100).check(&value).is_ok());
        assert!(Limits::bits(100).check(&value).is_ok());
        assert_eq!(
            Limits::bits(1).check(&value),
            Err(RationalError::BitBudgetExceeded {
                limit: 1,
                observed: value.bit_length()
            })
        );
    }

    #[test]
    fn digit_preflight_reports_the_existing_category() {
        assert_eq!(Limits::digits(10).check_digits(10), Ok(()));
        let error = Limits::digits(10)
            .check_digits(11)
            .expect_err("eleven digits over a ten-digit budget");
        assert_eq!(
            error,
            RationalError::DigitBudgetExceeded {
                limit: 10,
                observed: 11
            }
        );
        assert_eq!(error.code(), "RationalDigitBudgetExceeded");
        assert_eq!(error.category(), "BudgetExceeded");
        assert_eq!(Limits::unlimited().check_digits(usize::MAX), Ok(()));
        // A bit-only budget never refuses a digit preflight.
        assert_eq!(Limits::bits(1).check_digits(usize::MAX), Ok(()));
    }
}
