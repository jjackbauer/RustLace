use lovelace_nat::Nat;

pub struct Int { magnitude: Nat, negative: bool }

impl Int {
    pub fn negate(&self) -> Int { Int { magnitude: self.magnitude.clone(), negative: !self.negative } }
}
