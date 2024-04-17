use std::{fmt::{Debug, Display}, rc::Rc};

use crate::chunk::Chunk;
use crate::vm::{VM, InterpreterError};

use super::{HeapValue, Value};

pub struct Function {
    pub name: String,
    pub num_upvalues: u16,
    pub num_heap_upvalues: u16,
    pub arity: u8,
    pub heap_arity: u8,
    pub return_is_heap: bool,
    pub chunk: Chunk,
}

impl Default for Function {
    fn default() -> Self {
        Self { name: String::new(), num_upvalues: 0, num_heap_upvalues: 0, arity: 0, heap_arity: 0, return_is_heap: false, chunk: Chunk::new() }
    }
}

impl Debug for Function {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}[{}](<{}+{}>){{{}}}", self.name, self.num_upvalues, self.arity, self.heap_arity, self.chunk.len())
    }
}

impl Display for Function {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = if self.name.is_empty() { &"<anon>" } else { self.name.as_str() };
        write!(f, "{}(<{}+{}>)", name, self.arity, self.heap_arity)
    }
}

#[derive(Clone)]
pub struct Closure {
    pub function: Rc<Function>,
    pub upvalues: Vec<Value>,
    pub heap_upvalues: Vec<HeapValue>,
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
        let heap_upvalues = Vec::with_capacity(function.num_heap_upvalues as usize);
        Self { function, upvalues, heap_upvalues }
    }
}

#[derive(Clone)]
pub struct NativeFunction {
    pub name: &'static str,
    pub arity: u8,
    pub heap_arity: u8,
    pub return_is_heap: bool,
    pub function: fn(&mut VM, &[Value], &[HeapValue]) -> Result<(), InterpreterError>,
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