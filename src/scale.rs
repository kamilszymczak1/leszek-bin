use crate::note::Note;

#[derive(Debug, Clone)]
pub enum Scale {
    CMinor,
    CMajor,
}

// Returns a mapping from scale degrees to semitone offsets within an octave
fn scale_mapping(scale : &Scale) -> Vec<i8> {
    match scale {
        Scale::CMinor => vec![0, 2, 3, 5, 7, 8, 10],
        Scale::CMajor => vec![0, 2, 4, 5, 7, 9, 11],
    }
}

// Maps a note in scale degrees to a note in semitones
pub fn map_note(scale : &Scale, note : Note) -> Note {
    let mapping = scale_mapping(scale);
    let octave = note.value().div_euclid(mapping.len() as i8);
    let degree = note.value().rem_euclid(mapping.len() as i8);
    Note::new(octave * 12 + mapping[degree as usize])
}

impl TryInto<Scale> for String {
    type Error = ();

    fn try_into(self) -> Result<Scale, Self::Error> {
        match self.as_str() {
            "cminor" => Ok(Scale::CMinor),
            "cmajor" => Ok(Scale::CMajor),
            _ => Err(())
        }
    }
}

#[cfg(test)]
mod test {

    use crate::scale::{Scale, map_note};
    use crate::note::Note;

    #[test]
    fn test_map_note() {
        assert_eq!(map_note(&Scale::CMajor, Note::new(0)), Note::new(0));
        assert_eq!(map_note(&Scale::CMajor, Note::new(-1)), Note::new(-1));
        assert_eq!(map_note(&Scale::CMinor, Note::new(13)), Note::new(22));
    }
}
