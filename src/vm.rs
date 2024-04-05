use std::collections::HashMap;
use std::ops::{Add, BitAnd, BitOr, Div, Mul, Neg, Not, Sub};
use std::rc::Rc;

use crate::{Function, OpCode, Value, compile};

#[derive(Debug)]
pub enum InterpreterError {
    CompileError,
    RuntimeError(&'static str),
}

#[derive(Debug)]
struct CallFrame {
    function: Rc<Function>,
    ip: usize,
    stack_idx: usize,
}

impl CallFrame {
    fn new(function: Rc<Function>, stack_idx: usize) -> Self {
        Self { function, ip: 0, stack_idx }
    }
}

#[derive(Debug)]
pub struct VM {
    stack: Vec<Value>,
    globals: HashMap<String, Value>,
    last_value: Option<Value>,
}

impl VM {
    pub fn new() -> Self {
        Self {
            stack: Vec::new(),
            globals: HashMap::new(),
            last_value: None
        }
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

    fn call(&mut self, mut frame: CallFrame) -> Result<Option<Value>, InterpreterError> {
        let chunk = &frame.function.chunk;
        #[cfg(feature = "debug")]
        {
            println!("==call {:?}==", frame.function);
            chunk.disassemble();
            println!("");
        }
        loop {
            if frame.ip >= chunk.len() {
                return Err(InterpreterError::RuntimeError("Reached end of chunk with no return"));
            }
            #[cfg(feature = "debug")]
            {
                print!("stack: {:?}, ", &self.stack[frame.stack_idx..]);
                print!("globals: {:?}, ", self.globals);
                println!("last_value: {:?}", self.last_value);
                let mut ip_copy = frame.ip;
                chunk.disassemble_instruction(&mut ip_copy);
            }
            let opcode = OpCode::from(chunk.read_u8(&mut frame.ip));
            match opcode {
                OpCode::Return => {
                    // pop function
                    self.stack.pop();
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
                    let n_pops = chunk.read_u16(&mut frame.ip);
                    self.stack.truncate(self.stack.len() - n_pops as usize);
                    self.stack.push(match &self.last_value {
                        Some(x) => x.clone(),
                        None => Value::Bool(false),
                    });
                },
                OpCode::Jump => {
                    let offset = chunk.read_u16(&mut frame.ip);
                    frame.ip += offset as usize;
                },
                OpCode::JumpIfFalse => {
                    let offset = chunk.read_u16(&mut frame.ip);
                    let condition = match self.stack.pop() {
                        Some(x) => x,
                        None => return Err(InterpreterError::RuntimeError("Attempted to jump with null condition")),
                    };
                    match condition {
                        Value::Bool(false) => {
                            frame.ip += offset as usize;
                        },
                        Value::Bool(true) => (),
                        _ => return Err(InterpreterError::RuntimeError("Expected boolean in condition")),
                    }
                },
                OpCode::Call => {
                    let n_args = chunk.read_u8(&mut frame.ip);
                    let caller = match &self.stack[self.stack.len()-n_args as usize-1] {
                        Value::Function(f) => f.clone(),
                        _ => return Err(InterpreterError::RuntimeError("Attempted to call non-function")),
                    };
                    
                    let new_frame = CallFrame::new(caller, self.stack.len() - n_args as usize - 1);
                    let result = match self.call(new_frame)? {
                        Some(x) => x,
                        None => Value::Bool(false),
                    };
                    // clear stack used by function and function args
                    self.stack.truncate(self.stack.len() - n_args as usize - 1);
                    self.stack.push(result);
                },
                OpCode::Constant => {
                    let constant = chunk.read_constant(&mut frame.ip);
                    self.stack.push(constant.clone());
                },
                OpCode::SetGlobal => {
                    let name = chunk.read_constant(&mut frame.ip);
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
                    let name = match &chunk.read_constant(&mut frame.ip) {
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
                        Some(x) => self.stack.push(x.clone()),
                        None => return Err(InterpreterError::RuntimeError("Attempted to set null variable")),
                    }
                },
                OpCode::GetLocal => {
                    let idx = chunk.read_u16(&mut frame.ip);
                    let value = self.stack[frame.stack_idx + idx as usize].clone();
                    self.stack.push(value);
                }
            }
        }
    }

    pub fn interpret(&mut self, source: String) -> Result<Option<Value>, InterpreterError> {
        let function = Rc::new(compile(source).map_err(|_| InterpreterError::CompileError)?);
        self.stack.push(Value::Function(function.clone()));
        let frame = CallFrame::new(function, 0);

        self.call(frame)
    }
}
