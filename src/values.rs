use std::{fmt::Display, ops::{Add, Div, Mul, Neg, Sub}};

#[derive(Debug, Clone, Copy)]
pub enum Value {
    Float(f64),
    Int(i64),
}

impl Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Value::Float(x) => write!(f, "{}", x),
            Value::Int(x) => write!(f, "{}", x),
        }
    }
}

impl Add<Value> for Value {
    type Output = Result<Value, &'static str>;
    fn add(self, rhs: Value) -> Self::Output {
        match (self, rhs) {
            (Value::Int(x), Value::Int(y)) => Ok(Value::Int(x + y)),
            (Value::Float(x), Value::Float(y)) => Ok(Value::Float(x + y)),
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
            _ => Err("Cannot divide values of different types"),
        }
    }
}

impl Neg for Value {
    type Output = Value;
    fn neg(self) -> Self::Output {
        match self {
            Value::Int(x) => Value::Int(-x),
            Value::Float(x) => Value::Float(-x),
        }
    }
}
