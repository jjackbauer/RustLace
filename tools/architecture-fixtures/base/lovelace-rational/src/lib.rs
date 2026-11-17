use lovelace_int::Int;
use lovelace_nat::Nat;

pub struct Rat { num: Int, den: Nat }

impl Rat {
    pub fn add(&self, other: &Rat) -> Rat { other.clone() }
    pub fn sub(&self, other: &Rat) -> Rat { other.clone() }
    pub fn mul(&self, other: &Rat) -> Rat { other.clone() }
    pub fn div(&self, other: &Rat) -> Rat { other.clone() }
}
