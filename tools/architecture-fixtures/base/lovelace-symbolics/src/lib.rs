pub use lovelace_rational::Rat;

pub struct Expr { terms: Vec<(Rat, String)> }

pub fn derivative(expression: &Expr) -> Expr { Expr { terms: expression.terms.clone() } }
