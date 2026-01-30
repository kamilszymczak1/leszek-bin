//! Pattern module implementing time-based musical patterns.
//!
//! This module provides the core pattern abstraction used throughout Leszek.
//! Patterns are functions from time segments to events, enabling expressive
//! composition of musical sequences.
//!
//! # Core Concepts
//!
//! - **Time**: Represented as arbitrary-precision rationals for exact timing
//! - **Segment**: A time interval `[start, end)` for querying patterns
//! - **Event**: A value occurring during a specific time segment
//! - **Pattern**: A function that produces events for any queried time segment
//!
//! # Pattern Combinators
//!
//! Sequential composition:
//! - `slowcat`: Alternate patterns across cycles (one pattern per cycle)
//! - `fastcat`: Alternate patterns within cycles (all patterns each cycle)
//!
//! Parallel composition:
//! - `in_parallel`: Play multiple patterns simultaneously
//! - `combine`: Combine overlapping events with a function
//!
//! Transformations:
//! - `speed_up`: Change pattern tempo
//! - `structure`: Apply rhythmic gating
//! - `scale`, `transpose`: Musical transformations

use num::BigRational;
use num::{self};
use rosc::OscType;

use anyhow::Result;

use crate::note::Note;
use crate::scale::{Scale, map_note};
use crate::segment::{Segment, cycle_segment_from_time};
use crate::time::{Time, frac};
use std::rc::Rc;

use crate::superdirt::ControlMessage;

/// Represents the temporal extent of an event.
///
/// Each event has a `part` (the actual sounding portion) and optionally
/// a `whole` (the theoretical complete duration before any slicing).
/// The distinction is important for onset detection and pattern alignment.
#[derive(PartialEq, Eq, Clone, PartialOrd, Ord)]
pub struct Part {
    /// The actual sounding portion of the event
    pub part: Segment,
    /// The theoretical whole duration (before slicing by queries)
    pub whole: Option<Segment>,
}

impl Part {
    fn new(part: Segment, whole: Option<Segment>) -> Part {
        Part { part, whole }
    }
    fn scaled(self, factor: BigRational) -> Part {
        Part::new(
            self.part.scaled(factor.clone()),
            self.whole.map(|w| w.scaled(factor)),
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

/// An event produced by a pattern query.
///
/// Events associate a value with its temporal location. When patterns
/// are combined, events may be split, merged, or filtered based on
/// their time segments.
#[derive(Clone, PartialEq, Eq)]
pub struct Event<T> {
    /// The temporal extent of this event
    pub part: Part,
    /// The value carried by this event
    pub value: T,
}

impl<T> Event<T> {
    fn new(part: Part, a: T) -> Self {
        Event { part, value: a }
    }

    fn scaled(self, factor: BigRational) -> Event<T> {
        Event::new(self.part.scaled(factor), self.value)
    }

    fn map_value<S>(self, f: impl Fn(T) -> S) -> Event<S> {
        Event::new(self.part, f(self.value))
    }
}

impl<T> std::fmt::Display for Event<T>
where
    T: std::fmt::Display,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} | {}", self.part, self.value)
    }
}

impl<T> std::fmt::Debug for Event<T>
where
    T: std::fmt::Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?} | {:?}", self.part, self.value)
    }
}

/// The core pattern trait: a function from time segments to events.
///
/// Patterns are queried with a time segment and return an iterator of
/// events that occur within that segment. This functional representation
/// enables lazy evaluation and infinite patterns.
///
/// # Implementation
///
/// Any closure `Fn(Segment) -> Iterator<Item = Event<T>>` automatically
/// implements `Pattern<T>`, making it easy to create custom patterns.
pub trait Pattern<T>
where
    T: Sized,
{
    type Events: Iterator<Item = Event<T>>;

    /// Query the pattern for events within the given time segment.
    fn query(&self, segment: Segment) -> Self::Events;

    /// Box this pattern for dynamic dispatch and storage.
    fn boxed(self) -> BoxPattern<T>
    where
        Self: Sized,
        Self: 'static,
        T: 'static,
    {
        let boxed_pattern: Rc<dyn Pattern<T, Events = BoxEvents<T>>> =
            Rc::new(move |segment| BoxEvents(Box::new(self.query(segment))));
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

/// Creates a pattern that repeats a single value every cycle.
///
/// This is the simplest pattern constructor. The value fills each
/// complete cycle, making it useful as a building block for more
/// complex patterns.
pub fn cycled<T: Clone>(a: T) -> impl Pattern<T> {
    move |segment: Segment| {
        let a = a.clone();
        segment.split_on_cycles().into_iter().map(move |cycle| {
            Event::new(
                Part::new(cycle.clone(), Some(cycle_segment_from_time(&cycle.start))),
                a.clone(),
            )
        })
    }
}

impl<T, I, F> Pattern<T> for F
where
    F: Fn(Segment) -> I,
    I: Iterator<Item = Event<T>>,
{
    type Events = I;

    fn query(&self, segment: Segment) -> Self::Events {
        (*self)(segment)
    }
}

pub struct BoxEvents<T>(Box<dyn Iterator<Item = Event<T>>>);

impl<T> Iterator for BoxEvents<T> {
    type Item = Event<T>;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }
}

/// A type-erased pattern for dynamic dispatch.
///
/// `BoxPattern` wraps any pattern implementation, enabling storage
/// in collections and recursive pattern definitions. The cost is
/// dynamic dispatch through `Rc` and boxed iterators.
#[derive(Clone)]
pub struct BoxPattern<T>(Rc<dyn Pattern<T, Events = BoxEvents<T>>>);

impl<T> Pattern<T> for BoxPattern<T> {
    type Events = BoxEvents<T>;

    fn query(&self, segment: Segment) -> Self::Events {
        self.0.query(segment)
    }
}

// Speed up a pattern by a given factor. E.g a factor of 2 makes the pattern run twice as fast.
pub fn speed_up<T, P>(pattern: P, factor: BigRational) -> impl Pattern<T>
where
    P: Pattern<T>,
{
    move |segment: Segment| {
        let factor_inv = factor.recip();
        let scaled_segment = segment.scaled(factor.clone());
        pattern
            .query(scaled_segment)
            .map(move |event| event.scaled(factor_inv.clone()))
    }
}

// Slowcat takes an iterator of patterns and alternates between them, giving each pattern
// a full cycle before moving to the next pattern.
// E.g slowcat([A, B]) produces <A | B | A | B | A | B ...>
// slowcat([A, B, C]) produces <A | B | C | A | B | C ...>
pub fn slowcat<T, I>(pats: I) -> impl Pattern<T>
where
    I: Iterator,
    I::Item: Pattern<T>,
{
    let patterns: Rc<[I::Item]> = pats.collect();
    move |segment: Segment| {
        let patterns = patterns.clone();
        segment
            .split_on_cycles()
            .into_iter()
            .flat_map(move |cycle| {
                let len = patterns.len();
                // If this cycle's index is i, then we want to query pattern[i % len].
                let index = cycle.start.cycle_index();
                let pattern = &patterns[index as usize % len];
                // We are querying this pattern index/len'th time, so that is the cycle we want to
                // query.
                let offset =
                    Time::new(index as i64, 1) - Time::new((index as usize / len) as i64, 1);
                pattern
                    .query(cycle.clone() - offset.clone())
                    .map(move |event| {
                        let off = offset.clone();
                        Event::new(
                            Part::new(
                                event.part.part + off.clone(),
                                event.part.whole.map(|w| w + off),
                            ),
                            event.value,
                        )
                    })
            })
    }
}

// Fastcat takes an iterator of patterns and alternates between them, but speeds up time
// so that each pattern gets an equal share of each cycle.
// E.g fastcat([A, B]) produces <A B | A B | A B ...>
// fastcat([A, B, C]) produces <A B C | A B C | A B C ...>
pub fn fastcat<T, I>(pats: I) -> impl Pattern<T>
where
    I: ExactSizeIterator,
    I::Item: Pattern<T>,
{
    let len = pats.len();
    speed_up(slowcat(pats), frac(len as i64, 1))
}

/// Creates a pattern of control messages from a pattern of OSC values.
///
/// This is used internally to convert value patterns into SuperDirt
/// control parameters like "s" (sound) or "n" (note number).
pub fn keyed(key: &'static str, values: impl Pattern<OscType>) -> impl Pattern<ControlMessage> {
    move |segment| {
        values.query(segment).map(move |event| {
            Event::new(
                event.part,
                ControlMessage::new(vec![(String::from(key), event.value)]),
            )
        })
    }
}

/// Plays multiple patterns simultaneously.
///
/// All patterns are queried for the same time segment and their
/// events are combined. This is the primary way to layer sounds
/// in a live coding performance.
pub fn in_parallel<T, I>(pats: I) -> impl Pattern<T>
where
    I: Iterator,
    I::Item: Pattern<T>,
{
    let patterns: Rc<[I::Item]> = pats.collect();
    move |segment: Segment| {
        patterns
            .iter()
            .flat_map(move |pattern| pattern.query(segment.clone()))
            .collect::<Vec<_>>()
            .into_iter()
    }
}

/// Combines two patterns by applying a function to overlapping events.
///
/// For each pair of events that overlap in time, produces a new event
/// with the intersection of their time segments and the combined value.
/// This is useful for operations like addition or modulation.
pub fn combine<P, Q, T, R, S, F>(first: P, second: Q, f: F) -> impl Pattern<S>
where
    P: Pattern<T>,
    Q: Pattern<R>,
    T: Clone,
    R: Clone,
    F: Fn(T, R) -> S + Clone,
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
                        f(fevent.value.clone(), sevent.value.clone()),
                    ));
                }
            }
        }
        results.into_iter()
    }
}

/// Combines patterns, preserving the structure of the first pattern.
///
/// Unlike `combine`, this only produces events for each event in the
/// first pattern, taking the first overlapping value from the second.
/// Useful when the first pattern defines the rhythmic structure.
pub fn combine_left<P, Q, T, R, S, F>(first: P, second: Q, f: F) -> impl Pattern<S>
where
    P: Pattern<T>,
    Q: Pattern<R>,
    T: Clone,
    R: Clone,
    F: Fn(T, R) -> S + Clone,
{
    move |segment: Segment| {
        let second_events: Vec<Event<R>> = second.query(segment.clone()).collect();

        let mut results = Vec::new();
        for fevent in first.query(segment.clone()) {
            if let Some(sevent) = second_events
                .iter()
                .filter(|sevent| fevent.part.part.intersection(&sevent.part.part).is_some())
                .min_by_key(|sevent| sevent.part.part.start.clone())
            {
                results.push(fevent.map_value(|v| f(v, sevent.value.clone())));
            }
        }
        results.into_iter()
    }
}

pub fn combine_right<P, Q, T, R, S, F>(first: P, second: Q, f: F) -> impl Pattern<S>
where
    P: Pattern<T>,
    Q: Pattern<R>,
    T: Clone,
    R: Clone,
    F: Fn(T, R) -> S + Clone,
{
    combine_left(second, first, move |b, a| f(a, b))
}

/// Applies a rhythmic structure to a pattern.
///
/// The filter pattern determines when events from the main pattern
/// are allowed through. This enables rhythmic gating effects like
/// `[1, ~, 1, ~]` to create syncopation.
pub fn structure<P, Q, T>(pattern: P, filter: Q) -> impl Pattern<T>
where
    P: Pattern<T>,
    Q: Pattern<()>,
    T: Clone,
{
    combine_right(pattern, filter, |a, _| a)
}

pub fn filter_output<T>(pat: impl Pattern<Option<T>>) -> impl Pattern<T> {
    move |segment: Segment| {
        pat.query(segment).filter_map(|ev| {
            if let Some(val) = ev.value {
                Some(Event::new(ev.part, val))
            } else {
                None
            }
        })
    }
}

pub fn map_output<T, U, F, P>(pat: P, f: F) -> impl Pattern<U>
where
    P: Pattern<T>,
    F: Fn(T) -> U + Clone,
{
    move |segment: Segment| {
        let f = f.clone();
        pat.query(segment)
            .map(move |ev| Event::new(ev.part, f(ev.value)))
    }
}

pub fn filter_map_output<T, U, F, P>(pat: P, f: F) -> impl Pattern<U>
where
    P: Pattern<T>,
    F: Fn(T) -> Option<U> + Clone,
{
    filter_output(map_output(pat, f))
}

pub fn filter_map_result_output<T, U, F, P>(pat: P, f: F) -> impl Pattern<U>
where
    P: Pattern<T>,
    F: Fn(T) -> Result<U> + Clone,
{
    filter_output(map_output(pat, move |input| match f(input) {
        Ok(output) => Some(output),
        Err(e) => {
            eprintln!("error in pattern while applying fallible function: {}", e);
            None
        }
    }))
}

/// Applies a musical scale to a pattern of notes.
///
/// Maps scale degrees (0, 1, 2, ...) to actual MIDI note offsets
/// based on the active scale pattern. Enables melodic patterns
/// that stay in key as the scale changes.
pub fn scale<P, Q>(pattern: P, scales: Q) -> impl Pattern<Note>
where
    P: Pattern<Note>,
    Q: Pattern<Scale>,
{
    combine_left(pattern, scales, |note, scale| map_note(&scale, note))
}

/// Transposes a pattern of notes by a root note.
///
/// Shifts all notes by the semitone value of the given note name.
/// Note names: c, ch (C#), d, dh (D#), e, f, fh (F#), g, gh (G#), a, ah (A#), b
pub fn transpose<P, Q>(pattern: P, scales: Q) -> impl Pattern<Note>
where
    P: Pattern<Note>,
    Q: Pattern<String>,
{
    combine_left(pattern, scales, |note, note_str| {
        Note::new(
            note.value()
                + match note_str.as_str() {
                    "c" => 0,
                    "ch" => 1,
                    "d" => 2,
                    "dh" => 3,
                    "e" => 4,
                    "f" => 5,
                    "fh" => 6,
                    "g" => 7,
                    "gh" => 8,
                    "a" => 9,
                    "ah" => 10,
                    "b" => 11,
                    _ => 0,
                },
        )
    })
}

/// Creates a pattern that produces no events (silence).
///
/// This is the identity element for parallel composition and useful
/// for creating rests in rhythmic patterns.
pub fn empty<T>() -> impl Pattern<T> {
    move |_: Segment| std::iter::empty()
}

/// Merges two control message patterns into one.
///
/// When events overlap, their control parameters are combined into
/// a single message. This enables layering effects like combining
/// note and velocity patterns.
pub fn merge<P0, P1>(pattern0: P0, pattern1: P1) -> impl Pattern<ControlMessage>
where
    P0: Pattern<ControlMessage>,
    P1: Pattern<ControlMessage>,
{
    combine_left(pattern0, pattern1, |msg0, msg1| {
        let mut msg = msg0.clone();
        msg.merge(&mut msg1.clone());
        msg
    })
}

#[cfg(test)]
mod tests {
    use crate::{
        note::Note,
        pattern::{
            Pattern, Segment, Time, cycled, display_pattern, empty, fastcat, in_parallel, scale,
            slowcat, speed_up, structure,
        },
        scale::Scale,
        segment::cycle_segment_from_time,
        time::frac,
    };

    #[test]
    fn test_time() {
        let seg = Segment::new(Time::new(1, 2), Time::new(28, 10));
        assert_eq!(
            seg.split_on_cycles(),
            vec![
                Segment::new(Time::new(1, 2), Time::new(1, 1)),
                Segment::new(Time::new(1, 1), Time::new(2, 1)),
                Segment::new(Time::new(2, 1), Time::new(28, 10)),
            ]
        );

        let small_seg = Segment::new(Time::new(1, 3), Time::new(2, 3));
        assert_eq!(small_seg.split_on_cycles(), vec![small_seg]);

        assert_eq!(Time::new(85, 9).cycle_start(), Time::new(9, 1));
        assert_eq!(Time::new(85, 9).cycle_index(), 9);
        assert_eq!(
            cycle_segment_from_time(&Time::new(13, 2)),
            Segment::new(Time::new(6, 1), Time::new(7, 1))
        );
    }

    #[test]
    fn test_cycled() {
        let pattern = cycled(42);
        assert_eq!(
            display_pattern(&pattern),
            "[0, 1) | 42\n\
             [1, 2) | 42\n\
             [2, 3) | 42\n\
             [3, 4) | 42\n\
             [4, 5) | 42\n"
        );
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
            display_pattern(&slowcat(
                vec![
                    cycled(0).boxed(),
                    slowcat(vec![cycled(1), cycled(2)].into_iter()).boxed()
                ]
                .into_iter()
            )),
            "[0, 1) | 0\n\
             [1, 2) | 1\n\
             [2, 3) | 0\n\
             [3, 4) | 2\n\
             [4, 5) | 0\n"
        );
    }

    #[test]
    fn test_fastcat() {
        println!(
            "{}",
            display_pattern(&fastcat(vec![cycled(1), cycled(2)].into_iter()))
        );
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

        let pattern = in_parallel(
            vec![
                speed_up(cycled(1), frac(2, 3)),
                speed_up(cycled(2), frac(3, 2)),
            ]
            .into_iter(),
        );

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
        let pattern = crate::pattern::combine(cycled(1), cycled(10), |a, b| a + b);

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

        let pattern_combined = crate::pattern::combine(pattern1, pattern2, |a, b| a * b);

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
            |a, b| a + b,
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
            |a, b| a + b,
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
    fn test_scale() {
        let pattern = slowcat(
            vec![
                cycled(Note::new(0)),
                cycled(Note::new(2)),
                cycled(Note::new(4)),
            ]
            .into_iter(),
        );

        let scaled_pattern = scale(
            pattern,
            slowcat(vec![cycled(Scale::Major), cycled(Scale::Minor)].into_iter()),
        );

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
        let pattern = slowcat(vec![cycled(1), cycled(2), cycled(3)].into_iter());

        let filter = slowcat(vec![cycled(()).boxed(), empty().boxed()].into_iter());

        let filtered_pattern = structure(pattern, filter);

        assert_eq!(
            display_pattern(&filtered_pattern),
            "[0, 1) | 1\n\
             [2, 3) | 3\n\
             [4, 5) | 2\n"
        );

        let pattern = fastcat(vec![cycled(1), cycled(2), cycled(3)].into_iter());

        let filter = fastcat(vec![cycled(()).boxed(), empty().boxed()].into_iter());

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
