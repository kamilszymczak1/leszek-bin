//! Note representation for musical patterns.
//!
//! This module provides a simple wrapper type for MIDI-style note values,
//! supporting arithmetic operations for transposition.

use std::ops::{Add, Sub};

/// A musical note represented as a signed 8-bit integer.
///
/// The value represents a semitone offset from middle C (C4), where each
/// increment corresponds to one semitone higher in pitch.
///
/// # Examples
///
/// | Value | Note |
/// |-------|------|
/// | `0`   | C (middle C) |
/// | `1`   | C# / Db |
/// | `2`   | D |
/// | `3`   | D# / Eb |
/// | `4`   | E |
/// | `5`   | F |
/// | `12`  | C (one octave up) |
/// | `-1`  | B (one semitone below middle C) |
/// | `-12` | C (one octave down) |
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Note(i8);

impl Note {
    /// Creates a new note with the given value.
    pub fn new(note: i8) -> Self {
        Note(note)
    }

    /// Returns the underlying note value.
    pub fn value(&self) -> i8 {
        self.0
    }
}

impl std::fmt::Display for Note {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Note({})", self.0)
    }
}

/// Transposes the note up by the given number of semitones.
impl Add<i8> for Note {
    type Output = Note;

    fn add(self, rhs: i8) -> Self::Output {
        Note(self.0 + rhs)
    }
}

/// Transposes the note down by the given number of semitones.
impl Sub<i8> for Note {
    type Output = Note;

    fn sub(self, rhs: i8) -> Self::Output {
        Note(self.0 - rhs)
    }
}
