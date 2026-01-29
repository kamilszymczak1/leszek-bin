use std::sync::LazyLock;

use anyhow::{Result, anyhow};
use pest::Parser;
use pest::iterators::{Pair, Pairs};
use pest::pratt_parser::{Assoc, Op, PrattParser};

use num::{BigRational, FromPrimitive};

use crate::lang::Expr;

#[derive(pest_derive::Parser)]
#[grammar = "grammar.pest"]
pub struct LangParser;

static PRATT_PARSER: LazyLock<PrattParser<Rule>> = LazyLock::new(|| {
    use Rule::*;
    use Assoc::*;

    PrattParser::new()
});

pub fn parse(input: &str) -> Result<Expr> {
    let mut pairs = LangParser::parse(Rule::expr, input)?;
    let program_pair = pairs.next().unwrap();
    parse_expr(program_pair.into_inner())
}

fn parse_expr(pairs: Pairs<Rule>) -> Result<Expr> {
    PRATT_PARSER
        .map_primary(|primary| {
            match primary.as_rule() {
                Rule::number => {
                    // FIXME: parse rationals properly, instead of casting into f64 first
                    Ok(Expr::Number(BigRational::from_f64(primary.as_str().parse::<f64>().unwrap()).unwrap()))
                },
                Rule::identifier => {
                    Ok(Expr::Var(primary.as_str().into()))
                }
                _ => Err(anyhow!("Unexpected primary: {:?}", primary)),
            }
        })
        .parse(pairs)
}

#[cfg(test)]
mod test {
    use crate::parser::*;
    use crate::lang::*;
    use crate::pattern::frac;

    #[test]
    fn test_parse() {
        assert_eq!(parse("2.5").unwrap(), Expr::Number(frac(5, 2)));
        assert_eq!(parse("a").unwrap(), Expr::Var(String::from("a")));
    }
}
