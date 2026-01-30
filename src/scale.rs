//! Musical scale definitions and note mapping.
//!
//! This module provides scale types and functions to convert scale degrees
//! (e.g., 0, 1, 2, 3...) into actual semitone offsets, allowing patterns
//! to be written in terms of scale positions rather than raw note values.

use anyhow::bail;

use crate::note::Note;

/// Represents a musical scale rooted at C.
///
/// Each scale defines a specific pattern of intervals (whole and half steps)
/// that determines which notes belong to the scale.
///
/// # Supported Scales
///
/// | Scale    | Intervals (semitones)     | Notes              |
/// |----------|---------------------------|--------------------|
/// | `CMajor` | 0, 2, 4, 5, 7, 9, 11      | C, D, E, F, G, A, B |
/// | `CMinor` | 0, 2, 3, 5, 7, 8, 10      | C, D, Eb, F, G, Ab, Bb |
#[derive(Debug, Clone)]
pub enum Scale {
    Minor,
    Major,
    Dorian,
    Phrygian,
    Lydian,
    Mixolydian,
    Locrian,

    // Pentatonics
    MajorPentatonic,
    MinorPentatonic,

    // Blues
    Blues,

    // Exotic / useful
    HarmonicMinor,
    MelodicMinor,
    WholeTone,
}

/// Returns the semitone offsets for each degree of the given scale.
///
/// For example, in C major:
/// - Degree 0 → 0 semitones (C)
/// - Degree 1 → 2 semitones (D)
/// - Degree 2 → 4 semitones (E)
/// - etc.
fn scale_mapping(scale: &Scale) -> Vec<i8> {
    match scale {
        Scale::Major => vec![0, 2, 4, 5, 7, 9, 11],
        Scale::Minor => vec![0, 2, 3, 5, 7, 8, 10],

        // Modes
        Scale::Dorian => vec![0, 2, 3, 5, 7, 9, 10],
        Scale::Phrygian => vec![0, 1, 3, 5, 7, 8, 10],
        Scale::Lydian => vec![0, 2, 4, 6, 7, 9, 11],
        Scale::Mixolydian => vec![0, 2, 4, 5, 7, 9, 10],
        Scale::Locrian => vec![0, 1, 3, 5, 6, 8, 10],

        // Pentatonics
        Scale::MajorPentatonic => vec![0, 2, 4, 7, 9],
        Scale::MinorPentatonic => vec![0, 3, 5, 7, 10],

        // Blues
        Scale::Blues => vec![0, 3, 5, 6, 7, 10],

        // Exotic / useful
        Scale::HarmonicMinor => vec![0, 2, 3, 5, 7, 8, 11],
        Scale::MelodicMinor => vec![0, 2, 3, 5, 7, 9, 11],
        Scale::WholeTone => vec![0, 2, 4, 6, 8, 10],
    }
}

/// Converts a note expressed in scale degrees to semitone offset.
///
/// This allows patterns to use simple sequential numbers (0, 1, 2, 3...)
/// that automatically map to the correct notes in the chosen scale.
///
/// Handles octave wrapping: degree 7 in a 7-note scale becomes degree 0
/// of the next octave (12 semitones higher).
///
/// # Examples
///
/// ```
/// // In C major, degree 2 is E (4 semitones above C)
/// map_note(&Scale::CMajor, Note::new(2)) // → Note(4)
///
/// // Degree 7 wraps to the next octave
/// map_note(&Scale::CMajor, Note::new(7)) // → Note(12) (C one octave up)
///
/// // Negative degrees go down
/// map_note(&Scale::CMajor, Note::new(-1)) // → Note(-1) (B below middle C)
/// ```
pub fn map_note(scale: &Scale, note: Note) -> Note {
    let mapping = scale_mapping(scale);
    let octave = note.value().div_euclid(mapping.len() as i8);
    let degree = note.value().rem_euclid(mapping.len() as i8);
    Note::new(octave * 12 + mapping[degree as usize])
}

impl TryInto<Scale> for String {
    type Error = anyhow::Error;

    fn try_into(self) -> Result<Scale, Self::Error> {
        match self.as_str() {
            "major" => Ok(Scale::Major),
            "minor" => Ok(Scale::Minor),

            // Modes
            "dorian" => Ok(Scale::Dorian),
            "phrygian" => Ok(Scale::Phrygian),
            "lydian" => Ok(Scale::Lydian),
            "mixolydian" => Ok(Scale::Mixolydian),
            "locrian" => Ok(Scale::Locrian),

            // Pentatonics
            "major5" => Ok(Scale::MajorPentatonic),
            "minor5" => Ok(Scale::MinorPentatonic),

            // Blues
            "blues" => Ok(Scale::Blues),

            // Minor variants
            "harmonicminor" => Ok(Scale::HarmonicMinor),
            "melodicminor" => Ok(Scale::MelodicMinor),

            // Symmetric / utility
            "whole" => Ok(Scale::WholeTone),
            _ => bail!("unknown scale '{}'", self),
        }
    }
}

#[cfg(test)]
mod test {

    use crate::note::Note;
    use crate::scale::{Scale, map_note};

    #[test]
    fn test_map_note() {
        assert_eq!(map_note(&Scale::Major, Note::new(0)), Note::new(0));
        assert_eq!(map_note(&Scale::Major, Note::new(-1)), Note::new(-1));
        assert_eq!(map_note(&Scale::Minor, Note::new(13)), Note::new(22));
    }
}
