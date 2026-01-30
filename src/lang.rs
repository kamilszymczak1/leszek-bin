use std::collections::HashMap;

use num::BigRational;
use num::FromPrimitive;

use rosc::OscType;

use crate::note::Note;
use crate::pattern::{self, fastcat, filter_map_output, in_parallel, speed_up, structure};
use crate::pattern::{BoxPattern, Pattern, slowcat, map_output, filter_output};
use crate::superdirt::ControlMessage;

use num::ToPrimitive;

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
    Vector(Vec<Value>)
}

impl std::fmt::Debug for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> { 
        match self {
            Value::Pattern(pat) => {
                write!(f, "Pattern({})", pattern::display_pattern(pat))
            },
            Value::Atom(str) => {
                write!(f, "Atom({})", str)
            }
            Value::Number(num) => {
                write!(f, "Number({})", num)
            },
            Value::Vector(vec) => {
                write!(f, "Vector({:?})", vec)
            },
            Value::Lambda(name, arg) => {
                write!(f, "Lambda({}, {:?})", name, arg)
            },
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

fn to_pattern(value: Value) -> BoxPattern<Value> {
    if let Value::Pattern(pat) = value {
        pat
    } else {
        pattern::cycled(value).boxed()
    }
}

fn get_vector(val: Value) -> Option<Vec<Value>> {
    if let Value::Vector(vec) = val {
        Some(vec)
    } else {
        None
    }
}

fn as_pattern(val: Value) -> Option<BoxPattern<Value>> {
    if let Value::Pattern(pat) = val {
        Some(pat)
    } else {
        None
    }
}

fn as_atom(val: Value) -> Option<String> {
    if let Value::Atom(str) = val {
        Some(str)
    } else {
        None
    }
}

fn as_control(val: Value) -> Option<ControlMessage> {
    if let Value::Message(msg) = val {
        Some(msg)
    } else {
        None
    }
}

fn as_number(val: Value) -> Option<BigRational> {
    if let Value::Number(num) = val {
        Some(num)
    } else {
        None
    }
}

fn compute_external(name: String, mut args: Vec<Value>) -> Option<Value> {
    fn arg1(args: Vec<Value>) -> Option<Value> {
        match &args[..] {
            [a] => {
                Some(a.clone())
            },
            _ => None
        }
    }

    fn arg2(args: Vec<Value>) -> Option<(Value, Value)> {
        match &args[..] {
            [a1, a2] => {
                Some((a1.clone(), a2.clone()))
            },
            _ => None
        }
    }

    fn key(key: &'static str, args: Vec<Value>) -> Option<Value> {
        let pat = to_pattern(arg1(args)?);
        let sound_pat = filter_output(map_output(pat, |val: Value| -> Option<OscType> { Some(OscType::String(as_atom(val)?)) }));
        let output_pat = map_output(pattern::keyed(key, sound_pat), Value::Message).boxed();
        Some(Value::Pattern(output_pat))
    }

    fn key_number(key: &'static str, args: Vec<Value>) -> Option<Value> {
        let pat = to_pattern(arg1(args)?);
        let sound_pat = filter_output(map_output(pat, |val: Value| -> Option<OscType> { Some(OscType::Float(as_number(val)?.to_f32().unwrap())) }));
        let output_pat = map_output(pattern::keyed(key, sound_pat), Value::Message).boxed();
        Some(Value::Pattern(output_pat))
    }

    match &name as &str {
        "slowcat" => {
            Some(Value::Pattern(slowcat(get_vector(arg1(args)?)?.into_iter().map(to_pattern)).boxed()))
        },
        "fastcat" => {
            Some(Value::Pattern(fastcat(get_vector(arg1(args)?)?.into_iter().map(to_pattern)).boxed()))
        },
        "par" => {
            Some(Value::Pattern(in_parallel(get_vector(arg1(args)?)?.into_iter().map(to_pattern)).boxed()))
        }
        "sound" => {
            key("s", args)
        },
        "fast" => {
            let (rate, pat) = arg2(args)?;
            let out_pat = speed_up(
                to_pattern(pat),
                as_number(rate)?,
            );
            Some(Value::Pattern(out_pat.boxed()))
        }
        "slow" => {
            let (rate, pat) = arg2(args)?;
            let out_pat = speed_up(
                as_pattern(pat)?,
                pattern::frac(1, 1) / as_number(rate)?,
            );
            Some(Value::Pattern(out_pat.boxed()))
        }
        "merge" => {
            let (pat0, pat1) = arg2(args)?;
            let p0 = filter_map_output(to_pattern(pat0), as_control);
            let p1 = filter_map_output(to_pattern(pat1), as_control);
            Some(Value::Pattern(map_output(pattern::merge(p0, p1), Value::Message).boxed()))
        },
        "scale" => {
            let (scale_pat, note_pat) = arg2(args)?;
            let scale_pat = filter_map_output(to_pattern(scale_pat), |val| as_atom(val)?.try_into().ok());
            let note_pat = filter_map_output(
                filter_map_output(to_pattern(note_pat), as_number), 
                |num| { Some(Note::new(num.floor().to_i8()?)) }
                );
            Some(Value::Pattern(filter_map_output(pattern::scale(note_pat, scale_pat), |note| Some(Value::Number(BigRational::from_i8(note.value())?))).boxed()))
        },
        "struct" => {
            let (arg0, arg1) = arg2(args)?;
            let pat = to_pattern(arg0);
            let filter_pat = to_pattern(arg1);
            Some(Value::Pattern(pattern::structure(pat, map_output(filter_pat, |_| ())).boxed()))
        },
        "add" => {
            let (arg0, arg1) = arg2(args)?;
            let pat0 = filter_map_output(to_pattern(arg0), as_number);
            let pat1 = filter_map_output(to_pattern(arg1), as_number);
            let pat_result = pattern::combine(pat0, pat1, |l, r| l + r);
            Some(Value::Pattern(map_output(pat_result, Value::Number).boxed()))
        },
        "velocity" => key_number("velocity", args),
        "clip" => key_number("clip", args),
        "delay" => key_number("delay", args),
        "room" => key_number("room", args),
        "n" => key_number("n", args),
        _ => { todo!() }
    }
}

fn wrap_f1(fresh_var0: String, name: String) -> Value {
    let a = fresh_var0;
    Value::Lambda(a.clone(),
        Expr::External(name, vec![Expr::Var(a)])
    )
}

fn wrap_f2(fresh_var0: String, fresh_var1: String, name: String) -> Value {
    let a = fresh_var0;
    let b = fresh_var1;
    Value::Lambda(a.clone(),
        Expr::Lambda(b.clone(),
            Box::new(Expr::External(name, vec![Expr::Var(a), Expr::Var(b)]))
        )
    )
}

#[derive(Debug, Clone)]
pub struct Environment {
    fresh_counter: u16,
    variable_map: HashMap<String, Value>,
}

impl Environment {
    fn new() -> Self {
        Self { fresh_counter: 0, variable_map: HashMap::new() }
    }
}

pub fn eval_control_pattern(expr: Expr) -> Option<BoxPattern<ControlMessage>> {
    Some(filter_output(map_output(eval_pattern(expr)?, as_control)).boxed())
}

pub fn eval_pattern(expr: Expr) -> Option<BoxPattern<Value>> {
    let value = eval_with(&mut Environment::new(), expr)?;
    dbg!(value.clone());
    if let Value::Pattern(pat) = value {
        return Some(pat)
    }

    None
}

fn eval_with(env: &mut Environment, expr: Expr) -> Option<Value> {
    dbg!(&env, expr.clone());
    let mut get_fresh_var = || {
        let fresh_var = String::from(format!("fresh{}", env.fresh_counter));
        env.fresh_counter += 1;
        fresh_var
    };

    match expr {
        Expr::External(name, args) => {
            let arg_values: Vec<Value> = 
                args
                .into_iter()
                .map(|e| eval_with(env, e))
                .collect::<Option<Vec<_>>>()?;

            compute_external(name, arg_values)
        },
        Expr::Apply(f, arg) => {
            let (name, subexpr) =
                match eval_with(env, *f)? {
                    Value::Lambda(name, subexpr) => { Some((name, subexpr)) },
                    _ => None,
                }?;
            let arg_value = eval_with(env, *arg)?;
            dbg!("apply", env.clone(), name.clone(), subexpr.clone(), arg_value.clone());
            let old_value = env.variable_map.insert(name.clone(), arg_value);

            let result = eval_with(env, subexpr)?;
            if let Some(old_value) = old_value {
                env.variable_map.insert(name, old_value);
            }
            Some(result)
        },
        Expr::Lambda(name, expr) => {
            Some(Value::Lambda(name, *expr))
        },
        Expr::Number(num) => {
            Some(Value::Number(num))
        },
        Expr::Atom(atom) => {
            Some(Value::Atom(atom))
        },
        Expr::Vector(elements) => {
            Some(Value::Vector(elements.into_iter().map(|expr| eval_with(env, expr)).collect::<Option<_>>()?))
        }
        Expr::Var(name) => {
            match &name as &str {
                "slowcat" => { Some(wrap_f1(get_fresh_var(), String::from("slowcat"))) },
                "cat" => { Some(wrap_f1(get_fresh_var(), String::from("slowcat"))) },
                "fc" => { Some(wrap_f1(get_fresh_var(), String::from("fastcat"))) },
                "s" => { Some(wrap_f1(get_fresh_var(), String::from("sound"))) },
                "n" => { Some(wrap_f1(get_fresh_var(), String::from("n"))) }
                "par" => { Some(wrap_f1(get_fresh_var(), String::from("par"))) }
                "fast" => { Some(wrap_f2(get_fresh_var(), get_fresh_var(), String::from("fast"))) }
                "slow" => { Some(wrap_f2(get_fresh_var(), get_fresh_var(), String::from("slow"))) }
                "add" => { Some(wrap_f2(get_fresh_var(), get_fresh_var(), String::from("add"))) }
                "merge" => { Some(wrap_f2(get_fresh_var(), get_fresh_var(), String::from("merge"))) }
                "velocity" => { Some(wrap_f1(get_fresh_var(), String::from("velocity"))) }
                "clip" => { Some(wrap_f1(get_fresh_var(), String::from("clip"))) }
                "delay" => { Some(wrap_f1(get_fresh_var(), String::from("delay"))) }
                "room" => { Some(wrap_f1(get_fresh_var(), String::from("room"))) }
                "scale" => { Some(wrap_f2(get_fresh_var(), get_fresh_var(), String::from("scale"))) }
                "struct" => { Some(wrap_f2(get_fresh_var(), get_fresh_var(), String::from("struct"))) }
                _ => { 
                    let res = env.variable_map.get(&name).cloned();
                    if res.is_none() {
                        println!("ERROR: variable {} not found", name);
                    }
                    res
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::pattern;
    use crate::lang::*;

    #[test]
    fn test_eval() {
        let val = eval_with(&mut Environment::new(), 
            Expr::Apply(
                Box::new(Expr::Var(String::from("slowcat"))),
                Box::new(Expr::Vector(vec![
                        Expr::Number(pattern::frac(0, 1)), 
                        Expr::Number(pattern::frac(1, 1))
                ])),
            )
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
