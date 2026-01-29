use std::collections::HashMap;

use num::BigRational;

use crate::pattern;
use crate::{pattern::{BoxPattern, Pattern, slowcat}, superdirt::ControlMessage};

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

fn compute_external(name: String, mut args: Vec<Value>) -> Option<Value> {
    let mut arg1 = move || -> Option<Value> {
        if args.len() == 1 {
            Some(args.pop().unwrap())
        } else {
            None
        }
    };

    fn get_vector(val: Value) -> Option<Vec<Value>> {
        if let Value::Vector(vec) = val {
            Some(vec)
        } else {
            None
        }
    }

    match &name as &str {
        "slowcat" => {
            Some(Value::Pattern(slowcat(get_vector(arg1()?)?.into_iter().map(to_pattern)).boxed()))
        },
        _ => { todo!() }
    }
}

fn wrap_f1(name: String) -> Value {
    let a = String::from("a");
    Value::Lambda(a.clone(),
        Expr::External(name, vec![Expr::Var(a)])
    )
}


fn wrap_f2(name: String) -> Value {
    let a = String::from("a");
    let b = String::from("b");
    Value::Lambda(a.clone(),
        Expr::Lambda(b.clone(),
            Box::new(Expr::External(name, vec![Expr::Var(a), Expr::Var(b)]))
        )
    )
}

#[derive(Debug, Clone)]
pub struct Environment {
    variable_map: HashMap<String, Value>
}

impl Environment {
    fn new() -> Self {
        Self { variable_map: HashMap::new() }
    }
}

pub fn eval_pattern(expr: Expr) -> Option<BoxPattern<Value>> {
    let value = eval_with(&mut Environment::new(), expr)?;
    if let Value::Pattern(pat) = value {
        return Some(pat)
    }

    None
}

fn eval_with(env: &mut Environment, expr: Expr) -> Option<Value> {
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
            let old_value = env.variable_map.remove(&name);
            let arg_value = eval_with(env, *arg)?;
            env.variable_map.insert(name.clone(), arg_value);
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
                "slowcat" => { Some(wrap_f1(String::from("slowcat"))) },
                "cat" => { Some(wrap_f1(String::from("slowcat"))) },
                "fc" => { Some(wrap_f1(String::from("fastcat"))) },
                _ => { env.variable_map.get(&name).cloned() }
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
