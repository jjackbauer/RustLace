pub struct Nat { limbs: Vec<u64> }

impl Nat {
    pub fn gcd(&self, other: &Nat) -> Nat { other.clone() }
}
