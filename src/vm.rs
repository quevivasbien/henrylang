use std::collections::HashMap;
use std::ops::{Add, BitAnd, BitOr, Div, Mul, Neg, Not, Sub};

use byteorder::ReadBytesExt;

use crate::{Chunk, OpCode, Value};

#[derive(Debug)]
pub enum InterpreterError {
    CompileError(&'static str),
    RuntimeError(&'static str),
}

#[derive(Debug)]
pub struct VM {
    stack: Vec<Value>,
    varstack: Vec<Value>,
    globals: HashMap<String, Value>,
    last_value: Option<Value>,
}

impl VM {
    pub fn new() -> Self {
        Self { stack: Vec::new(), varstack: Vec::new(), globals: HashMap::new(), last_value: None }
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
                println!("{:?}", self);
                chunk._disassemble_instruction(&mut cursor_copy);
            }
            let opcode = match cursor.read_u8() {
                Ok(x) => OpCode::from(x),
                Err(_) => return Err(InterpreterError::CompileError("Reached end of chunk with no return")),
            };
            match opcode {
                OpCode::Return => {
                    return Ok(self.last_value.clone());
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
                OpCode::And => self.binary_op(Value::bitand)?,
                OpCode::Or => self.binary_op(Value::bitor)?,
                OpCode::Negate => self.unary_op(Value::neg)?,
                OpCode::Not => self.unary_op(Value::not)?,
                OpCode::EndExpr => {
                    self.last_value = self.stack.pop();
                },
                OpCode::EndBlock => {
                    let n_pops = Chunk::read_u16(&mut cursor);
                    self.varstack.truncate(self.varstack.len() - n_pops as usize);
                    self.stack.push(match &self.last_value {
                        Some(x) => x.clone(),
                        None => Value::Bool(false),
                    });
                },
                OpCode::Constant => {
                    let constant = chunk.read_constant(&mut cursor);
                    self.stack.push(constant.clone());
                },
                OpCode::SetGlobal => {
                    let name = chunk.read_constant(&mut cursor);
                    let name = match name.clone() {
                        Value::String(name) => name,
                        _ => unreachable!("Global name was not a string"),
                    };
                    let value = match self.stack.last() {
                        Some(x) => x.clone(),
                        None => return Err(InterpreterError::RuntimeError("Attempted to set null global")),
                    };
                    self.globals.insert(name, value);
                },
                OpCode::GetGlobal => {
                    let name = match &chunk.read_constant(&mut cursor) {
                        Value::String(name) => name,
                        _ => unreachable!("Global name was not a string"),
                    };
                    match self.globals.get(name) {
                        Some(x) => self.stack.push(x.clone()),
                        None => return Err(InterpreterError::RuntimeError("Variable is undefined")),
                    };
                }
                OpCode::SetLocal => {
                    let value = self.stack.last();
                    match value {
                        Some(x) => self.varstack.push(x.clone()),
                        None => return Err(InterpreterError::RuntimeError("Attempted to set null variable")),
                    }
                },
                OpCode::GetLocal => {
                    let idx = Chunk::read_u16(&mut cursor);
                    let value = self.varstack[idx as usize].clone();
                    self.stack.push(value);
                }
            }
        }
    }
}
