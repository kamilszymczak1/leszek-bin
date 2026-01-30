//! Parser for the Leszek pattern language.
//!
//! This module parses text input into an abstract syntax tree (`Expr`).
//! It uses the [pest](https://pest.rs/) parser generator with a Pratt parser
//! for handling operator precedence (e.g., the `.` method chaining operator).
//!
//! # Syntax Overview
//!
//! The language supports:
//! - **Numbers**: `42`, `3.14`, `0.5`
//! - **Strings**: `"arpy"`, `"bd"`
//! - **Identifiers**: `n`, `slow`, `cat`
//! - **Function calls**: `slow(2, pattern)`, `n(fc([1, 2, 3]))`
//! - **Vectors**: `[1, 2, 3]`, `[[1, 2], [3, 4]]`
//! - **Method chaining**: `n(fc([1, 2])).s("arpy")` (using `.` operator)
//!
//! # Example
//!
//! ```text
//! par([
//!     n(scale(slow(2, cat(["cminor", "cmajor"])), fc([0, 3, 2, 1]))).s("arpy")
//! ])
//! ```

use std::sync::LazyLock;

use anyhow::{Result, anyhow};
use pest::Parser;
use pest::iterators::Pairs;
use pest::pratt_parser::{Assoc, Op, PrattParser};

use num::{BigRational, FromPrimitive};

use crate::lang::{Expr, apply, var};

/// The pest-generated parser using the grammar defined in `grammar.pest`.
#[derive(pest_derive::Parser)]
#[grammar = "grammar.pest"]
pub struct LangParser;

/// Pratt parser for handling operator precedence.
///
/// Currently only handles the `.` (dot) operator for method chaining,
/// which is left-associative: `a.b.c` parses as `((a.b).c)`.
static PRATT_PARSER: LazyLock<PrattParser<Rule>> = LazyLock::new(|| {
    use Assoc::*;
    use Rule::*;

    PrattParser::new()
        .op(Op::infix(dot, Left))
        .op(Op::infix(at, Left))
});

/// Parses a string into an expression AST.
///
/// This is the main entry point for parsing Leszek code.
///
/// # Arguments
///
/// * `input` - The source code to parse
///
/// # Returns
///
/// The parsed expression, or an error if parsing fails.
///
/// # Example
///
/// ```
/// let expr = parse("n(fc([1, 2, 3])).s(\"arpy\")")?;
/// ```
pub fn parse(input: &str) -> Result<Expr> {
    let mut pairs = LangParser::parse(Rule::expr, input)?;
    let program_pair = pairs.next().unwrap();
    parse_expr(program_pair.into_inner())
}

/// Parses a sequence of tokens into an expression.
///
/// Handles primary expressions (numbers, strings, identifiers, calls, vectors)
/// and infix operators (currently just `.` for method chaining).
fn parse_expr(pairs: Pairs<Rule>) -> Result<Expr> {
    PRATT_PARSER
        .map_primary(|primary| {
            match primary.as_rule() {
                Rule::number => {
                    // FIXME: parse rationals properly, instead of casting into f64 first
                    Ok(Expr::Number(
                        BigRational::from_f64(primary.as_str().parse::<f64>().unwrap()).unwrap(),
                    ))
                }
                Rule::string => {
                    // Strip the surrounding quotes
                    let s = primary.as_str();
                    Ok(Expr::Atom(s[1..s.len() - 1].to_string()))
                }
                Rule::identifier => Ok(Expr::Var(primary.as_str().into())),
                Rule::call => parse_call(primary.into_inner()),
                Rule::vector => {
                    // Recursively parse each element of the vector
                    let vec = primary
                        .into_inner()
                        .map(|pair| parse_expr(pair.into_inner()))
                        .collect::<Result<_>>()?;

                    Ok(Expr::Vector(vec))
                },
                Rule::empty => {
                    Ok(Expr::External("empty".to_string(), vec![]))
                }
                _ => Err(anyhow!("Unexpected primary: {:?}", primary)),
            }
        })
        .map_infix(|lhs, op, rhs| match op.as_rule() {
            Rule::dot => Ok(apply(rhs?, lhs?)),
            Rule::at => Ok(apply(apply(var("merge"), lhs?), rhs?)),
            _ => Err(anyhow!("Unexpected operator: {:?}", op)),
        })
        .parse(pairs)
}

/// Parses a function call into an expression.
///
/// Converts `func(arg1, arg2, arg3)` into curried application:
/// `((func arg1) arg2) arg3`
///
/// This allows partial application and matches the internal
/// representation used by the interpreter.
fn parse_call(mut pairs: Pairs<Rule>) -> Result<Expr> {
    let name = pairs.next().unwrap().as_str();

    let mut args: Vec<Expr> = vec![];
    for arg in pairs {
        args.push(parse_expr(arg.into_inner())?);
    }

    // Build curried application: func(a, b) → ((func a) b)
    args.reverse();
    let mut result_expr = Expr::Var(String::from(name));
    while !args.is_empty() {
        result_expr = Expr::Apply(Box::new(result_expr), Box::new(args.pop().unwrap()));
    }

    Ok(result_expr)
}

#[cfg(test)]
mod test {
    use crate::lang::*;
    use crate::parser::*;
    use crate::time::frac;

    #[test]
    fn test_parse() {
        assert_eq!(parse("2.5").unwrap(), Expr::Number(frac(5, 2)));
        assert_eq!(parse("a").unwrap(), Expr::Var(String::from("a")));
        assert_eq!(
            parse("two_args(1, 2)").unwrap(),
            Expr::Apply(
                Box::new(Expr::Apply(
                    Box::new(Expr::Var(String::from("two_args"))),
                    Box::new(Expr::Number(frac(1, 1)))
                )),
                Box::new(Expr::Number(frac(2, 1)))
            )
        );
        assert_eq!(
            parse("[[1, 2], test(3)]").unwrap(),
            Expr::Vector(vec![
                Expr::Vector(vec![Expr::Number(frac(1, 1)), Expr::Number(frac(2, 1)),]),
                apply(var("test"), Expr::Number(frac(3, 1)))
            ])
        );
        let test_string = "test 123_@";
        let wrapped_string = format!("\"{}\"", test_string);
        assert_eq!(parse(&wrapped_string).unwrap(), atom(test_string))
    }
}
