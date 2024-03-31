use std::{collections::HashMap, ops::{Add, Div, Mul, Neg, Not, Sub}};

use byteorder::ReadBytesExt;

use crate::{Chunk, OpCode, Value, ObjectString};

#[derive(Debug)]
pub enum InterpreterError {
    CompileError(&'static str),
    RuntimeError(&'static str),
}

pub struct VM {
    stack: Vec<Value>,
    globals: HashMap<String, Value>,
}

impl VM {
    pub fn new() -> Self {
        Self { stack: Vec::new(), globals: HashMap::new() }
    }

    fn binary_op(&mut self, op: fn(Value, Value) -> Result<Value, &'static str>) -> Result<(), InterpreterError> {
        match (self.stack.pop(), self.stack.pop()) {
            (Some(x), Some(y)) => match op(y, x) {
                Ok(x) => Ok(self.stack.push(x)),
                Err(e) => return Err(InterpreterError::RuntimeError(e)),
            },
            _ => Err(InterpreterError::RuntimeError("Attempted to perform binary operation on null")),
        }
    }

    fn unary_op(&mut self, op: fn(Value) -> Result<Value, &'static str>) -> Result<(), InterpreterError> {
        match self.stack.pop() {
            Some(x) => match op(x) {
                Ok(x) => Ok(self.stack.push(x)),
                Err(e) => return Err(InterpreterError::RuntimeError(e)),
            },
            None => Err(InterpreterError::RuntimeError("Attempted to perform unary operation on null")),
        }
    }

    pub fn run(&mut self, chunk: &Chunk) -> Result<Option<Value>, InterpreterError> {
        let mut cursor = chunk.cursor();
        #[cfg(feature = "debug")]
        let mut cursor_copy = chunk.cursor();
        loop {
            #[cfg(feature = "debug")]
            {
                println!("{:?}", self.stack);
                chunk.disassemble_instruction(&mut cursor_copy);
            }
            let opcode = match cursor.read_u8() {
                Ok(x) => OpCode::from(x),
                Err(_) => return Err(InterpreterError::CompileError("Reached end of chunk with no return")),
            };
            match opcode {
                OpCode::Return => {
                    return Ok(self.stack.pop());
                },
                OpCode::True => self.stack.push(Value::Bool(true)),
                OpCode::False => self.stack.push(Value::Bool(false)),
                OpCode::Equal => self.binary_op(Value::eq)?,
                OpCode::NotEqual => self.binary_op(Value::ne)?,
                OpCode::Greater => self.binary_op(Value::gt)?,
                OpCode::GreaterEqual => self.binary_op(Value::ge)?,
                OpCode::Less => self.binary_op(Value::lt)?,
                OpCode::LessEqual => self.binary_op(Value::le)?,
                OpCode::Add => self.binary_op(Value::add)?,
                OpCode::Subtract => self.binary_op(Value::sub)?,
                OpCode::Multiply => self.binary_op(Value::mul)?,
                OpCode::Divide => self.binary_op(Value::div)?,
                OpCode::Negate => self.unary_op(Value::neg)?,
                OpCode::Not => self.unary_op(Value::not)?,
                OpCode::Constant => {
                    let constant = chunk.read_constant(&mut cursor);
                    self.stack.push(constant.clone());
                },
                OpCode::DefineGlobal => {
                    let name = match &chunk.read_constant(&mut cursor) {
                        Value::Object(name) => {
                            &name.downcast_ref::<ObjectString>().unwrap().value
                        },
                        _ => unreachable!(),
                    };
                    let value = match self.stack.last() {
                        Some(x) => x,
                        None => return Err(InterpreterError::RuntimeError("Attempted to define global variable as null")),
                    };
                    self.globals.insert(name.clone(), value.clone());
                },
                OpCode::GetGlobal => {
                    let name = match &chunk.read_constant(&mut cursor) {
                        Value::Object(name) => {
                            &name.downcast_ref::<ObjectString>().unwrap().value
                        },
                        _ => unreachable!(),
                    };
                    match self.globals.get(name) {
                        Some(x) => self.stack.push(x.clone()),
                        None => return Err(InterpreterError::RuntimeError("Attempted to access undefined global variable")),
                    }
                }
            }
        }
    }
}
