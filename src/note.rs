use std::ops::{Add, Sub};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Note(i8);

impl Note {
    pub fn new(note: i8) -> Self {
        Note(note)
    }

    pub fn value(&self) -> i8 {
        self.0
    }
}

impl std::fmt::Display for Note {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Note({})", self.0)
    }
}

impl Add<i8> for Note {
    type Output = Note;

    fn add(self, rhs: i8) -> Self::Output {
        Note(self.0 + rhs)
    }
}

impl Sub<i8> for Note {
    type Output = Note;

    fn sub(self, rhs: i8) -> Self::Output {
        Note(self.0 - rhs)
    }
}