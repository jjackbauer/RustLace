pub use lovelace_rational::Rat as Q;

use std::ops::Add;

pub struct ServiceTime(Q);

impl ServiceTime {
    pub fn seconds(value: Q) -> ServiceTime { ServiceTime(value) }
    pub fn demand(&self, visits: &Q) -> Q { self.0.mul(visits) }
}

impl Add for ServiceTime {
    type Output = ServiceTime;
    fn add(self, other: Self) -> Self { ServiceTime(self.0.add(&other.0)) }
}

pub fn utilization(arrival: &Q, service: &Q) -> Q { arrival.mul(service) }
