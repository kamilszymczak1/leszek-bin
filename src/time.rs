use num::{BigRational, ToPrimitive};
use num::rational::Ratio;
use num::{self, BigInt};

use std::ops::{Add, Sub, Mul, Div};

#[derive(Eq, PartialEq, Clone, PartialOrd, Ord)]
pub struct Time(pub num::BigRational);

pub fn frac(a: i64, b: i64) -> BigRational {
    Ratio::new(BigInt::from(a), BigInt::from(b))
}

impl Time {
    pub fn new(a: i64, b: i64) -> Self {
        Time(frac(a, b))
    }

    pub fn cycle_index(&self) -> u32 {
        let bigint: BigInt = self.0.floor().to_integer();
        bigint.to_u32().unwrap()
    }

    pub fn cycle_start(&self) -> Time {
        Time(self.0.floor())
    }

    pub fn from_cycle_index(cycle: u32) -> Self {
        Time(BigRational::from(BigInt::from(cycle)))
    }
}

impl std::fmt::Display for Time {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::fmt::Debug for Time {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Time({})", self.0)
    }
}

impl Add<i64> for Time {
    type Output = Time;

    fn add(self, rhs: i64) -> Self::Output {
        Time(self.0 + frac(rhs, 1))
    }
}

impl Add<BigRational> for Time {
    type Output = Time;

    fn add(self, rhs: BigRational) -> Self::Output {
        Time(self.0 + rhs)
    }
}

impl Add<Time> for Time {
    type Output = Time;

    fn add(self, rhs: Time) -> Self::Output {
        Time(self.0 + rhs.0)
    }
}

impl Sub<Time> for Time {
    type Output = Time;

    fn sub(self, rhs: Time) -> Self::Output {
        Time(self.0 - rhs.0)
    }
}

impl Mul<i64> for Time {
    type Output = Time;

    fn mul(self, rhs: i64) -> Self::Output {
        Time(self.0 * frac(rhs, 1))
    }
}

impl Mul<BigRational> for Time {
    type Output = Time;

    fn mul(self, rhs: BigRational) -> Self::Output {
        Time(self.0 * rhs)
    }
}

impl Div<i64> for Time {
    type Output = Time;

    fn div(self, rhs: i64) -> Self::Output {
        Time(self.0 / frac(rhs, 1))
    }
}

impl Div<BigRational> for Time {
    type Output = Time;

    fn div(self, rhs: BigRational) -> Self::Output {
        Time(self.0 / rhs)
    }
}