use std::collections::HashMap;
use std::ops::{Add, BitAnd, BitOr, Div, Mul, Neg, Not, Sub};
use std::rc::Rc;

use crate::chunk::Chunk;
use crate::values::{Object, TypeDef};
use crate::{Closure, Function, NativeFunction, OpCode, Value, builtins, compile};

#[derive(Debug)]
pub enum InterpreterError {
    CompileError,
    RuntimeError(String),
}

impl std::fmt::Display for InterpreterError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InterpreterError::CompileError => write!(f, "Compile error"),
            InterpreterError::RuntimeError(msg) => write!(f, "Runtime error: {}", msg),
        }
    }
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

    pub fn runtime_err(&self, e: String) -> InterpreterError {
        // print track
        println!("Function call trace:");
        for frame in self.frames.iter() {
            println!(" | Line {}, in {}...", frame.closure.function.chunk.line_num(frame.ip), frame.closure.function);
        }
        // return error
        InterpreterError::RuntimeError(e.to_string())
    }

    fn binary_op(&mut self, op: fn(Value, Value) -> Result<Value, String>) -> Result<(), InterpreterError> {
        match (self.stack.pop(), self.stack.pop()) {
            (Some(x), Some(y)) => match op(y, x) {
                Ok(x) => Ok(self.stack.push(x)),
                Err(e) => return Err(self.runtime_err(e)),
            },
            _ => panic!("Attempted to perform binary with empty stack"),
        }
    }

    fn unary_op(&mut self, op: fn(Value) -> Result<Value, String>) -> Result<(), InterpreterError> {
        match self.stack.pop() {
            Some(x) => match op(x) {
                Ok(x) => Ok(self.stack.push(x)),
                Err(e) => return Err(self.runtime_err(e)),
            },
            None => panic!("Attempted to perform unary with empty stack"),
        }
    }

    pub fn call_function(&mut self, n_args: u8, closure: Box<Closure>) -> Result<(), InterpreterError> {
        if n_args != closure.function.arity {
            return Err(self.runtime_err(
                format!("Incorrect number of arguments: Expected {}, got {}.", closure.function.arity, n_args)
            ));
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
            return Err(self.runtime_err(
                format!("Incorrect number of arguments: Expected {}, got {}.", function.arity, n_args)
            ));
        }
        let args = self.stack.split_off(self.stack.len() - n_args as usize);
        let result = (function.function)(self, &args)?;
        self.stack.pop();
        self.stack.push(result);
        Ok(())
    }

    pub fn array_index(&mut self, n_args: u8, arr: &Vec<Value>) -> Result<(), InterpreterError> {
        if n_args != 1 {
            return Err(self.runtime_err(
                format!("When accessing array, expected 1 index, got {}", n_args)
            ));
        }
        let result = match self.stack.pop() {
            Some(Value::Int(mut idx)) => {
                if idx < 0 {
                    idx = arr.len() as i64 + idx;
                }
                if idx < 0 || idx >= arr.len() as i64 {
                    return Err(self.runtime_err(
                        format!("Index {} out of bounds for array of length {}", idx, arr.len())
                    ));
                }
                arr[idx as usize].clone()
            },
            Some(x) => return Err(self.runtime_err(
                format!("When accessing array, expected index to be an integer, got {}", x)
            )),
            None => panic!("Attempted to perform array access with empty stack"),
        };
        self.stack.pop();
        self.stack.push(result);
        Ok(())
    }

    pub fn create_object(&mut self, n_args: u8, typedef: Rc<TypeDef>) -> Result<(), InterpreterError> {
        if n_args != typedef.fieldnames.len() as u8 {
            return Err(self.runtime_err(
                format!("When creating object, expected {} field names, got {}", typedef.fieldnames.len(), n_args)
            ));
        }
        let mut fields = HashMap::new();
        for name in typedef.fieldnames.iter().cloned().rev() {
            let value = self.stack.pop().expect("Call to create_object resulted in empty stack");
            fields.insert(name, value);
        }
        self.stack.pop();
        self.stack.push(Value::Object(Rc::new(Object::new(typedef, fields))));
        Ok(())
    }

    pub fn get_field(&mut self, n_args: u8, obj: Rc<Object>) -> Result<(), InterpreterError> {
        if n_args != 1 {
            return Err(self.runtime_err(
                format!("When accessing field, expected 1 field name, got {}", n_args)
            ));
        }
        let result = match self.stack.pop() {
            Some(Value::String(name)) => {
                match obj.fields.get(name.as_ref()) {
                    Some(x) => x.clone(),
                    None => return Err(self.runtime_err(
                        format!("Field {} not found in {}", name, obj)
                    )),
                }
            },
            Some(Value::Int(mut idx)) => {
                if idx < 0 {
                    idx = obj.fields.len() as i64 + idx;
                }
                if idx < 0 || idx >= obj.fields.len() as i64 {
                    return Err(self.runtime_err(
                        format!("Field index {} out of bounds for object of size {}", idx, obj.fields.len())
                    ));
                }
                let fieldname = &obj.typedef.fieldnames[idx as usize];
                obj.fields.get(fieldname).unwrap().clone()
            },
            Some(x) => return Err(self.runtime_err(
                format!("When accessing field, expected field name to be a string or integer, got {}", x)
            )),
            None => panic!("Attempted to perform field access with empty stack"),
        };
        self.stack.pop();
        self.stack.push(result);
        Ok(())
    }

    fn call(&mut self) -> Result<Option<Value>, InterpreterError> {
        if self.frames.is_empty() {
            panic!("Attempted to call with no active call frame");
        }
        #[cfg(feature = "debug")]
        {
            println!("==call {:?}==", self.frame().closure);
            self.chunk().disassemble();
            println!("");
        }
        loop {
            if self.frame().ip >= self.chunk().len() {
                panic!("Reached end of chunk with no return");
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
                        None => panic!("Attempted to jump with null condition"),
                    };
                    match condition {
                        Value::Bool(false) => {
                            let ip = &mut self.frame_mut().ip;
                            *ip += offset as usize;
                        },
                        Value::Bool(true) => (),
                        x => return Err(self.runtime_err(
                            format!("Expected boolean in condition, found {}", x)
                        )),
                    }
                },
                OpCode::Call => {
                    let n_args = self.read_u8();
                    match &self.stack[self.stack.len()-n_args as usize-1] {
                        Value::Closure(f) => self.call_function(n_args, f.clone())?,
                        Value::NativeFunction(f) => self.call_native_function(n_args, f)?,
                        Value::Array(arr) => self.array_index(n_args, arr.clone().as_ref())?,
                        Value::TypeDef(td) => self.create_object(n_args, td.clone())?,
                        Value::Object(obj) => self.get_field(n_args, obj.clone())?,
                        x => return Err(self.runtime_err(
                            format!("Attempted to call non-callable {}", x)
                        )),
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
                        None => panic!("Attempted to set global with empty stack"),
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
                        None => return Err(self.runtime_err(
                            format!("Could not find global variable with name {}", name)
                        )),
                    };
                }
                OpCode::SetLocal => {
                    let value = self.stack.last();
                    match value {
                        Some(x) => self.stack.push(x.clone()),
                        None => panic!("Attempted to set local with empty stack"),
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

