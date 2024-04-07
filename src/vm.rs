use std::collections::HashMap;
use std::ops::{Add, BitAnd, BitOr, Div, Mul, Neg, Not, Sub};
use std::rc::Rc;

use crate::chunk::Chunk;
use crate::{Closure, Function, NativeFunction, OpCode, Value, builtins, compile};

#[derive(Debug)]
pub enum InterpreterError {
    CompileError,
    RuntimeError(&'static str),
}

pub fn runtime_err(e: &'static str) -> InterpreterError {
    InterpreterError::RuntimeError(e)
}

pub struct CallFrame {
    closure: Box<Closure>,
    ip: usize,
    stack_idx: usize,
}

impl CallFrame {
    fn new(closure: Box<Closure>, stack_idx: usize) -> Self {
        Self { closure, ip: 0, stack_idx }
    }
}

pub struct VM {
    pub stack: Vec<Value>,
    pub globals: HashMap<String, Value>,
    pub last_value: Option<Value>,
    pub frames: Vec<CallFrame>,
}

impl VM {
    pub fn new() -> Self {
        Self {
            stack: Vec::new(),
            globals: builtins(),
            last_value: None,
            frames: Vec::new(),
        }
    }

    fn init(&mut self, function: Rc<Function>) {
        let closure = Box::new(Closure::new(function));
        let frame = CallFrame::new(closure.clone(), 0);
        self.frames.push(frame);
        self.stack.push(Value::Closure(closure));
    }

    fn frame(&self) -> &CallFrame {
        self.frames.last().unwrap()
    }
    fn frame_mut(&mut self) -> &mut CallFrame {
        self.frames.last_mut().unwrap()
    }
    fn frame_ptr(&mut self) -> *mut CallFrame {
        self.frames.last_mut().unwrap()
    }

    fn read_u8(&mut self) -> u8 {
        let ip = unsafe { &mut (*self.frame_ptr()).ip };
        self.chunk().read_u8(ip)
    }
    fn read_u16(&mut self) -> u16 {
        let ip = unsafe { &mut (*self.frame_ptr()).ip };
        self.chunk().read_u16(ip)
    }
    fn read_constant(&mut self) -> &Value {
        let ip = unsafe { &mut (*self.frame_ptr()).ip };
        self.chunk().read_constant(ip)
    }

    pub fn chunk(&self) -> &Chunk {
        &self.frames.last().unwrap().closure.function.chunk
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

    pub fn call_function(&mut self, n_args: u8, closure: Box<Closure>) -> Result<(), InterpreterError> {
        if n_args != closure.function.arity {
            return Err(InterpreterError::RuntimeError("Incorrect number of arguments"));
        }
        let new_frame = CallFrame::new(closure, self.stack.len() - n_args as usize - 1);
        self.frames.push(new_frame);
        let result = match self.call()? {
            Some(x) => x,
            None => Value::Bool(false),
        };
        // clear stack used by function and function args
        self.stack.truncate(self.stack.len() - n_args as usize - 1);
        self.stack.push(result);
        Ok(())
    }

    pub fn call_native_function(&mut self, n_args: u8, function: &'static NativeFunction) -> Result<(), InterpreterError> {
        if n_args != function.arity {
            return Err(InterpreterError::RuntimeError("Incorrect number of arguments"));
        }
        let args = self.stack.split_off(self.stack.len() - n_args as usize);
        let result = (function.function)(self, &args)?;
        self.stack.pop();
        self.stack.push(result);
        Ok(())
    }

    pub fn array_index(&mut self, n_args: u8, arr: &Vec<Value>) -> Result<(), InterpreterError> {
        if n_args != 1 {
            return Err(InterpreterError::RuntimeError("Can only access arrays with a single index"));
        }
        let result = match self.stack.pop() {
            Some(Value::Int(mut idx)) => {
                if idx < 0 {
                    idx = arr.len() as i64 + idx;
                }
                if idx < 0 || idx >= arr.len() as i64 {
                    return Err(InterpreterError::RuntimeError("Array index out of bounds"));
                }
                arr[idx as usize].clone()
            },
            Some(_) => return Err(InterpreterError::RuntimeError("Array index must be an integer")),
            None => return Err(InterpreterError::RuntimeError("Missing array index")),
        };
        self.stack.pop();
        self.stack.push(result);
        Ok(())
    }

    fn call(&mut self) -> Result<Option<Value>, InterpreterError> {
        if self.frames.is_empty() {
            return Err(InterpreterError::RuntimeError("Attempted to call with no active call frame"));
        }
        #[cfg(feature = "debug")]
        {
            println!("==call {:?}==", self.frame().closure);
            self.chunk().disassemble();
            println!("");
        }
        loop {
            if self.frame().ip >= self.chunk().len() {
                return Err(InterpreterError::RuntimeError("Reached end of chunk with no return"));
            }
            #[cfg(feature = "debug")]
            {
                print!("stack: {:?}, ", &self.stack[self.frame().stack_idx..]);
                // print!("globals: {:?}, ", self.globals);
                println!("last_value: {:?}", self.last_value);
                let mut ip_copy = self.frame().ip;
                self.chunk().disassemble_instruction(&mut ip_copy);
            }
            let opcode = OpCode::from(self.read_u8());
            match opcode {
                OpCode::Return => {
                    // pop function
                    self.stack.pop();
                    self.frames.pop();
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
                OpCode::To => self.binary_op(Value::range)?,
                OpCode::Negate => self.unary_op(Value::neg)?,
                OpCode::Not => self.unary_op(Value::not)?,
                OpCode::EndExpr => {
                    self.last_value = self.stack.pop();
                },
                OpCode::EndBlock => {
                    let n_pops = self.read_u16();
                    self.stack.truncate(self.stack.len() - n_pops as usize);
                    self.stack.push(match &self.last_value {
                        Some(x) => x.clone(),
                        None => Value::Bool(false),
                    });
                },
                OpCode::Jump => {
                    let offset = self.read_u16();
                    let ip = &mut self.frame_mut().ip;
                    *ip += offset as usize;
                },
                OpCode::JumpIfFalse => {
                    let offset = self.read_u16();
                    let condition = match self.stack.pop() {
                        Some(x) => x,
                        None => return Err(InterpreterError::RuntimeError("Attempted to jump with null condition")),
                    };
                    match condition {
                        Value::Bool(false) => {
                            let ip = &mut self.frame_mut().ip;
                            *ip += offset as usize;
                        },
                        Value::Bool(true) => (),
                        _ => return Err(InterpreterError::RuntimeError("Expected boolean in condition")),
                    }
                },
                OpCode::Call => {
                    let n_args = self.read_u8();
                    match &self.stack[self.stack.len()-n_args as usize-1] {
                        Value::Closure(f) => self.call_function(n_args, f.clone())?,
                        Value::NativeFunction(f) => self.call_native_function(n_args, f)?,
                        Value::Array(arr) => self.array_index(n_args, &arr.clone())?,
                        _ => return Err(InterpreterError::RuntimeError("Attempted to call non-function")),
                    };
                },
                OpCode::Array => {
                    let n_elems = self.read_u16();
                    let array = Rc::new(
                        self.stack.split_off(self.stack.len() - n_elems as usize)
                    );
                    self.stack.push(Value::Array(array));
                },
                OpCode::Map => {
                    let args = self.stack.split_off(self.stack.len() - 2);
                    let result = (builtins::MAP.function)(self, &args)?;
                    self.stack.push(result);
                },
                OpCode::Constant => {
                    let constant = self.read_constant().clone();
                    self.stack.push(constant);
                },
                OpCode::SetGlobal => {
                    let name = self.read_constant();
                    let name = match name.clone() {
                        Value::String(name) => name,
                        _ => unreachable!("Global name was not a string"),
                    };
                    let value = match self.stack.last() {
                        Some(x) => x.clone(),
                        None => return Err(InterpreterError::RuntimeError("Attempted to set null global")),
                    };
                    self.globals.insert(name.as_ref().clone(), value);
                },
                OpCode::GetGlobal => {
                    let name = match self.read_constant() {
                        Value::String(name) => name.clone(),
                        _ => unreachable!("Global name was not a string"),
                    };
                    match self.globals.get(name.as_ref()) {
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
                    let idx = self.read_u16();
                    let value = self.stack[self.frame().stack_idx + idx as usize].clone();
                    self.stack.push(value);
                },
                OpCode::Closure => {
                    let mut closure = match self.read_constant() {
                        Value::Closure(c) => c.clone(),
                        _ => unreachable!("Closure was not a function"),
                    };
                    for _ in 0..closure.function.num_upvalues {
                        let is_local = self.read_u8() == 1;
                        let index = self.read_u16();
                        closure.upvalues.push(if is_local {
                            self.stack[self.frame().stack_idx + index as usize].clone()
                        }
                        else {
                            self.frame().closure.upvalues[index as usize].clone()
                        });
                    }
                    self.stack.push(Value::Closure(closure));
                },
                OpCode::GetUpvalue => {
                    let idx = self.read_u16();
                    let value = self.frame().closure.upvalues[idx as usize].clone();
                    self.stack.push(value);
                },
            }
        }
    }

    pub fn interpret(&mut self, source: String) -> Result<Option<Value>, InterpreterError> {
        let function = Rc::new(compile(source).map_err(|_| InterpreterError::CompileError)?);
        self.init(function);
        self.call().map_err(|e| {
            // in case of error, clean up before returning
            self.stack.pop();
            self.frames.pop();
            e
        })
    }
}

