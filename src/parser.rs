use std::sync::LazyLock;

use anyhow::{Result, anyhow};
use pest::Parser;
use pest::iterators::Pairs;
use pest::pratt_parser::{Assoc, PrattParser};

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
                Rule::string => {
                    let s = primary.as_str();
                    Ok(Expr::Atom(s[1..s.len() - 1].to_string()))
                },
                Rule::identifier => {
                    Ok(Expr::Var(primary.as_str().into()))
                },
                Rule::call => {
                    parse_call(primary.into_inner())
                },
                Rule::vector => {
                    let vec = primary.into_inner()
                        .map(|pair| parse_expr(pair.into_inner()))
                        .collect::<Result<_>>()?;

                    Ok(Expr::Vector(vec))
                },
                _ => Err(anyhow!("Unexpected primary: {:?}", primary)),
            }
        })
        .parse(pairs)
}

fn parse_call(mut pairs: Pairs<Rule>) -> Result<Expr> {
    let name = pairs.next().unwrap().as_str();

    let mut args: Vec<Expr> = vec![];
    for arg in pairs {
        args.push(parse_expr(arg.into_inner())?);
    }

    args.reverse();
    let mut result_expr = Expr::Var(String::from(name));
    while !args.is_empty() {
        result_expr = Expr::Apply(Box::new(result_expr), Box::new(args.pop().unwrap()));
    }

    Ok(result_expr)
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
                Expr::Vector(vec![
                    Expr::Number(frac(1, 1)),
                    Expr::Number(frac(2, 1)),
                ]),
                apply(var("test"), Expr::Number(frac(3, 1)))
            ])
        );
        let test_string = "test 123_@";
        let wrapped_string = format!("\"{}\"", test_string);
        assert_eq!(
            parse(&wrapped_string).unwrap(),
            atom(test_string)
        )
    }
}
