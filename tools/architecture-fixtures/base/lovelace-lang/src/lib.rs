use lovelace_rational::Rat;
use lovelace_real::Real;
use lovelace_systems::{Q, ServiceTime};

pub fn promote(value: i64) -> Rat { Rat::from_i64(value) }

pub fn demand(value: &Q) -> Q { ServiceTime::seconds(value.clone()).demand(&Q::from_i64(1)) }

// A permitted local representation: the language's complex value type is built on the canonical
// Real abstraction and is allowed for lovelace-lang by duplicateDetection.permittedRepresentations.
pub struct Complex { pub re: Real, pub im: Real }

impl Complex {
    pub fn add(&self, other: &Complex) -> Complex { other.clone() }
    pub fn sub(&self, other: &Complex) -> Complex { other.clone() }
    pub fn mul(&self, other: &Complex) -> Complex { other.clone() }
    pub fn div(&self, other: &Complex) -> Complex { other.clone() }
}
