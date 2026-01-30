//! Time representation for musical patterns.
//!
//! This module provides arbitrary-precision rational time values, enabling
//! exact timing calculations without floating-point errors. Time is measured
//! in cycles, where one cycle typically represents one bar or loop iteration.

use num::rational::Ratio;
use num::{self, BigInt};
use num::{BigRational, ToPrimitive};

use std::ops::{Add, Div, Mul, Sub};

/// A point in time represented as an arbitrary-precision rational number.
///
/// Time is measured in cycles (typically bars), where:
/// - `0` is the start of the first cycle
/// - `1` is the start of the second cycle
/// - `0.5` is halfway through the first cycle
/// - `1/3` is one-third through the first cycle
///
/// Using rational numbers ensures exact timing without floating-point drift,
/// which is critical for musical applications.
///
/// # Examples
///
/// | Time      | Meaning                           |
/// |-----------|-----------------------------------|
/// | `0`       | Start of cycle 0                  |
/// | `1/2`     | Halfway through cycle 0           |
/// | `1`       | Start of cycle 1                  |
/// | `3/4`     | Three-quarters through cycle 0    |
/// | `5/2`     | Halfway through cycle 2           |
#[derive(Eq, PartialEq, Clone, PartialOrd, Ord)]
pub struct Time(pub num::BigRational);

/// Creates a rational number from a numerator and denominator.
///
/// Convenience function for constructing `BigRational` values.
pub fn frac(a: i64, b: i64) -> BigRational {
    Ratio::new(BigInt::from(a), BigInt::from(b))
}

impl Time {
    /// Creates a new time value from a fraction `a/b`.
    ///
    /// # Example
    ///
    /// ```
    /// Time::new(1, 4) // Quarter of a cycle (one beat in 4/4 time)
    /// Time::new(3, 2) // Halfway through cycle 1
    /// ```
    pub fn new(a: i64, b: i64) -> Self {
        Time(frac(a, b))
    }

    /// Returns the index of the cycle containing this time (0-indexed).
    ///
    /// This is the floor of the time value.
    ///
    /// # Examples
    ///
    /// - `Time(0.5).cycle_index()` → `0`
    /// - `Time(1.0).cycle_index()` → `1`
    /// - `Time(2.7).cycle_index()` → `2`
    pub fn cycle_index(&self) -> u32 {
        let bigint: BigInt = self.0.floor().to_integer();
        bigint.to_u32().unwrap()
    }

    /// Returns the time at the start of the cycle containing this time.
    ///
    /// # Examples
    ///
    /// - `Time(0.5).cycle_start()` → `Time(0)`
    /// - `Time(2.7).cycle_start()` → `Time(2)`
    pub fn cycle_start(&self) -> Time {
        Time(self.0.floor())
    }

    /// Creates a time value representing the start of the given cycle.
    ///
    /// # Example
    ///
    /// ```
    /// Time::from_cycle_index(3) // Time(3) - start of cycle 3
    /// ```
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

/// Adds an integer number of cycles to the time.
impl Add<i64> for Time {
    type Output = Time;

    fn add(self, rhs: i64) -> Self::Output {
        Time(self.0 + frac(rhs, 1))
    }
}

/// Adds a rational offset to the time.
impl Add<BigRational> for Time {
    type Output = Time;

    fn add(self, rhs: BigRational) -> Self::Output {
        Time(self.0 + rhs)
    }
}

/// Adds two time values together.
impl Add<Time> for Time {
    type Output = Time;

    fn add(self, rhs: Time) -> Self::Output {
        Time(self.0 + rhs.0)
    }
}

/// Subtracts one time from another.
impl Sub<Time> for Time {
    type Output = Time;

    fn sub(self, rhs: Time) -> Self::Output {
        Time(self.0 - rhs.0)
    }
}

/// Multiplies time by an integer factor (for time stretching).
impl Mul<i64> for Time {
    type Output = Time;

    fn mul(self, rhs: i64) -> Self::Output {
        Time(self.0 * frac(rhs, 1))
    }
}

/// Multiplies time by a rational factor (for time stretching).
impl Mul<BigRational> for Time {
    type Output = Time;

    fn mul(self, rhs: BigRational) -> Self::Output {
        Time(self.0 * rhs)
    }
}

/// Divides time by an integer (for time compression).
impl Div<i64> for Time {
    type Output = Time;

    fn div(self, rhs: i64) -> Self::Output {
        Time(self.0 / frac(rhs, 1))
    }
}

/// Divides time by a rational value (for time compression).
impl Div<BigRational> for Time {
    type Output = Time;

    fn div(self, rhs: BigRational) -> Self::Output {
        Time(self.0 / rhs)
    }
}
