use std::collections::HashMap;

use anyhow::Context;
use num::BigRational;
use num::FromPrimitive;

use rosc::OscType;

use anyhow::{anyhow, bail, Result};

use crate::note::Note;
use crate::pattern::{self, fastcat, filter_map_output, filter_map_result_output, in_parallel, speed_up, empty};
use crate::pattern::{BoxPattern, Pattern, filter_output, map_output, slowcat};
use crate::superdirt::ControlMessage;
use crate::time;

use std::collections::HashSet;

use std::sync::LazyLock;

use num::ToPrimitive;

static EXTERNAL_ARG1: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
    HashSet::from([
        "cat",
        "fc",
        "s",
        "n",
        "par",
        "velocity",
        "clip",
        "delay",
        "room",
        "modamp",
        "accelerate",
    ])
});

static EXTERNAL_ARG2: LazyLock<HashSet<&'static str>> =
    LazyLock::new(|| HashSet::from(["scale", "struct", "fast", "slow", "add", "merge"]));

#[derive(Clone, Debug, PartialEq)]
pub enum Expr {
    Lambda(String, Box<Expr>),
    Apply(Box<Expr>, Box<Expr>),
    Atom(String),
    Number(BigRational),
    External(String, Vec<Expr>),
    Vector(Vec<Expr>),
    Var(String),
}

pub fn var(str: &str) -> Expr {
    Expr::Var(String::from(str))
}

pub fn apply(f: Expr, arg: Expr) -> Expr {
    Expr::Apply(Box::new(f), Box::new(arg))
}

#[allow(dead_code)]
pub fn atom(str: &str) -> Expr {
    Expr::Atom(String::from(str))
}

#[derive(Clone)]
pub enum Value {
    Lambda(String, Expr),
    Atom(String),
    Number(BigRational),
    Message(ControlMessage),
    Pattern(BoxPattern<Value>),
    Vector(Vec<Value>),
}

impl std::fmt::Debug for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        match self {
            Value::Pattern(pat) => {
                write!(f, "Pattern({})", pattern::display_pattern(pat))
            }
            Value::Atom(str) => {
                write!(f, "Atom({})", str)
            }
            Value::Number(num) => {
                write!(f, "Number({})", num)
            }
            Value::Vector(vec) => {
                write!(f, "Vector({:?})", vec)
            }
            Value::Lambda(name, arg) => {
                write!(f, "Lambda({}, {:?})", name, arg)
            }
            Value::Message(message) => {
                write!(f, "Message({:?})", message)
            }
        }
    }
}

impl std::fmt::Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

fn _as_vector(val: Value) -> Option<Vec<Value>> {
    val.try_into().ok()
}

fn _as_pattern(val: Value) -> Option<BoxPattern<Value>> {
    val.try_into().ok()
}

fn as_atom(val: Value) -> Option<String> {
    val.try_into().ok()
}

fn _as_control(val: Value) -> Option<ControlMessage> {
    val.try_into().ok()
}

fn as_number(val: Value) -> Option<BigRational> {
    val.try_into().ok()
}

fn to_pattern(value: Value) -> BoxPattern<Value> {
    if let Value::Pattern(pat) = value {
        pat
    } else {
        pattern::cycled(value).boxed()
    }
}

fn try_pattern(val: Value) -> Result<BoxPattern<Value>> {
    match val {
        Value::Pattern(pat) => Ok(pat),
        other => bail!("expected pattern, got {:?}", other),
    }
}

fn try_vector(val: Value) -> Result<Vec<Value>> {
    match val {
        Value::Vector(vec) => Ok(vec),
        other => bail!("expected vector, got {:?}", other),
    }
}

fn try_atom(val: Value) -> Result<String> {
    match val {
        Value::Atom(s) => Ok(s),
        other => bail!("expected atom, got {:?}", other),
    }
}

fn try_control(val: Value) -> Result<ControlMessage> {
    match val {
        Value::Message(msg) => Ok(msg),
        other => bail!("expected control message, got {:?}", other),
    }
}

fn try_number(val: Value) -> Result<BigRational> {
    val.try_into()
}

impl TryFrom<Value> for BigRational {
    type Error = anyhow::Error;

    fn try_from(value: Value) -> Result<Self> {
        match value {
            Value::Number(n) => Ok(n),
            other => bail!("expected number, got {:?}", other),
        }
    }
}

impl TryFrom<Value> for String {
    type Error = anyhow::Error;

    fn try_from(value: Value) -> Result<Self> {
        match value {
            Value::Atom(s) => Ok(s),
            other => bail!("expected atom, got {:?}", other),
        }
    }
}

impl TryFrom<Value> for Vec<Value> {
    type Error = anyhow::Error;

    fn try_from(value: Value) -> Result<Self> {
        match value {
            Value::Vector(v) => Ok(v),
            other => bail!("expected vector, got {:?}", other),
        }
    }
}

impl TryFrom<Value> for BoxPattern<Value> {
    type Error = anyhow::Error;

    fn try_from(value: Value) -> Result<Self> {
        match value {
            Value::Pattern(p) => Ok(p),
            other => bail!("expected pattern, got {:?}", other),
        }
    }
}

impl TryFrom<Value> for ControlMessage {
    type Error = anyhow::Error;

    fn try_from(value: Value) -> Result<Self> {
        match value {
            Value::Message(m) => Ok(m),
            other => bail!("expected control message, got {:?}", other),
        }
    }
}


fn compute_external(name: String, args: Vec<Value>) -> Result<Value> {
    let arg1 = |args: Vec<Value>| -> Result<Value> {
        match &args[..] {
            [a] => Ok(a.clone()),
            _ => bail!("expected 1 arguments to {}, but got {} instead", &name, args.len()),
        }
    };

    let arg2 = |args: Vec<Value>| -> Result<(Value, Value)> {
        match &args[..] {
            [a1, a2] => Ok((a1.clone(), a2.clone())),
            _ => bail!("expected 2 arguments to {}, but got {} instead", &name, args.len()),
        }
    };

    let key = |key: &'static str, args: Vec<Value>| -> Result<Value> {
        let pat = to_pattern(arg1(args)?);
        let sound_pat = filter_output(map_output(pat, |val: Value| -> Option<OscType> {
            Some(OscType::String(as_atom(val)?))
        }));
        let output_pat = map_output(pattern::keyed(key, sound_pat), Value::Message).boxed();
        Ok(Value::Pattern(output_pat))
    };

    let key_number = |key: &'static str, args: Vec<Value>| -> Result<Value> {
        let pat = to_pattern(arg1(args)?);
        let sound_pat = filter_output(map_output(pat, |val: Value| -> Option<OscType> {
            Some(OscType::Float(as_number(val)?.to_f32().unwrap()))
        }));
        let output_pat = map_output(pattern::keyed(key, sound_pat), Value::Message).boxed();
        Ok(Value::Pattern(output_pat))
    };

    match &name as &str {
        "cat" => Ok(Value::Pattern(
            slowcat(try_vector(arg1(args)?)?.into_iter().map(to_pattern)).boxed(),
        )),
        "fc" => Ok(Value::Pattern(
            fastcat(try_vector(arg1(args)?)?.into_iter().map(to_pattern)).boxed(),
        )),
        "par" => Ok(Value::Pattern(
            in_parallel(try_vector(arg1(args)?)?.into_iter().map(to_pattern)).boxed(),
        )),
        "s" => key("s", args),
        "fast" => {
            let (rate, pat) = arg2(args)?;
            let out_pat = speed_up(to_pattern(pat), try_number(rate)?);
            Ok(Value::Pattern(out_pat.boxed()))
        }
        "slow" => {
            let (rate, pat) = arg2(args)?;
            let out_pat = speed_up(try_pattern(pat)?, time::frac(1, 1) / try_number(rate)?);
            Ok(Value::Pattern(out_pat.boxed()))
        }
        "merge" => {
            let (pat0, pat1) = arg2(args)?;
            let p0 = filter_map_result_output(to_pattern(pat0), try_control);
            let p1 = filter_map_result_output(to_pattern(pat1), try_control);
            Ok(Value::Pattern(
                map_output(pattern::merge(p0, p1), Value::Message).boxed(),
            ))
        }
        "scale" => {
            let (scale_pat, note_pat) = arg2(args)?;
            let scale_pat =
                filter_map_result_output(to_pattern(scale_pat), |val| try_atom(val)?.try_into());
            let note_pat =
                filter_map_output(filter_map_result_output(to_pattern(note_pat), try_number), |num| {
                    Some(Note::new(num.floor().to_i8()?))
                });
            Ok(Value::Pattern(
                filter_map_output(pattern::scale(note_pat, scale_pat), |note| {
                    Some(Value::Number(BigRational::from_i8(note.value())?))
                })
                .boxed(),
            ))
        }
        "struct" => {
            let (arg0, arg1) = arg2(args)?;
            let filter_pat = to_pattern(arg0);
            let pat = to_pattern(arg1);
            Ok(Value::Pattern(
                pattern::structure(pat, map_output(filter_pat, |_| ())).boxed(),
            ))
        }
        "add" => {
            let (arg0, arg1) = arg2(args)?;
            let pat0 = filter_map_result_output(to_pattern(arg0), try_number);
            let pat1 = filter_map_result_output(to_pattern(arg1), try_number);
            let pat_result = pattern::combine(pat0, pat1, |l, r| l + r);
            Ok(Value::Pattern(
                map_output(pat_result, Value::Number).boxed(),
            ))
        }
        "empty" => {
            Ok(Value::Pattern(empty().boxed()))
        }
        "modamp" => key_number("modamp", args),
        "accelerate" => key_number("accelerate", args),
        "velocity" => key_number("velocity", args),
        "clip" => key_number("clip", args),
        "delay" => key_number("delay", args),
        "room" => key_number("room", args),
        "n" => key_number("n", args),
        _ => {
            bail!("invalid external {}", name);
        }
    }
}

fn wrap_f1(fresh_var0: String, name: String) -> Value {
    let a = fresh_var0;
    Value::Lambda(a.clone(), Expr::External(name, vec![Expr::Var(a)]))
}

fn wrap_f2(fresh_var0: String, fresh_var1: String, name: String) -> Value {
    let a = fresh_var0;
    let b = fresh_var1;
    Value::Lambda(
        a.clone(),
        Expr::Lambda(
            b.clone(),
            Box::new(Expr::External(name, vec![Expr::Var(a), Expr::Var(b)])),
        ),
    )
}

#[derive(Debug, Clone)]
pub struct Environment {
    fresh_counter: u16,
    variable_map: HashMap<String, Value>,
}

impl Environment {
    fn new() -> Self {
        Self {
            fresh_counter: 0,
            variable_map: HashMap::new(),
        }
    }
}

pub fn eval_control_pattern(expr: Expr) -> Result<BoxPattern<ControlMessage>> {
    let value_pattern = eval_pattern(expr)?;
    Ok(filter_map_result_output(value_pattern, try_control).boxed())
}

pub fn eval_pattern(expr: Expr) -> Result<BoxPattern<Value>> {
    let value = eval_with(&mut Environment::new(), expr)
        .with_context(|| "while trying to evaluate code as a pattern".to_string())?;

    match value {
        Value::Pattern(pat) => Ok(pat),
        _ => bail!("failed evaluating code as a pattern, got value {} instead", value)
    }
}

fn eval_with(env: &mut Environment, expr: Expr) -> Result<Value> {
    let mut get_fresh_var = || {
        let fresh_var = format!("fresh{}", env.fresh_counter);
        env.fresh_counter += 1;
        fresh_var
    };

    match expr {
        Expr::External(name, args) => {
            let arg_values: Vec<Value> = args
                .into_iter()
                .map(|e| eval_with(env, e))
                .collect::<Result<Vec<_>>>()
                .with_context(|| format!("while evaluating arguments to '{}'", name))?;

            compute_external(name, arg_values)
        }
        Expr::Apply(f, arg) => {
            let (name, subexpr) = match eval_with(env, *f).with_context(|| "while evaluating function of function application".to_string())? {
                Value::Lambda(name, subexpr) => Ok::<(String, Expr), anyhow::Error>((name, subexpr)),
                val => bail!("tried applying a non-function value {val}"),
            }?;

            let arg_value = eval_with(env, *arg).with_context(|| "while evaluating argument of function application")?;
            let old_value = env.variable_map.insert(name.clone(), arg_value);

            let result = eval_with(env, subexpr)?;
            if let Some(old_value) = old_value {
                env.variable_map.insert(name, old_value);
            }
            Ok(result)
        }
        Expr::Lambda(name, expr) => Ok(Value::Lambda(name, *expr)),
        Expr::Number(num) => Ok(Value::Number(num)),
        Expr::Atom(atom) => Ok(Value::Atom(atom)),
        Expr::Vector(elements) => Ok(Value::Vector(
            elements
                .into_iter()
                .map(|expr| eval_with(env, expr))
                .collect::<Result<_>>()
                .with_context(|| "while evaluating elements of a vector".to_string())?
        )),
        Expr::Var(name) => {
            if EXTERNAL_ARG1.contains(name.as_str()) {
                Ok(wrap_f1(get_fresh_var(), name))
            } else if EXTERNAL_ARG2.contains(name.as_str()) {
                Ok(wrap_f2(get_fresh_var(), get_fresh_var(), name))
            } else {
                let res = env.variable_map.get(&name).cloned();
                res.ok_or_else(|| anyhow!("variable not {} found", name))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lang::*;
    use crate::time;

    #[test]
    fn test_eval() {
        let val = eval_with(
            &mut Environment::new(),
            Expr::Apply(
                Box::new(Expr::Var(String::from("slowcat"))),
                Box::new(Expr::Vector(vec![
                    Expr::Number(time::frac(0, 1)),
                    Expr::Number(time::frac(1, 1)),
                ])),
            ),
        );

        assert_eq!(
            format!("{:?}", val),
            "Some(Pattern([0, 1) | Number(0)\n\
            [1, 2) | Number(1)\n\
            [2, 3) | Number(0)\n\
            [3, 4) | Number(1)\n\
            [4, 5) | Number(0)\n\
            ))"
        )
    }
}
