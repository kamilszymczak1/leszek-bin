use crate::{lang, parser, pattern::BoxPattern, superdirt::ControlMessage};
use anyhow::Result;

/// Loads and parses a code string, returning the pattern to play.
pub fn parse_and_eval_code(code: &str) -> Result<BoxPattern<ControlMessage>> {
    let parsed = parser::parse(code)?;
    lang::eval_control_pattern(parsed)
}
