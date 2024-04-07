use std::{fmt::{Debug, Display}, ops::{Add, BitAnd, BitOr, Div, Mul, Neg, Not, Sub}};
use std::rc::Rc;
// use downcast_rs::{impl_downcast, DowncastSync};

use crate::{vm::{InterpreterError, VM}, Chunk};

pub struct Function {
    pub num_upvalues: u16,
    pub arity: u8,
    pub chunk: Chunk,
}

impl Default for Function {
    fn default() -> Self {
        Self { num_upvalues: 0, arity: 0, chunk: Chunk::new() }
    }
}

impl Debug for Function {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}]({}){{{}}}", self.num_upvalues, self.arity, self.chunk.len())
    }
}

#[derive(Clone)]
pub struct Closure {
    pub function: Rc<Function>,
    pub upvalues: Vec<Value>,
}

impl Debug for Closure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "up{:?}fn{:?}", self.upvalues, self.function)
    }
}

impl Closure {
    pub fn new(function: Rc<Function>) -> Self {
        let upvalues = Vec::with_capacity(function.num_upvalues as usize);
        Self { function, upvalues }
    }
}

#[derive(Clone)]
pub struct NativeFunction {
    pub name: &'static str,
    pub arity: u8,
    pub function: fn(&mut VM, &[Value]) -> Result<Value, InterpreterError>,
}

impl Debug for NativeFunction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}[{}]()", self.name, self.arity)
    }
}

#[derive(Clone, Debug)]
pub enum Value {
    Float(f64),
    Int(i64),
    Bool(bool),
    String(Rc<String>),
    Array(Rc<Vec<Value>>),
    Closure(Box<Closure>),
    NativeFunction(&'static NativeFunction),
    // Object(Rc<dyn Object>),
}

impl Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Value::Float(x) => write!(f, "{}", x),
            Value::Int(x) => write!(f, "{}", x),
            Value::Bool(x) => write!(f, "{}", x),
            Value::String(x) => write!(f, "{}", x),
            Value::Closure(x) => write!(f, "{:?}", x),
            Value::NativeFunction(x) => write!(f, "{:?}", x),
            Value::Array(x) => {    
                write!(f, "[")?;
                if x.is_empty() {
                    return write!(f, "]");
                }
                for v in x.iter().take(x.len() - 1) {
                    write!(f, "{}", v)?;
                    write!(f, ", ")?;
                }
                write!(f, "{}]", x.last().unwrap())
            }
            // Value::Object(x) => write!(f, "{}", x.string()),
        }
    }
}

impl Add<Value> for Value {
    type Output = Result<Value, &'static str>;
    fn add(self, rhs: Value) -> Self::Output {
        match (self, rhs) {
            (Value::Int(x), Value::Int(y)) => Ok(Value::Int(x + y)),
            (Value::Float(x), Value::Float(y)) => Ok(Value::Float(x + y)),
            (Value::Bool(_x), Value::Bool(_y)) => Err("Cannot add booleans"),
            (Value::String(x), Value::String(y)) => Ok(Value::String(Rc::new(format!("{}{}", x, y)))),
            (Value::Array(x), Value::Array(y)) => Ok(Value::Array({
                let mut v = x.as_ref().clone();
                v.append(&mut y.as_ref().clone());
                Rc::new(v)
            })),
            (Value::Closure(_x), Value::Closure(_y)) => Err("Cannot add functions"),
            (Value::NativeFunction(_x), Value::NativeFunction(_y)) => Err("Cannot add functions"),
            _ => Err("Cannot add values of different types"),
        }
    }
}

impl Sub<Value> for Value {
    type Output = Result<Value, &'static str>;
    fn sub(self, rhs: Value) -> Self::Output {
        match (self, rhs) {
            (Value::Int(x), Value::Int(y)) => Ok(Value::Int(x - y)),
            (Value::Float(x), Value::Float(y)) => Ok(Value::Float(x - y)),
            (Value::Bool(_x), Value::Bool(_y)) => Err("Cannot subtract booleans"),
            (Value::String(_x), Value::String(_y)) => Err("Cannot subtract strings"),
            (Value::Array(_x), Value::Array(_y)) => Err("Cannot subtract arrays"),
            (Value::Closure(_x), Value::Closure(_y)) => Err("Cannot subtract functions"),
            (Value::NativeFunction(_x), Value::NativeFunction(_y)) => Err("Cannot subtract functions"),
            _ => Err("Cannot subtract values of different types"),
        }
    }
}

impl Mul<Value> for Value {
    type Output = Result<Value, &'static str>;
    fn mul(self, rhs: Value) -> Self::Output {
        match (self, rhs) {
            (Value::Int(x), Value::Int(y)) => Ok(Value::Int(x * y)),
            (Value::Float(x), Value::Float(y)) => Ok(Value::Float(x * y)),
            (Value::Bool(_x), Value::Bool(_y)) => Err("Cannot multiply booleans"),
            (Value::String(_x), Value::String(_y)) => Err("Cannot multiply strings"),
            (Value::Array(_x), Value::Array(_y)) => Err("Cannot multiply arrays"),
            (Value::Closure(_x), Value::Closure(_y)) => Err("Cannot multiply functions"),
            (Value::NativeFunction(_x), Value::NativeFunction(_y)) => Err("Cannot multiply functions"),
            _ => Err("Cannot multiply values of different types"),
        }
    }
}

impl Div<Value> for Value {
    type Output = Result<Value, &'static str>;
    fn div(self, rhs: Value) -> Self::Output {
        match (self, rhs) {
            (Value::Int(x), Value::Int(y)) => Ok(Value::Int(x / y)),
            (Value::Float(x), Value::Float(y)) => Ok(Value::Float(x / y)),
            (Value::Bool(_x), Value::Bool(_y)) => Err("Cannot divide booleans"),
            (Value::String(_x), Value::String(_y)) => Err("Cannot divide strings"),
            (Value::Array(_x), Value::Array(_y)) => Err("Cannot divide arrays"),
            (Value::Closure(_x), Value::Closure(_y)) => Err("Cannot divide functions"),
            (Value::NativeFunction(_x), Value::NativeFunction(_y)) => Err("Cannot divide functions"),
            _ => Err("Cannot divide values of different types"),
        }
    }
}

impl BitAnd<Value> for Value {
    type Output = Result<Value, &'static str>;
    fn bitand(self, rhs: Value) -> Self::Output {
        match (self, rhs) {
            (Value::Bool(x), Value::Bool(y)) => Ok(Value::Bool(x && y)),
            _ => Err("Cannot use `and` operator on non-boolean values"),
        }
    }
}

impl BitOr<Value> for Value {
    type Output = Result<Value, &'static str>;
    fn bitor(self, rhs: Value) -> Self::Output {
        match (self, rhs) {
            (Value::Bool(x), Value::Bool(y)) => Ok(Value::Bool(x || y)),
            _ => Err("Cannot use `or` operator on non-boolean values"),
        }
    }
}

impl Neg for Value {
    type Output = Result<Value, &'static str>;
    fn neg(self) -> Self::Output {
        match self {
            Value::Int(x) => Ok(Value::Int(-x)),
            Value::Float(x) => Ok(Value::Float(-x)),
            _ => Err("Cannot negate non-numeric type"),
        }
    }
}

impl Not for Value {
    type Output = Result<Value, &'static str>;
    fn not(self) -> Self::Output {
        match self {
            Value::Bool(x) => Ok(Value::Bool(!x)),
            _ => Err("Cannot negate non-boolean type"),
        }
    }
}

impl Value {
    pub fn eq(self, other: Self) -> Result<Self, &'static str> {
        match (self, other) {
            (Value::Int(x), Value::Int(y)) => Ok(Value::Bool(x == y)),
            (Value::Float(x), Value::Float(y)) => Ok(Value::Bool(x == y)),
            (Value::Bool(x), Value::Bool(y)) => Ok(Value::Bool(x == y)),
            (Value::String(x), Value::String(y)) => Ok(Value::Bool(x == y)),
            (Value::Array(x), Value::Array(y)) => {
                if x.len() != y.len() {
                    return Ok(Value::Bool(false));
                }
                for (x, y) in x.iter().zip(y.iter()) {
                    match x.clone().eq(y.clone()) {
                        Ok(Value::Bool(b)) => if !b {
                            return Ok(Value::Bool(false));
                        },
                        Err(e) => return Err(e),
                        Ok(_) => unreachable!(),
                    }
                }
                Ok(Value::Bool(true))
            },
            _ => Err("Cannot compare values of different types"),
        }
    }

    pub fn ne(self, other: Self) -> Result<Self, &'static str> {
        match (self, other) {
            (Value::Int(x), Value::Int(y)) => Ok(Value::Bool(x != y)),
            (Value::Float(x), Value::Float(y)) => Ok(Value::Bool(x != y)),
            (Value::Bool(x), Value::Bool(y)) => Ok(Value::Bool(x != y)),
            (Value::String(x), Value::String(y)) => Ok(Value::Bool(x != y)),
            (Value::Array(x), Value::Array(y)) => {
                if x.len() != y.len() {
                    return Ok(Value::Bool(false));
                }
                for (x, y) in x.iter().zip(y.iter()) {
                    match x.clone().ne(y.clone()) {
                        Ok(Value::Bool(b)) => if !b {
                            return Ok(Value::Bool(false));
                        },
                        Err(e) => return Err(e),
                        Ok(_) => unreachable!(),
                    }
                }
                Ok(Value::Bool(true))
            },
            (x, y) => x.eq(y).map(|b| match b {
                Value::Bool(b) => Value::Bool(!b),
                _ => unreachable!()
            }),
        }
    }

    pub fn lt(self, other: Self) -> Result<Self, &'static str> {
        match (self, other) {
            (Value::Int(x), Value::Int(y)) => Ok(Value::Bool(x < y)),
            (Value::Float(x), Value::Float(y)) => Ok(Value::Bool(x < y)),
            (Value::Bool(_x), Value::Bool(_y)) => Err("Order of booleans is not defined"),
            (Value::String(x), Value::String(y)) => Ok(Value::Bool(x < y)),
            (Value::Array(_x), Value::Array(_y)) => Err("Order of arrays is not defined"),
            _ => Err("Cannot compare values of different types"),
        }
    }

    pub fn le(self, other: Self) -> Result<Self, &'static str> {
        match (self, other) {
            (Value::Int(x), Value::Int(y)) => Ok(Value::Bool(x <= y)),
            (Value::Float(x), Value::Float(y)) => Ok(Value::Bool(x <= y)),
            (Value::Bool(_x), Value::Bool(_y)) => Err("Order of booleans is not defined"),
            (Value::String(x), Value::String(y)) => Ok(Value::Bool(x <= y)),
            (Value::Array(_x), Value::Array(_y)) => Err("Order of arrays is not defined"),
            _ => Err("Cannot compare values of different types"),
        }
    }

    pub fn gt(self, other: Self) -> Result<Self, &'static str> {
        match (self, other) {
            (Value::Int(x), Value::Int(y)) => Ok(Value::Bool(x > y)),
            (Value::Float(x), Value::Float(y)) => Ok(Value::Bool(x > y)),
            (Value::Bool(_x), Value::Bool(_y)) => Err("Order of booleans is not defined"),
            (Value::String(x), Value::String(y)) => Ok(Value::Bool(x > y)),
            (Value::Array(_x), Value::Array(_y)) => Err("Order of arrays is not defined"),
            _ => Err("Cannot compare values of different types"),
        }
    }

    pub fn ge(self, other: Self) -> Result<Self, &'static str> {
        match (self, other) {
            (Value::Int(x), Value::Int(y)) => Ok(Value::Bool(x >= y)),
            (Value::Float(x), Value::Float(y)) => Ok(Value::Bool(x >= y)),
            (Value::Bool(_x), Value::Bool(_y)) => Err("Order of booleans is not defined"),
            (Value::String(x), Value::String(y)) => Ok(Value::Bool(x >= y)),
            (Value::Array(_x), Value::Array(_y)) => Err("Order of arrays is not defined"),
            _ => Err("Cannot compare values of different types"),
        }
    }

    pub fn range(self, rhs: Self) -> Result<Self, &'static str> {
        match (self, rhs) {
            (Value::Int(x), Value::Int(y)) => {
                if x > y {
                    Ok(Value::Array(
                        Rc::new((y..=x).rev().map(Value::Int).collect()))
                    )
                }
                else {
                    Ok(Value::Array(
                        Rc::new((x..=y).map(Value::Int).collect()))
                    )
                }
            },
            _ => Err("Can only create ranges from Ints"),
        }
    }
}
