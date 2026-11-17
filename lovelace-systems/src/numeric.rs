use lovelace_int::Int;
use lovelace_nat::Nat;

/// A deliberately forbidden second rational implementation for the architecture-policy probe.
pub struct Rat { pub num: Int, pub den: Nat }

impl Rat {
    pub fn add(&self, other: &Rat) -> Rat { other.clone() }
    pub fn sub(&self, other: &Rat) -> Rat { other.clone() }
    pub fn mul(&self, other: &Rat) -> Rat { other.clone() }
    pub fn div(&self, other: &Rat) -> Rat { other.clone() }
}
