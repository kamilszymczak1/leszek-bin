use num::{self, BigInt};
use num::BigRational;
use num::rational::Ratio;
use num_traits::cast::ToPrimitive;
use rand::Rng;
use rosc::OscType;

use crate::scale::{Scale, map_note};
use crate::note::Note;

use std::cmp::{max, min};
use std::ops::{Add, Div, Mul, Sub};
use std::rc::Rc;

use crate::superdirt::{self, ControlMessage};

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

    fn scaled(self, factor: BigRational) -> Part {
        Part::new(
            self.part.scaled(factor.clone()),
            self.whole.map(|w| w.scaled(factor))
        )
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

    fn scaled(self, factor: BigRational) -> Event<T>  {
        Event::new(
            Part::new(
                self.part.part.scaled(factor.clone()),
                self.part.whole.map(|w| w.scaled(factor.clone()))
            ),
            self.value
        )
    }

    fn map_value<S>(self, f: impl Fn(T) -> S) -> Event<S> {
        Event::new(
            self.part,
            f(self.value)
        )
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
        let boxed_pattern: Rc<dyn Pattern<T, Events = BoxEvents<T>>> = Rc::new(move |segment| {
            BoxEvents(Box::new(self.query(segment)))
        });
        BoxPattern(boxed_pattern)
    }
}

pub fn display_pattern<T, P>(pattern: &P) -> String
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

#[derive(Clone)]
pub struct BoxPattern<T>(Rc<dyn Pattern<T, Events = BoxEvents<T>>>);

impl<T> Pattern<T> for BoxPattern<T>
{
    type Events = BoxEvents<T>;

    fn query(&self, segment: Segment) -> Self::Events {
        self.0.query(segment)
    }
}

// Speed up a pattern by a given factor. E.g a factor of 2 makes the pattern run twice as fast.
pub fn speed_up<T, P>(pattern: P, factor: BigRational) -> impl Pattern<T>
where
    P: Pattern<T>
{
    move |segment: Segment| {
        let factor_inv = factor.recip();
        let scaled_segment = segment.scaled(factor.clone());
        pattern.query(scaled_segment).map(move |event| {
            event.scaled(factor_inv.clone())
        })
    }
}

// Slowcat takes an iterator of patterns and alternates between them, giving each pattern
// a full cycle before moving to the next pattern.
// E.g slowcat([A, B]) produces <A | B | A | B | A | B ...>
// slowcat([A, B, C]) produces <A | B | C | A | B | C ...>
pub fn slowcat<T, I>(pats: I) -> impl Pattern<T>
where 
    I: Iterator,
    I::Item: Pattern<T>
{
    let patterns: Rc<[I::Item]>  = pats.collect();
    move |segment: Segment| {
        let patterns = patterns.clone();
        segment.split_on_cycles().into_iter().flat_map(move |cycle| {
            let len = patterns.len();
            // If this cycle's index is i, then we want to query pattern[i % len].
            let index = cycle.start.cycle_index();
            let pattern = &patterns[index as usize % len];
            // We are querying this pattern index/len'th time, so that is the cycle we want to
            // query.
            let offset = Time::new(index as i64, 1) - Time::new((index as usize/len) as i64, 1);
            pattern.query(cycle.clone() - offset.clone()).map(move |event| {
                let off = offset.clone();
                Event::new(Part::new(event.part.part + off.clone(), event.part.whole.map(|w| w + off)), event.value)
            })
        })
    }
}

// Fastcat takes an iterator of patterns and alternates between them, but speeds up time
// so that each pattern gets an equal share of each cycle.
// E.g fastcat([A, B]) produces <A B | A B | A B ...>
// fastcat([A, B, C]) produces <A B C | A B C | A B C ...> 
pub fn fastcat<T, I>(pats : I) -> impl Pattern<T>
where
    I: ExactSizeIterator,
    I::Item: Pattern<T>
{
    let len = pats.len();
    speed_up(slowcat(pats), frac(len as i64, 1))
}

pub fn random_slowcat<T, I>(pats: I) -> impl Pattern<T>
where 
    I: Iterator,
    I::Item: Pattern<T>
{
    let patterns: Rc<[I::Item]>  = pats.collect();
    move |segment: Segment| {
        let patterns = patterns.clone();
        segment.split_on_cycles().into_iter().flat_map(move |cycle| {
            let mut rng = rand::rng();
            let len = patterns.len();
            let index = rng.random_range(0..len);
            let pattern = &patterns[index];
            pattern.query(cycle.clone())
        })
    }
}

pub fn keyed(key: &'static str, values: impl Pattern<OscType>) -> impl Pattern<ControlMessage> {
    move |segment| {
        values.query(segment).map(move |event| {
            Event::new(event.part, ControlMessage::new(vec![(String::from(key), event.value)]))
        })
    }
}
                

pub fn in_parallel<T, I>(pats: I) -> impl Pattern<T>
where
    I: Iterator,
    I::Item: Pattern<T>,
{
    let patterns: Rc<[I::Item]>  = pats.collect();
    move |segment: Segment| {
        patterns.iter().flat_map(move |pattern| {
            pattern.query(segment.clone())
        }).collect::<Vec<_>>().into_iter()
    }
}

pub fn combine<P, Q, T, R, S, F>(first : P, second : Q, f : F) -> impl Pattern<S>
where
    P: Pattern<T>,
    Q : Pattern<R>,
    T : Clone,
    R : Clone,
    F: Fn(T, R) -> S + Clone
{
    move |segment: Segment| {
        let second_events: Vec<Event<R>> = second.query(segment.clone()).collect();

        // Note: This is quadratic, can be optimized to linear if needed.
        let mut results = Vec::new();
        for fevent in first.query(segment.clone()) {
            for sevent in &second_events {
                if let Some(intersection) = fevent.part.part.intersection(&sevent.part.part) {
                    let whole = match (&fevent.part.whole, &sevent.part.whole) {
                        (Some(fw), Some(sw)) => fw.intersection(sw),
                        _ => None,
                    };
                    results.push(Event::new(
                        Part::new(intersection, whole),
                        f(fevent.value.clone(), sevent.value.clone())
                    ));
                }
            }
        }
        results.into_iter()
    }
}

pub fn combine_left<P, Q, T, R, S, F>(first : P, second : Q, f : F) -> impl Pattern<S>
where
    P: Pattern<T>,
    Q : Pattern<R>,
    T : Clone,
    R : Clone,
    F: Fn(T, R) -> S + Clone
{
    move |segment: Segment| {
        let second_events: Vec<Event<R>> = second.query(segment.clone()).collect();

        let mut results = Vec::new();
        for fevent in first.query(segment.clone()) {
            second_events.iter().filter(
                |sevent| fevent.part.part.intersection(&sevent.part.part).is_some()
            )
            .min_by_key(
                |sevent| {
                    sevent.part.part.start.clone()
                }
            ).map(
                |sevent| {
                    results.push(fevent.map_value(|v| f(v, sevent.value.clone())));
                }
            );
        }
        results.into_iter()
    }
}

pub fn combine_right<P, Q, T, R, S, F>(first : P, second : Q, f : F) -> impl Pattern<S>
where
    P: Pattern<T>,
    Q : Pattern<R>,
    T : Clone,
    R : Clone,
    F: Fn(T, R) -> S + Clone
{
    combine_left(
        second,
        first,
        move |b, a| f(a, b)
    )
}

pub fn structure<P, Q, T>(pattern : P, filter : Q) -> impl Pattern<T>
where
    P: Pattern<T>,
    Q : Pattern<()>,
    T : Clone
{
    combine_right(
        pattern,
        filter,
        |a, _| a
    )
}

pub fn filter_output<T>(pat: impl Pattern<Option<T>>) -> impl Pattern<T> { 
    move |segment: Segment| {
        pat.query(segment).map(|ev| {
            if let Some(val) = ev.value {
                Some(Event::new(ev.part, val))
            } else {
                None
            }
        }).flatten()
    }
}

pub fn map_output<T, U, F, P>(pat: P, f: F) -> impl Pattern<U>
where
    P: Pattern<T>,
    F: Fn(T) -> U + Clone
{ 
    move |segment: Segment| {
        let f = f.clone();
        pat.query(segment).map(move |ev| {
            Event::new(ev.part, f(ev.value))
        })
    }
}

pub fn filter_map_output<T, U, F, P>(pat: P, f: F) -> impl Pattern<U>
where
    P: Pattern<T>,
    F: Fn(T) -> Option<U> + Clone
{ 
    filter_output(map_output(pat, f))
}

pub fn scale<P, Q>(pattern : P, scales: Q) -> impl Pattern<Note>
where P: Pattern<Note>, Q: Pattern<Scale>
{
    combine_left(
        pattern,
        scales,
        |note, scale| {
            map_note(&scale, note)
        }
    )
}

pub fn empty<T>() -> impl Pattern<T>
{
    move |_: Segment| {
        std::iter::empty()
    }
}

pub fn merge<P0, P1>(pattern0: P0, pattern1: P1) -> impl Pattern<ControlMessage> 
where 
    P0: Pattern<ControlMessage>,
    P1: Pattern<ControlMessage>,
{
    combine(pattern0, pattern1, |msg0, msg1| {
        let mut msg = msg0.clone();
        msg.merge(&mut msg1.clone());
        msg
    })
}

#[cfg(test)]
mod tests {
    use crate::{note::Note, scale::Scale, pattern::{Pattern, Segment, Time, cycled, display_pattern, fastcat, frac, in_parallel, random_slowcat, slowcat, speed_up, scale, empty, structure}};

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
            display_pattern(&cycled(1)),
            "[0, 1) | 1\n\
             [1, 2) | 1\n\
             [2, 3) | 1\n\
             [3, 4) | 1\n\
             [4, 5) | 1\n"
        );

        assert_eq!(
            display_pattern(&slowcat(vec![cycled(1), cycled(2)].into_iter())),
            "[0, 1) | 1\n\
             [1, 2) | 2\n\
             [2, 3) | 1\n\
             [3, 4) | 2\n\
             [4, 5) | 1\n"
        );
        
        // Make sure the inner slowcat alternates, i.e that each pattern's time
        // doesn't advance while they are not active.
        // <0 <1 2>> = 0 1 0 2 0 1 ...
        assert_eq!(
            display_pattern(&slowcat(vec![
                    cycled(0).boxed(), 
                    slowcat(vec![cycled(1), cycled(2)].into_iter()).boxed()
            ].into_iter())),
            "[0, 1) | 0\n\
             [1, 2) | 1\n\
             [2, 3) | 0\n\
             [3, 4) | 2\n\
             [4, 5) | 0\n"
        );
    }

    #[test]
    fn test_fastcat() {
        println!("{}", display_pattern(&fastcat(vec![cycled(1), cycled(2)].into_iter())));
        assert_eq!(
            display_pattern(&fastcat(vec![cycled(1), cycled(2)].into_iter())),
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
            display_pattern(&fastcat(vec![cycled(1), cycled(2), cycled(3)].into_iter())),
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

    #[test]
    fn test_speed_up() {
        let pattern = speed_up(cycled(1), frac(2, 1));
        assert_eq!(
            display_pattern(&pattern),
            "[0, 1/2) | 1\n\
             [1/2, 1) | 1\n\
             [1, 3/2) | 1\n\
             [3/2, 2) | 1\n\
             [2, 5/2) | 1\n\
             [5/2, 3) | 1\n\
             [3, 7/2) | 1\n\
             [7/2, 4) | 1\n\
             [4, 9/2) | 1\n\
             [9/2, 5) | 1\n"
        );

        let pattern = speed_up(slowcat(vec![cycled(1), cycled(2)].into_iter()), frac(2, 3));
        assert_eq!(
            display_pattern(&pattern),
            "[0, 3/2) | 1\n\
             [3/2, 3) | 2\n\
             [3, 9/2) | 1\n\
             [9/2, 5) | 2\n"
        );
    }

    #[test]
    fn test_in_parallel() {
        let pattern = in_parallel(vec![cycled(1), cycled(2)].into_iter());
        assert_eq!(
            display_pattern(&pattern),
            "[0, 1) | 1\n\
             [1, 2) | 1\n\
             [2, 3) | 1\n\
             [3, 4) | 1\n\
             [4, 5) | 1\n\
             [0, 1) | 2\n\
             [1, 2) | 2\n\
             [2, 3) | 2\n\
             [3, 4) | 2\n\
             [4, 5) | 2\n"
        );

        let pattern = in_parallel(vec![
            speed_up(cycled(1), frac(2, 3)),
            speed_up(cycled(2), frac(3, 2))
        ].into_iter());

        assert_eq!(
            display_pattern(&pattern),
            "[0, 3/2) | 1\n\
             [3/2, 3) | 1\n\
             [3, 9/2) | 1\n\
             [9/2, 5) | 1\n\
             [0, 2/3) | 2\n\
             [2/3, 4/3) | 2\n\
             [4/3, 2) | 2\n\
             [2, 8/3) | 2\n\
             [8/3, 10/3) | 2\n\
             [10/3, 4) | 2\n\
             [4, 14/3) | 2\n\
             [14/3, 5) | 2\n"
        )
    }

    #[test]
    fn test_combine() {
        let pattern = crate::pattern::combine(
            cycled(1),
            cycled(10),
            |a, b| a + b
        );

        assert_eq!(
            display_pattern(&pattern),
            "[0, 1) | 11\n\
             [1, 2) | 11\n\
             [2, 3) | 11\n\
             [3, 4) | 11\n\
             [4, 5) | 11\n"
        );

        let pattern1 = slowcat(vec![cycled(1), cycled(2)].into_iter());
        let pattern2 = speed_up(fastcat(vec![cycled(3), cycled(4)].into_iter()), frac(1, 3));

        // assert_eq!(
        //     display_pattern(pattern1),
        //     "[0, 1) | 1\n\
        //      [1, 2) | 2\n\
        //      [2, 3) | 1\n\
        //      [3, 4) | 2\n\
        //      [4, 5) | 1\n"
        // );

        // assert_eq!(
        //     display_pattern(pattern2),
        //     "[0, 3/2) | 3\n\
        //      [3/2, 3) | 4\n\
        //      [3, 9/2) | 3\n\
        //      [9/2, 5) | 4\n"
        // );

        let pattern_combined = crate::pattern::combine(
            pattern1,
            pattern2,
            |a, b| a * b
        );

        assert_eq!(
            display_pattern(&pattern_combined),
            "[0, 1) | 3\n\
             [1, 3/2) | 6\n\
             [3/2, 2) | 8\n\
             [2, 3) | 4\n\
             [3, 4) | 6\n\
             [4, 9/2) | 3\n\
             [9/2, 5) | 4\n"
        );
    }

    #[test]
    fn test_combine_left() {
        let pattern = crate::pattern::combine_left(
            fastcat(vec![cycled(1), cycled(2)].into_iter()).boxed(),
            fastcat(vec![cycled(5), cycled(10), cycled(15)].into_iter()).boxed(),
            |a, b| a + b
        );

        assert_eq!(
            display_pattern(&pattern),
            "[0, 1/2) | 6\n\
             [1/2, 1) | 12\n\
             [1, 3/2) | 6\n\
             [3/2, 2) | 12\n\
             [2, 5/2) | 6\n\
             [5/2, 3) | 12\n\
             [3, 7/2) | 6\n\
             [7/2, 4) | 12\n\
             [4, 9/2) | 6\n\
             [9/2, 5) | 12\n"
        );
    }

    #[test]
    fn test_combine_right() {
        let pattern = crate::pattern::combine_right(
            fastcat(vec![cycled(1), cycled(2)].into_iter()).boxed(),
            fastcat(vec![cycled(5), cycled(10), cycled(15)].into_iter()).boxed(),
            |a, b| a + b
        );

        assert_eq!(
            display_pattern(&pattern),
            "[0, 1/3) | 6\n\
             [1/3, 2/3) | 11\n\
             [2/3, 1) | 17\n\
             [1, 4/3) | 6\n\
             [4/3, 5/3) | 11\n\
             [5/3, 2) | 17\n\
             [2, 7/3) | 6\n\
             [7/3, 8/3) | 11\n\
             [8/3, 3) | 17\n\
             [3, 10/3) | 6\n\
             [10/3, 11/3) | 11\n\
             [11/3, 4) | 17\n\
             [4, 13/3) | 6\n\
             [13/3, 14/3) | 11\n\
             [14/3, 5) | 17\n"
        );
    }
    
    #[test]
    fn test_random_slowcat() {
        let pattern = random_slowcat(vec![cycled(1), cycled(2)].into_iter());
        let output = display_pattern(&pattern);
        // Since the output is random, we just check that it has the correct number of events.
        let lines: Vec<&str> = output.lines().collect();
        assert_eq!(lines.len(), 5);
    }

    #[test]
    fn test_scale() {
        let pattern = slowcat(vec![
            cycled(Note::new(0)),
            cycled(Note::new(2)),
            cycled(Note::new(4)),
        ].into_iter());

        let scaled_pattern = scale(pattern, slowcat(vec![
            cycled(Scale::CMajor),
            cycled(Scale::CMinor),
        ].into_iter()));

        assert_eq!(
            display_pattern(&scaled_pattern),
            "[0, 1) | Note(0)\n\
             [1, 2) | Note(3)\n\
             [2, 3) | Note(7)\n\
             [3, 4) | Note(0)\n\
             [4, 5) | Note(4)\n"
        );
    }

    #[test]
    fn test_structure() {
        let pattern = slowcat(vec![
            cycled(1),
            cycled(2),
            cycled(3),
        ].into_iter());

        let filter = slowcat(vec![
            cycled(()).boxed(),
            empty().boxed(),
        ].into_iter());

        let filtered_pattern = structure(pattern, filter);

        assert_eq!(
            display_pattern(&filtered_pattern),
            "[0, 1) | 1\n\
             [2, 3) | 3\n\
             [4, 5) | 2\n"
        );

        let pattern = fastcat(vec![
            cycled(1),
            cycled(2),
            cycled(3),
        ].into_iter());

        let filter = fastcat(vec![
            cycled(()).boxed(),
            empty().boxed(),
        ].into_iter());

        let filtered_pattern = structure(pattern, filter);

        assert_eq!(
            display_pattern(&filtered_pattern),
            "[0, 1/2) | 1\n\
             [1, 3/2) | 1\n\
             [2, 5/2) | 1\n\
             [3, 7/2) | 1\n\
             [4, 9/2) | 1\n"
        )
    }
}
