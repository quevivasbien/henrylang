use std::{fmt::{Debug, Display}, ops::{Add, Div, Mul, Neg, Not, Sub}, rc::Rc};
use downcast_rs::{impl_downcast, DowncastSync};

#[derive(Debug, PartialEq)]
pub enum ObjectType {
    String
}

pub trait Object: DowncastSync {
    fn get_type(&self) -> ObjectType;
    fn string(&self) -> String {
        format!("{:?}", self.get_type())
    }
}
impl_downcast!(sync Object);

impl Debug for dyn Object {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.string())
    }
}

pub struct ObjectString {
    pub value: String
}

impl Object for ObjectString {
    fn get_type(&self) -> ObjectType {
        ObjectType::String
    }
    fn string(&self) -> String {
        self.value.clone()
    }
}

impl ObjectString {
    pub fn new(value: String) -> Self {
        Self { value }
    }
}

fn add_objects(x: &dyn Object, y: &dyn Object) -> Result<Value, &'static str> {
    match (x.get_type(), y.get_type()) {
        (ObjectType::String, ObjectType::String) => Ok(Value::Object(
            Rc::new(ObjectString { value: format!("{}{}", x.string(), y.string()) })
        )),
        _ => Err("Add not implemented for this object type"),
    }
}

#[derive(Clone, Debug)]
pub enum Value {
    Float(f64),
    Int(i64),
    Bool(bool),
    Object(Rc<dyn Object>),
}

impl Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Value::Float(x) => write!(f, "{}", x),
            Value::Int(x) => write!(f, "{}", x),
            Value::Bool(x) => write!(f, "{}", x),
            Value::Object(x) => write!(f, "{}", x.string()),
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
            (Value::Object(x), Value::Object(y)) => add_objects(x.as_ref(), y.as_ref()),
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
            _ => Err("Cannot divide values of different types"),
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
            _ => Err("Cannot compare values of different types"),
        }
    }

    pub fn ne(self, other: Self) -> Result<Self, &'static str> {
        match (self, other) {
            (Value::Int(x), Value::Int(y)) => Ok(Value::Bool(x != y)),
            (Value::Float(x), Value::Float(y)) => Ok(Value::Bool(x != y)),
            (Value::Bool(x), Value::Bool(y)) => Ok(Value::Bool(x != y)),
            _ => Err("Cannot compare values of different types"),
        }
    }

    pub fn lt(self, other: Self) -> Result<Self, &'static str> {
        match (self, other) {
            (Value::Int(x), Value::Int(y)) => Ok(Value::Bool(x < y)),
            (Value::Float(x), Value::Float(y)) => Ok(Value::Bool(x < y)),
            (Value::Bool(_x), Value::Bool(_y)) => Err("Order of booleans is not defined"),
            _ => Err("Cannot compare values of different types"),
        }
    }

    pub fn le(self, other: Self) -> Result<Self, &'static str> {
        match (self, other) {
            (Value::Int(x), Value::Int(y)) => Ok(Value::Bool(x <= y)),
            (Value::Float(x), Value::Float(y)) => Ok(Value::Bool(x <= y)),
            (Value::Bool(_x), Value::Bool(_y)) => Err("Order of booleans is not defined"),
            _ => Err("Cannot compare values of different types"),
        }
    }

    pub fn gt(self, other: Self) -> Result<Self, &'static str> {
        match (self, other) {
            (Value::Int(x), Value::Int(y)) => Ok(Value::Bool(x > y)),
            (Value::Float(x), Value::Float(y)) => Ok(Value::Bool(x > y)),
            (Value::Bool(_x), Value::Bool(_y)) => Err("Order of booleans is not defined"),
            _ => Err("Cannot compare values of different types"),
        }
    }

    pub fn ge(self, other: Self) -> Result<Self, &'static str> {
        match (self, other) {
            (Value::Int(x), Value::Int(y)) => Ok(Value::Bool(x >= y)),
            (Value::Float(x), Value::Float(y)) => Ok(Value::Bool(x >= y)),
            (Value::Bool(_x), Value::Bool(_y)) => Err("Order of booleans is not defined"),
            _ => Err("Cannot compare values of different types"),
        }
    }
}