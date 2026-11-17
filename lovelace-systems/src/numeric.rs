//! Exact rational arithmetic for the systems library.
//!
//! [`Q`] is a reduced rational over the workspace's arbitrary-precision [`Int`] and [`Nat`]. The
//! reduction uses [`Nat::gcd`] after every operation, so a long MVA recurrence does not grow
//! denominators without bound. The audit behind this type (rather than reusing
//! `lovelace_symbolics::Rat`) is recorded in the crate docs: `Rat::new` reduces only when both
//! parts fit `u64`, and it rebuilds denominators through decimal strings, which is the wrong cost
//! profile for a tight exact recurrence.
//!
//! There is no floating point here and no approximate path: a quotient either exists exactly or
//! the operation returns `None` (division by zero). A caller that needs a decimal rendering asks
//! [`Q::to_real`], which preserves the periodic-decimal metadata `lovelace-real` carries.

use std::cmp::Ordering;
use std::fmt;

use lovelace_int::Int;
use lovelace_nat::Nat;
use lovelace_real::Real;

/// An exact rational number in lowest terms with a positive denominator.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Q {
    num: Int,
    den: Nat,
}

impl Q {
    /// Zero (`0/1`).
    pub fn zero() -> Q {
        Q {
            num: Int::zero(),
            den: Nat::from_u64(1),
        }
    }

    /// One (`1/1`).
    pub fn one() -> Q {
        Q::from_i64(1)
    }

    /// An integer as a rational.
    pub fn from_i64(value: i64) -> Q {
        Q {
            num: Int::from_i64(value),
            den: Nat::from_u64(1),
        }
    }

    /// A non-negative integer as a rational.
    pub fn from_u64(value: u64) -> Q {
        Q {
            num: Int::from_nat(Nat::from_u64(value)),
            den: Nat::from_u64(1),
        }
    }

    /// A non-negative natural as a rational.
    pub fn from_nat(value: &Nat) -> Q {
        Q {
            num: Int::from_nat(value.clone()),
            den: Nat::from_u64(1),
        }
    }

    /// Builds a rational from a numerator and a denominator.
    ///
    /// Returns `None` for a zero denominator. The result is always reduced (the gcd of the
    /// magnitude and the denominator is divided out) and the sign lives only in the numerator.
    pub fn from_parts(num: Int, den: Nat) -> Option<Q> {
        if den.is_zero() {
            return None;
        }
        if num.is_zero() {
            return Some(Q::zero());
        }
        let divisor = num.magnitude().gcd(&den);
        let reduced_num = if divisor.to_decimal() == "1" {
            num
        } else {
            num.div_rem(&divisor).map(|(q, _)| q).unwrap_or(num)
        };
        let reduced_den = if divisor.to_decimal() == "1" {
            den
        } else {
            den.div_rem(&divisor).map(|(q, _)| q).unwrap_or(den)
        };
        Some(Q {
            num: reduced_num,
            den: reduced_den,
        })
    }

    /// Converts an exact [`Real`] to its rational value.
    ///
    /// A `Real` is a decimal at heart, so this is exact for every value the engine can hold,
    /// including a value with period metadata (`1/3` becomes `1/3`, not a rounded decimal).
    pub fn from_real(value: &Real) -> Q {
        let (num, den) = value.to_rational();
        Q::from_parts(num, den).unwrap_or_else(Q::zero)
    }

    /// The numerator (the sign lives here).
    pub fn numerator(&self) -> &Int {
        &self.num
    }

    /// The denominator (strictly positive).
    pub fn denominator(&self) -> &Nat {
        &self.den
    }

    /// True when the value is zero.
    pub fn is_zero(&self) -> bool {
        self.num.is_zero()
    }

    /// True when the value is one.
    pub fn is_one(&self) -> bool {
        !self.num.is_negative() && self.num.to_decimal() == "1" && self.den.to_decimal() == "1"
    }

    /// True when the value is negative.
    pub fn is_negative(&self) -> bool {
        self.num.is_negative()
    }

    /// True when the value is positive.
    pub fn is_positive(&self) -> bool {
        !self.num.is_negative() && !self.num.is_zero()
    }

    /// True when the denominator is `1`.
    pub fn is_integer(&self) -> bool {
        self.den.to_decimal() == "1"
    }

    /// The integer value, when it is integral and fits an `i64`.
    pub fn to_i64(&self) -> Option<i64> {
        if !self.is_integer() {
            return None;
        }
        self.num.to_decimal().parse().ok()
    }

    /// The total number of decimal digits in the numerator and denominator.
    ///
    /// This is the growth measure a solver budgets: it bounds the size of an exact intermediate
    /// without rounding it.
    pub fn decimal_digits(&self) -> usize {
        self.num.magnitude().to_decimal().len() + self.den.to_decimal().len()
    }

    /// Negation.
    pub fn neg(&self) -> Q {
        Q {
            num: self.num.negate(),
            den: self.den.clone(),
        }
    }

    /// Absolute value.
    pub fn abs(&self) -> Q {
        if self.is_negative() {
            self.neg()
        } else {
            self.clone()
        }
    }

    /// Addition.
    pub fn add(&self, other: &Q) -> Q {
        let left = self.num.mul(&Int::from_nat(other.den.clone()));
        let right = other.num.mul(&Int::from_nat(self.den.clone()));
        let den = self.den.mul(&other.den);
        Q::from_parts(left.add(&right), den).unwrap_or_else(Q::zero)
    }

    /// Subtraction.
    pub fn sub(&self, other: &Q) -> Q {
        self.add(&other.neg())
    }

    /// Multiplication.
    pub fn mul(&self, other: &Q) -> Q {
        let num = self.num.mul(&other.num);
        let den = self.den.mul(&other.den);
        Q::from_parts(num, den).unwrap_or_else(Q::zero)
    }

    /// Division; `None` when `other` is zero.
    pub fn div(&self, other: &Q) -> Option<Q> {
        if other.is_zero() {
            return None;
        }
        let num = self.num.mul(&Int::from_nat(other.den.clone()));
        let den = self.den.mul(other.num.magnitude());
        Q::from_parts(num, den)
    }

    /// Exact integer power. `None` when a negative exponent would divide by zero.
    pub fn pow_i64(&self, exponent: i64) -> Option<Q> {
        if exponent == 0 {
            return Some(Q::one());
        }
        if exponent < 0 {
            if self.is_zero() {
                return None;
            }
            let positive = exponent.checked_neg()?;
            return self.pow_i64(positive).and_then(|p| Q::one().div(&p));
        }
        let mut acc = Q::one();
        for _ in 0..exponent {
            acc = acc.mul(self);
        }
        Some(acc)
    }

    /// Renders the value as an exact decimal or periodic decimal.
    ///
    /// `digits` bounds the fractional places used to *find* the expansion: a terminating decimal
    /// is exact immediately, and a repeating one is published with its period (`0.(3)`) when the
    /// period closes inside the budget. This is a rendering of the exact rational, never a rounded
    /// value; a caller that wants a fixed-place approximation asks [`Q::to_real`] and formats it.
    pub fn to_decimal(&self, digits: usize) -> String {
        self.to_real(digits)
            .map(|value| value.to_string())
            .unwrap_or_else(|| self.to_string())
    }

    /// Converts to an exact [`Real`], with the period detected when the expansion repeats.
    pub fn to_real(&self, max_digits: usize) -> Option<Real> {
        let value = Real::from_fraction(self.num.magnitude(), &self.den, max_digits.max(1))?;
        Some(if self.num.is_negative() {
            value.neg()
        } else {
            value
        })
    }
}

impl Ord for Q {
    fn cmp(&self, other: &Self) -> Ordering {
        let left = self.num.mul(&Int::from_nat(other.den.clone()));
        let right = other.num.mul(&Int::from_nat(self.den.clone()));
        left.cmp(&right)
    }
}

impl PartialOrd for Q {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl From<i64> for Q {
    fn from(value: i64) -> Q {
        Q::from_i64(value)
    }
}

impl From<u64> for Q {
    fn from(value: u64) -> Q {
        Q::from_u64(value)
    }
}

impl fmt::Display for Q {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_integer() {
            return f.write_str(&self.num.to_decimal());
        }
        write!(f, "{}/{}", self.num.to_decimal(), self.den.to_decimal())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn q(num: i64, den: u64) -> Q {
        Q::from_parts(Int::from_i64(num), Nat::from_u64(den)).expect("nonzero denominator")
    }

    #[test]
    fn fractions_are_reduced_and_added_exactly() {
        assert_eq!(q(1, 3).add(&q(1, 6)), q(1, 2));
        assert_eq!(q(1, 2).add(&q(1, 2)), Q::one());
        assert_eq!(q(2, 4), q(1, 2));
        assert_eq!(q(-2, 4), q(-1, 2));
        assert_eq!(q(1, 3).sub(&q(1, 3)), Q::zero());
    }

    #[test]
    fn large_products_stay_reduced() {
        // 99991 and 99989 are coprime; a product of rationals must divide the common factors
        // back out rather than accumulating an unreduced denominator.
        let left = q(99991, 99989);
        let right = q(99989, 99991);
        assert_eq!(left.mul(&right), Q::one());
        assert_eq!(left.mul(&right).denominator().to_decimal(), "1");
        let sum = left.add(&right);
        assert_eq!(sum.denominator().to_decimal(), "9998000099");
        assert!(sum.decimal_digits() < 40);
    }

    #[test]
    fn division_by_zero_is_refused() {
        assert!(q(1, 2).div(&Q::zero()).is_none());
        assert!(Q::from_parts(Int::from_i64(1), Nat::zero()).is_none());
        assert!(Q::zero().pow_i64(-1).is_none());
    }

    #[test]
    fn real_round_trip_keeps_the_period() {
        let third = q(1, 3);
        let real = third.to_real(30).expect("converts");
        assert_eq!(real.period_length(), 1);
        assert_eq!(real.to_decimal_fixed(6), "0.333333");
        assert_eq!(third.to_decimal(6), "0.(3)");
        assert_eq!(Q::from_real(&real), third);
        assert_eq!(q(1, 8).to_decimal(10), "0.125");
        assert_eq!(q(-1, 8).to_decimal(3), "-0.125");
        assert_eq!(q(1, 12).to_decimal(6), "0.08(3)");
    }

    #[test]
    fn ordering_is_numeric_not_lexical() {
        assert!(q(1, 3) < q(1, 2));
        assert!(q(-1, 2) < q(-1, 3));
        assert_eq!(q(2, 4).cmp(&q(1, 2)), Ordering::Equal);
        assert_eq!(q(3, 1).to_i64(), Some(3));
        assert_eq!(q(1, 2).to_i64(), None);
    }
}
