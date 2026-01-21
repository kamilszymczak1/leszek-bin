use num::{self, BigInt};
use num::BigRational;
use num::rational::Ratio;
use num_traits::cast::ToPrimitive;

use std::ops::Add;
use std::rc::Rc;

pub fn frac(a: i64, b: i64) -> BigRational {
    Ratio::new(BigInt::from(a), BigInt::from(b))
}

#[derive(Eq, PartialEq, Debug, Clone, PartialOrd, Ord)]
pub struct Time(pub num::BigRational);

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

    pub fn cycle_segment(&self) -> Segment {
        Segment::new(self.cycle_start(), Time(self.cycle_start().0 + frac(1, 1)))
    }

    pub fn from_cycle_index(cycle: u32) -> Self {
        Time(BigRational::from(BigInt::from(cycle)))
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

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Segment {
    start: Time,
    end: Time,
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
}

#[derive(PartialEq, Eq, Clone, PartialOrd, Ord, Debug)]
pub struct Part {
    part: Segment,
    whole: Option<Segment>,
}

impl Part {
    fn new(part: Segment, whole: Option<Segment>) -> Part {
        Part {
            part, 
            whole,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Event<T> {
    part: Part,
    value: T,
}

impl<T> Event<T> {
    fn new(part: Part, a: T) -> Self {
        Event {
            part,
            value: a,
        }
    }
}

trait Pattern<T> where T: Sized {
    type Events: Iterator<Item = Event<T>>;

    fn query(&self, segment: Segment) -> Self::Events;
}

fn cycled<T: Clone>(a: T) -> impl Pattern<T> {
    move |segment: Segment| {
        let a = a.clone();
        segment.split_on_cycles().into_iter().map(move |cycle| {
            Event::new(Part::new(cycle.clone(), Some(cycle.start.cycle_segment())), a.clone())
        })
    }
}

impl<T, I, F> Pattern<T> for F 
where
    F: Fn(Segment) -> I,
    I: Iterator<Item = Event<T>>
{
    type Events = I;

    fn query(&self, segment: Segment) -> Self::Events {
        (*self)(segment)
    }
}

fn slowcat<T, I>(pats: I) -> impl Pattern<T>
where 
    I: Iterator,
    I::Item: Pattern<T>,
{
    let patterns: Rc<[I::Item]>  = pats.collect();
    move |segment: Segment| {
        let patterns = patterns.clone();
        segment.split_on_cycles().into_iter().flat_map(move |cycle| {
            let pattern = &patterns[cycle.start.cycle_index() as usize % patterns.len()];
            pattern.query(cycle.clone())
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::pattern::{Time, Segment, cycled, Pattern};

    #[test]
    fn test_time() {
        let seg = Segment::new(Time::new(1, 2), Time::new(28, 10));
        assert_eq!(seg.split_on_cycles(), vec![
            Segment::new(Time::new(1, 2), Time::new(1, 1)),
            Segment::new(Time::new(1, 1), Time::new(2, 1)),
            Segment::new(Time::new(2, 1), Time::new(28, 10)),
        ]);

        let small_seg = Segment::new(Time::new(1, 3), Time::new(2, 3));
        assert_eq!(small_seg.split_on_cycles(), vec![
            small_seg
        ]);
        
        assert_eq!(Time::new(85, 9).cycle_start(), Time::new(9, 1));
        assert_eq!(Time::new(85, 9).cycle_index(), 9);
        assert_eq!(Time::new(13, 2).cycle_segment(), Segment::new(Time::new(6, 1), Time::new(7, 1)));
    }

    #[test]
    fn test_cycled() {
        let res = cycled(0).query(Segment::new(Time::new(1, 2), Time::new(5, 2)));
        // TODO
    }
}
