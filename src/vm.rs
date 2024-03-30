use std::ops::{Add, Sub, Mul, Div};

use byteorder::ReadBytesExt;

use crate::{Chunk, OpCode, Value};

#[derive(Debug)]
pub enum InterpreterError {
    CompileError(String),
    RuntimeError(String),
}

pub struct VM {
    stack: Vec<Value>,
}

impl VM {
    pub fn new() -> Self {
        Self { stack: Vec::new() }
    }

    fn binary_op(&mut self, op: fn(Value, Value) -> Result<Value, &'static str>) -> Result<(), InterpreterError> {
        match (self.stack.pop(), self.stack.pop()) {
            (Some(x), Some(y)) => match op(y, x) {
                Ok(x) => Ok(self.stack.push(x)),
                Err(e) => return Err(InterpreterError::RuntimeError(e.to_string())),
            },
            _ => Err(InterpreterError::RuntimeError("Line {}: Attempted to perform binary operation on null".to_string())),
        }
    }

    pub fn run(&mut self, chunk: &Chunk) -> Result<(), InterpreterError> {
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
                Err(_) => return Err(InterpreterError::CompileError("Reached end of chunk with no return".to_string())),
            };
            match opcode {
                OpCode::Return => {
                    match self.stack.pop() {
                        Some(x) => println!("{}", x),
                        None => println!("{{}}"),
                    }
                    return Ok(());
                },
                OpCode::Constant => {
                   let constant = chunk.read_constant(&mut cursor);
                   self.stack.push(constant);
                },  
                OpCode::Add => {
                    self.binary_op(Value::add)?;
                },
                OpCode::Subtract => {
                    self.binary_op(Value::sub)?;
                },
                OpCode::Multiply => {
                   self.binary_op(Value::mul)?; 
                },
                OpCode::Divide => {
                    self.binary_op(Value::div)?;
                },
                OpCode::Negate => {
                   match self.stack.pop() {
                       Some(x) => self.stack.push(-x),
                       None => return Err(InterpreterError::RuntimeError("Attempted to negate null".to_string())),
                   }
                },
            }
        }
    }
}
