use std::{cmp::{max, min}, ops::{Add, Sub}};

use num::{BigInt, BigRational};

use crate::time::Time;

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Segment {
    pub start: Time,
    pub end: Time,
}

impl Add<Time> for Segment {
    type Output = Segment;

    fn add(self, rhs: Time) -> Self::Output {
        Segment::new(self.start + rhs.clone(), self.end + rhs)
    }
}

impl Sub<Time> for Segment {
    type Output = Segment;

    fn sub(self, rhs: Time) -> Self::Output {
        Segment::new(self.start - rhs.clone(), self.end - rhs)
    }
}

impl Segment {
    pub fn new(start: Time, end: Time) -> Self {
        Segment { start, end }
    }

    // Split segment on the boundaries between cycles. E.g splitting [0.5, 2.8) results in 
    // [[0.5, 1), [1, 2), [2, 2.8)]
    pub fn split_on_cycles(&self) -> Vec<Segment> {
        let start_cycle = self.start.cycle_index() + 1;
        let end_cycle = self.end.cycle_index();

        if end_cycle < start_cycle {
            return vec![self.clone()];
        }

        let mut result: Vec<Segment> = vec![];

        let left_full_time = Time::from_cycle_index(start_cycle);
        if self.start != left_full_time  {
            result.push(Segment::new(self.start.clone(), left_full_time));
        }

        for cycle in start_cycle..end_cycle {
            let cycle_segment = Segment::new(Time::from_cycle_index(cycle), Time::from_cycle_index(cycle + 1));
            result.push(cycle_segment);
        }

        let right_full_time = Time::from_cycle_index(end_cycle);
        if self.end != right_full_time {
            result.push(Segment::new(right_full_time, self.end.clone()));
        }

        result
    }

    pub fn duration(self) -> BigRational {
        self.end.0 - self.start.0
    }

    pub fn scaled(self, factor: BigRational) -> Segment {
        Segment::new(
            Time(self.start.0 * factor.clone()),
            Time(self.end.0 * factor),
        )
    }

    pub fn intersection(&self, other: &Segment) -> Option<Segment> {
        let start = max(self.start.clone(), other.start.clone());
        let end = min(self.end.clone(), other.end.clone());
        if start < end {
            Some(Segment::new(start, end))
        } else {
            None
        }
    }
}

impl std::fmt::Display for Segment {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}, {})", self.start, self.end)
    }
}

impl std::fmt::Debug for Segment {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}, {})", self.start, self.end)
    }
}

pub fn cycle_segment_from_time(t: &Time) -> Segment {
    Segment::new(t.cycle_start(), Time(t.cycle_start().0 + BigRational::from(BigInt::from(1))))
}