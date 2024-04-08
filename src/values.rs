use std::{collections::HashMap, fmt::{Debug, Display}, ops::{Add, BitAnd, BitOr, Div, Mul, Neg, Not, Sub}};
use std::rc::Rc;
// use downcast_rs::{impl_downcast, DowncastSync};

use crate::{vm::{InterpreterError, VM}, Chunk};

pub struct Function {
    pub name: String,
    pub num_upvalues: u16,
    pub arity: u8,
    pub chunk: Chunk,
}

impl Default for Function {
    fn default() -> Self {
        Self { name: String::new(), num_upvalues: 0, arity: 0, chunk: Chunk::new() }
    }
}

impl Debug for Function {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}[{}](<{}>){{{}}}", self.name, self.num_upvalues, self.arity, self.chunk.len())
    }
}

impl Display for Function {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = if self.name.is_empty() { &"<anon>" } else { self.name.as_str() };
        write!(f, "{}(<{}>)", name, self.arity)
    }
}

#[derive(Clone)]
pub struct Closure {
    pub function: Rc<Function>,
    pub upvalues: Vec<Value>,
}

impl Debug for Closure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}{:?}", self.upvalues, self.function)
    }
}

impl Display for Closure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.function)
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
        write!(f, "{}({})", self.name, self.arity)
    }
}

impl Display for NativeFunction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}({})", self.name, self.arity)
    }
}

pub struct TypeDef {
    pub name: String,
    pub fieldnames: Vec<String>,
}

impl Debug for TypeDef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.name.is_empty() {
            write!(f, "{{ {:?} }}", self.fieldnames)
        }
        else {
            write!(f, "{} {{ {:?} }}", self.name, self.fieldnames)
        }
    }
}

impl Display for TypeDef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.name.is_empty() {
            write!(f, "<anontype> {{")?;
        }
        else {
            write!(f, "{} {{", self.name)?;
        }
        for field in &self.fieldnames {
            write!(f, " {}", field)?;
        }
        write!(f, " }}")
    }
}

impl TypeDef {
    pub fn new(name: String, fieldnames: Vec<String>) -> Self {
        Self { name, fieldnames }
    }
}

pub struct Object {
    pub typedef: Rc<TypeDef>,
    pub fields: HashMap<String, Value>,
}

impl Debug for Object {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.typedef.name.is_empty() {
            write!(f, "{:?}", self.fields)
        }
        else {
            write!(f, "{} {:?}", self.typedef, self.fields)
        }
    }
}

impl Display for Object {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.typedef.name.is_empty() {
            write!(f, "{{")?;
        }
        else {
            write!(f, "{} {{", self.typedef.name)?;
        }
        for field in &self.typedef.fieldnames {
            write!(f, " {}: {}", field, self.fields.get(field).unwrap())?;
        }
        write!(f, " }}")
    }
}

impl Object {
    pub fn new(typedef: Rc<TypeDef>, fields: HashMap<String, Value>) -> Self {
        Self { typedef, fields }
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
    TypeDef(Rc<TypeDef>),
    Object(Rc<Object>),
    Maybe(Box<Option<Value>>),
}

impl Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Value::Float(x) => write!(f, "{:?}", x),
            Value::Int(x) => write!(f, "{}", x),
            Value::Bool(x) => write!(f, "{}", x),
            Value::String(x) => write!(f, "\"{}\"", x),
            Value::Closure(x) => write!(f, "{}", x),
            Value::NativeFunction(x) => write!(f, "{}", x),
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
            },
            Value::TypeDef(x) => write!(f, "{}", x),
            Value::Object(x) => write!(f, "{}", x),
            Value::Maybe(x) => {
                if x.is_none() {
                    write!(f, "null")
                }
                else {
                    write!(f, "some({})", x.clone().unwrap())
                }
            },
        }
    }
}

impl Add<Value> for Value {
    type Output = Result<Value, String>;
    fn add(self, rhs: Value) -> Self::Output {
        match (self, rhs) {
            (Value::Int(x), Value::Int(y)) => Ok(Value::Int(x + y)),
            (Value::Float(x), Value::Float(y)) => Ok(Value::Float(x + y)),
            (Value::Bool(_x), Value::Bool(_y)) => Err("Cannot add booleans".to_string()),
            (Value::String(x), Value::String(y)) => Ok(Value::String(Rc::new(format!("{}{}", x, y)))),
            (Value::Array(x), Value::Array(y)) => Ok(Value::Array({
                let mut v = x.as_ref().clone();
                v.append(&mut y.as_ref().clone());
                Rc::new(v)
            })),
            (Value::Closure(_x), Value::Closure(_y)) => Err("Cannot add functions".to_string()),
            (Value::NativeFunction(_x), Value::NativeFunction(_y)) => Err("Cannot add functions".to_string()),
            (Value::TypeDef(_x), Value::TypeDef(_y)) => Err("Cannot add type definitions".to_string()),
            (Value::Object(_x), Value::Object(_y)) => Err("Cannot add objects".to_string()),
            (Value::Maybe(_x), Value::Maybe(_y)) => Err("Cannot add maybe values without first unwrapping".to_string()),
            (x, y) => Err(
                format!("Tried to add {} and {}, but addition of different types is not allowed", x, y)
            ),
        }
    }
}

impl Sub<Value> for Value {
    type Output = Result<Value, String>;
    fn sub(self, rhs: Value) -> Self::Output {
        match (self, rhs) {
            (Value::Int(x), Value::Int(y)) => Ok(Value::Int(x - y)),
            (Value::Float(x), Value::Float(y)) => Ok(Value::Float(x - y)),
            (Value::Bool(_x), Value::Bool(_y)) => Err("Cannot subtract booleans".to_string()),
            (Value::String(_x), Value::String(_y)) => Err("Cannot subtract strings".to_string()),
            (Value::Array(_x), Value::Array(_y)) => Err("Cannot subtract arrays".to_string()),
            (Value::Closure(_x), Value::Closure(_y)) => Err("Cannot subtract functions".to_string()),
            (Value::NativeFunction(_x), Value::NativeFunction(_y)) => Err("Cannot subtract functions".to_string()),
            (Value::TypeDef(_x), Value::TypeDef(_y)) => Err("Cannot subtract type definitions".to_string()),
            (Value::Object(_x), Value::Object(_y)) => Err("Cannot subtract objects".to_string()),
            (Value::Maybe(_x), Value::Maybe(_y)) => Err("Cannot subtract maybe values without first unwrapping".to_string()),
            (x, y) => Err(
                format!("Tried to subtract {} and {}, but subtraction of different types is not allowed", x, y)
            ),
        }
    }
}

impl Mul<Value> for Value {
    type Output = Result<Value, String>;
    fn mul(self, rhs: Value) -> Self::Output {
        match (self, rhs) {
            (Value::Int(x), Value::Int(y)) => Ok(Value::Int(x * y)),
            (Value::Float(x), Value::Float(y)) => Ok(Value::Float(x * y)),
            (Value::Bool(_x), Value::Bool(_y)) => Err("Cannot multiply booleans".to_string()),
            (Value::String(_x), Value::String(_y)) => Err("Cannot multiply strings".to_string()),
            (Value::Array(_x), Value::Array(_y)) => Err("Cannot multiply arrays".to_string()),
            (Value::Closure(_x), Value::Closure(_y)) => Err("Cannot multiply functions".to_string()),
            (Value::NativeFunction(_x), Value::NativeFunction(_y)) => Err("Cannot multiply functions".to_string()),
            (Value::TypeDef(_x), Value::TypeDef(_y)) => Err("Cannot multiply type definitions".to_string()),
            (Value::Object(_x), Value::Object(_y)) => Err("Cannot multiply objects".to_string()),
            (Value::Maybe(_x), Value::Maybe(_y)) => Err("Cannot multiply maybe values without first unwrapping".to_string()),
            (x, y) => Err(
                format!("Tried to multiply {} and {}, but multiplication of different types is not allowed", x, y)
            ),
        }
    }
}

impl Div<Value> for Value {
    type Output = Result<Value, String>;
    fn div(self, rhs: Value) -> Self::Output {
        match (self, rhs) {
            (Value::Int(x), Value::Int(y)) => Ok(Value::Int(x / y)),
            (Value::Float(x), Value::Float(y)) => Ok(Value::Float(x / y)),
            (Value::Bool(_x), Value::Bool(_y)) => Err("Cannot divide booleans".to_string()),
            (Value::String(_x), Value::String(_y)) => Err("Cannot divide strings".to_string()),
            (Value::Array(_x), Value::Array(_y)) => Err("Cannot divide arrays".to_string()),
            (Value::Closure(_x), Value::Closure(_y)) => Err("Cannot divide functions".to_string()),
            (Value::NativeFunction(_x), Value::NativeFunction(_y)) => Err("Cannot divide functions".to_string()),
            (Value::TypeDef(_x), Value::TypeDef(_y)) => Err("Cannot divide type definitions".to_string()),
            (Value::Object(_x), Value::Object(_y)) => Err("Cannot divide objects".to_string()),
            (Value::Maybe(_x), Value::Maybe(_y)) => Err("Cannot divide maybe values without first unwrapping".to_string()),
            (x, y) => Err(
                format!("Tried to divide {} and {}, but division of different types is not allowed", x, y)
            ),
        }
    }
}

impl BitAnd<Value> for Value {
    type Output = Result<Value, String>;
    fn bitand(self, rhs: Value) -> Self::Output {
        match (self, rhs) {
            (Value::Bool(x), Value::Bool(y)) => Ok(Value::Bool(x && y)),
            (x, y) => Err(
                format!("Cannot use `and` operator on non-boolean values: got {} and {}", x, y)
            ),
        }
    }
}

impl BitOr<Value> for Value {
    type Output = Result<Value, String>;
    fn bitor(self, rhs: Value) -> Self::Output {
        match (self, rhs) {
            (Value::Bool(x), Value::Bool(y)) => Ok(Value::Bool(x || y)),
            (x, y) => Err(
                format!("Cannot use `or` operator on non-boolean values: got {} or {}", x, y)
            ),
        }
    }
}

impl Neg for Value {
    type Output = Result<Value, String>;
    fn neg(self) -> Self::Output {
        match self {
            Value::Int(x) => Ok(Value::Int(-x)),
            Value::Float(x) => Ok(Value::Float(-x)),
            _ => Err(
                format!("Cannot use `-` prefix on non-numeric value: got -{}", self)
            ),
        }
    }
}

impl Not for Value {
    type Output = Result<Value, String>;
    fn not(self) -> Self::Output {
        match self {
            Value::Bool(x) => Ok(Value::Bool(!x)),
            _ => Err(
                format!("Cannot use `!` prefix on non-boolean value: got !{}", self)
            ),
        }
    }
}

impl Value {
    pub fn eq(self, other: Self) -> Result<Self, String> {
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
            (Value::Object(x), Value::Object(y)) => {
                if x.fields.len() != y.fields.len() {
                    return Ok(Value::Bool(false));
                }
                if x.typedef.name != y.typedef.name {
                    return Ok(Value::Bool(false));
                }
                for (x, y) in x.fields.iter().zip(y.fields.iter()) {
                    if x.0 != y.0 {
                        return Ok(Value::Bool(false));
                    }
                    match x.1.clone().eq(y.1.clone()) {
                        Ok(Value::Bool(b)) => if !b {
                            return Ok(Value::Bool(false));
                        },
                        Ok(_) => unreachable!(),
                        Err(e) => return Err(e),
                    }
                }
                Ok(Value::Bool(true))
            },
            (Value::Maybe(x), Value::Maybe(y)) => Ok(match (x.as_ref(), y.as_ref()) {
                (None, None) => Value::Bool(true),
                (None, _) => Value::Bool(false),
                (_, None) => Value::Bool(false),
                (Some(x), Some(y)) => x.clone().eq(y.clone())?
            }),
            (x, y) => Err(
                format!("Tried to check equality of {} and {}, but comparison of different types is not allowed", x, y)
            ),
        }
    }

    pub fn ne(self, other: Self) -> Result<Self, String> {
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

    pub fn lt(self, other: Self) -> Result<Self, String> {
        match (self, other) {
            (Value::Int(x), Value::Int(y)) => Ok(Value::Bool(x < y)),
            (Value::Float(x), Value::Float(y)) => Ok(Value::Bool(x < y)),
            (Value::Bool(_x), Value::Bool(_y)) => Err("Order of booleans is not defined".to_string()),
            (Value::String(x), Value::String(y)) => Ok(Value::Bool(x < y)),
            (Value::Array(_x), Value::Array(_y)) => Err("Order of arrays is not defined".to_string()),
            (Value::Object(_x), Value::Object(_y)) => Err("Order of objects is not defined".to_string()),
            (x, y) => Err(
                format!("Cannot compare values of different types: got {} < {}", x, y)
            ),
        }
    }

    pub fn le(self, other: Self) -> Result<Self, String> {
        match (self, other) {
            (Value::Int(x), Value::Int(y)) => Ok(Value::Bool(x <= y)),
            (Value::Float(x), Value::Float(y)) => Ok(Value::Bool(x <= y)),
            (Value::Bool(_x), Value::Bool(_y)) => Err("Order of booleans is not defined".to_string()),
            (Value::String(x), Value::String(y)) => Ok(Value::Bool(x <= y)),
            (Value::Array(_x), Value::Array(_y)) => Err("Order of arrays is not defined".to_string()),
            (Value::Object(_x), Value::Object(_y)) => Err("Order of objects is not defined".to_string()),
            (x, y) => Err(
                format!("Cannot compare values of different types: got {} <= {}", x, y)
            ),
        }
    }

    pub fn gt(self, other: Self) -> Result<Self, String> {
        match (self, other) {
            (Value::Int(x), Value::Int(y)) => Ok(Value::Bool(x > y)),
            (Value::Float(x), Value::Float(y)) => Ok(Value::Bool(x > y)),
            (Value::Bool(_x), Value::Bool(_y)) => Err("Order of booleans is not defined".to_string()),
            (Value::String(x), Value::String(y)) => Ok(Value::Bool(x > y)),
            (Value::Array(_x), Value::Array(_y)) => Err("Order of arrays is not defined".to_string()),
            (Value::Object(_x), Value::Object(_y)) => Err("Order of objects is not defined".to_string()),
            (x, y) => Err(
                format!("Cannot compare values of different types: got {} > {}", x, y)
            ),
        }
    }

    pub fn ge(self, other: Self) -> Result<Self, String> {
        match (self, other) {
            (Value::Int(x), Value::Int(y)) => Ok(Value::Bool(x >= y)),
            (Value::Float(x), Value::Float(y)) => Ok(Value::Bool(x >= y)),
            (Value::Bool(_x), Value::Bool(_y)) => Err("Order of booleans is not defined".to_string()),
            (Value::String(x), Value::String(y)) => Ok(Value::Bool(x >= y)),
            (Value::Array(_x), Value::Array(_y)) => Err("Order of arrays is not defined".to_string()),
            (Value::Object(_x), Value::Object(_y)) => Err("Order of objects is not defined".to_string()),
            (x, y) => Err(
                format!("Cannot compare values of different types: got {} >= {}", x, y)
            ),
        }
    }

    pub fn range(self, rhs: Self) -> Result<Self, String> {
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
            (x, y) => Err(
                format!("Cannot create ranges from non-integers: got {} to {}", x, y)
            ),
        }
    }
}
