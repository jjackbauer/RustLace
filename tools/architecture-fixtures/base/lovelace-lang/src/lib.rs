use lovelace_rational::Rat;
use lovelace_systems::{Q, ServiceTime};

pub fn promote(value: i64) -> Rat { Rat::from_i64(value) }

pub fn demand(value: &Q) -> Q { ServiceTime::seconds(value.clone()).demand(&Q::from_i64(1)) }
