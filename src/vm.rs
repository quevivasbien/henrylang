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

    fn binary_op(&mut self, op: fn(f64, f64) -> f64) -> Result<(), InterpreterError> {
        match (self.stack.pop(), self.stack.pop()) {
            (Some(x), Some(y)) => Ok(self.stack.push(op(x, y))),
            _ => Err(InterpreterError::RuntimeError("Line {}: Attempted to perform binary operation on null".to_string())),
        }
    }

    pub fn run(&mut self, chunk: &Chunk) -> Result<(), InterpreterError> {
        let mut cursor = chunk.cursor();
        #[cfg(feature = "debug-trace")]
        let mut cursor_copy = chunk.cursor();
        loop {
            #[cfg(feature = "debug-trace")]
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
                        None => println!("null"),
                    }
                    return Ok(());
                },
                OpCode::Constant => {
                   let constant = chunk.read_constant(&mut cursor);
                   self.stack.push(constant);
                },  
                OpCode::Add => {
                    self.binary_op(|x, y| x + y)?;
                },
                OpCode::Subtract => {
                    self.binary_op(|x, y| x - y)?;
                },
                OpCode::Multiply => {
                   self.binary_op(|x, y| x * y)?; 
                },
                OpCode::Divide => {
                    self.binary_op(|x, y| x / y)?;
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