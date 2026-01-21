use num::{self, BigInt};
use num::BigRational;
use num::rational::Ratio;
use num_traits::cast::ToPrimitive;

use std::ops::{Add, Div, Mul};
use std::rc::Rc;

pub fn frac(a: i64, b: i64) -> BigRational {
    Ratio::new(BigInt::from(a), BigInt::from(b))
}

#[derive(Eq, PartialEq, Clone, PartialOrd, Ord)]
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

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Segment {
    pub start: Time,
    pub end: Time,
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

#[derive(PartialEq, Eq, Clone, PartialOrd, Ord)]
pub struct Part {
    pub part: Segment,
    pub whole: Option<Segment>,
}

impl Part {
    fn new(part: Segment, whole: Option<Segment>) -> Part {
        Part {
            part, 
            whole,
        }
    }
}

impl std::fmt::Display for Part {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.part)
    }
}

impl std::fmt::Debug for Part {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.part)
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct Event<T> {
    pub part: Part,
    pub value: T,
}

impl<T> Event<T> {
    fn new(part: Part, a: T) -> Self {
        Event {
            part,
            value: a,
        }
    }
}

impl<T> std::fmt::Display for Event<T> where T: std::fmt::Display {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} | {}", self.part, self.value)
    }
}

impl<T> std::fmt::Debug for Event<T> where T: std::fmt::Debug {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?} | {:?}", self.part, self.value)
    }
}

pub trait Pattern<T> where T: Sized {
    type Events: Iterator<Item = Event<T>>;

    fn query(&self, segment: Segment) -> Self::Events;

    fn boxed(self) -> BoxPattern<T> 
    where Self: Sized, Self: 'static, T: 'static {
        let boxed_pattern: Box<dyn Pattern<T, Events = BoxEvents<T>>> = Box::new(move |segment| {
            BoxEvents(Box::new(self.query(segment)))
        });
        BoxPattern(boxed_pattern)
    }
}

fn display_pattern<T, P>(pattern: P) -> String
where
    P: Pattern<T>,
    T: std::fmt::Display,
{
    let mut result = String::new();
    let segment = Segment::new(Time::new(0, 1), Time::new(5, 1));
    for event in pattern.query(segment) {
        result.push_str(&format!("{}\n", event));
    }
    result
}



pub fn cycled<T: Clone>(a: T) -> impl Pattern<T> {
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

struct Rate<T, I> 
where T: Pattern<I> {
    pattern: T,
    _marker: std::marker::PhantomData<I>,
}

pub struct BoxEvents<T>(Box<dyn Iterator<Item = Event<T>>>);

impl<T> Iterator for BoxEvents<T> {
    type Item = Event<T>;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }
}

pub struct BoxPattern<T>(Box<dyn Pattern<T, Events = BoxEvents<T>>>);

impl<T> Pattern<T> for BoxPattern<T>
{
    type Events = BoxEvents<T>;

    fn query(&self, segment: Segment) -> Self::Events {
        self.0.query(segment)
    }
}

pub fn slowcat<T, I>(pats: I) -> impl Pattern<T>
where 
    I: Iterator,
    I::Item: Pattern<T>
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

fn fastcat<T, I>(pats : I) -> impl Pattern<T>
where
    I: ExactSizeIterator,
    I::Item: Pattern<T>
{
    let len = pats.len();
    let slowcat = slowcat(pats);
    move |segment: Segment| {
        let extended_segment = Segment::new(
            segment.start.clone() * BigRational::from(BigInt::from(len as i64)),
            segment.end.clone() * BigRational::from(BigInt::from(len as i64)),
        );
        slowcat.query(extended_segment).map(move |event| {
            let part = Segment::new(
                event.part.part.start.clone() / BigRational::from(BigInt::from(len as i64)),
                event.part.part.end.clone() / BigRational::from(BigInt::from(len as i64)),
            );
            Event::new(Part::new(part, event.part.whole.clone()), event.value)
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::pattern::{Time, Segment, cycled, Pattern, display_pattern, slowcat, fastcat};

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

    #[test]
    fn test_display_pattern_output() {
        assert_eq!(
            display_pattern(cycled(1)),
            "[0, 1) | 1\n\
             [1, 2) | 1\n\
             [2, 3) | 1\n\
             [3, 4) | 1\n\
             [4, 5) | 1\n"
        );

        assert_eq!(
            display_pattern(slowcat(vec![cycled(1), cycled(2)].into_iter())),
            "[0, 1) | 1\n\
             [1, 2) | 2\n\
             [2, 3) | 1\n\
             [3, 4) | 2\n\
             [4, 5) | 1\n"
        )
    }

    #[test]
    fn test_fastcat() {
        println!("{}", display_pattern(fastcat(vec![cycled(1), cycled(2)].into_iter())));
        assert_eq!(
            display_pattern(fastcat(vec![cycled(1), cycled(2)].into_iter())),
            "[0, 1/2) | 1\n\
             [1/2, 1) | 2\n\
             [1, 3/2) | 1\n\
             [3/2, 2) | 2\n\
             [2, 5/2) | 1\n\
             [5/2, 3) | 2\n\
             [3, 7/2) | 1\n\
             [7/2, 4) | 2\n\
             [4, 9/2) | 1\n\
             [9/2, 5) | 2\n"
        );

        assert_eq!(
            display_pattern(fastcat(vec![cycled(1), cycled(2), cycled(3)].into_iter())),
            "[0, 1/3) | 1\n\
             [1/3, 2/3) | 2\n\
             [2/3, 1) | 3\n\
             [1, 4/3) | 1\n\
             [4/3, 5/3) | 2\n\
             [5/3, 2) | 3\n\
             [2, 7/3) | 1\n\
             [7/3, 8/3) | 2\n\
             [8/3, 3) | 3\n\
             [3, 10/3) | 1\n\
             [10/3, 11/3) | 2\n\
             [11/3, 4) | 3\n\
             [4, 13/3) | 1\n\
             [13/3, 14/3) | 2\n\
             [14/3, 5) | 3\n"
        )
    }
}
