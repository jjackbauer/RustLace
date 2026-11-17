use lovelace_rational::Rat;

// The canonical real. `Real` is reserved to this crate by
// duplicateDetection.reservedTypeNames, and the owner may declare it exactly once.
pub struct Real { pub int: Rat, pub exp: i64 }
