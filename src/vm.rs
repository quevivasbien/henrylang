use std::cell::RefCell;
use std::collections::HashMap;
use std::ops::{Add, Mul, Sub, Div, Neg};
use std::rc::Rc;

use crate::ast;
use crate::builtins;
use crate::chunk::{Chunk, OpCode};
use crate::compiler;
use crate::values::{Closure, Function, HeapValue, NativeFunction, Object, ReturnValue, TaggedValue, TypeDef, Value};

#[derive(Debug)]
pub enum InterpreterError {
    CompileError(String),
    RuntimeError(String),
}

impl std::fmt::Display for InterpreterError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InterpreterError::CompileError(msg) => write!(f, "Compile error: {}", msg),
            InterpreterError::RuntimeError(msg) => write!(f, "Runtime error: {}", msg),
        }
    }
}

pub struct CallFrame {
    closure: Box<Closure>,
    ip: usize,
    stack_idx: usize,
    heap_stack_idx: usize,
}

impl CallFrame {
    fn new(closure: Box<Closure>, stack_idx: usize, heap_stack_idx: usize) -> Self {
        Self { closure, ip: 0, stack_idx, heap_stack_idx }
    }
}

pub struct VM {
    pub stack: Vec<Value>,
    pub heap_stack: Vec<HeapValue>,
    pub globals: HashMap<String, Value>,
    pub heap_globals: HashMap<String, HeapValue>,
    pub frames: Vec<CallFrame>,
    pub typecontext: compiler::TypeContext,
}

impl VM {
    pub fn new() -> Self {
        Self {
            stack: Vec::new(),
            heap_stack: Vec::new(),
            globals: builtins::builtins(),
            heap_globals: builtins::heap_builtins(),
            frames: Vec::new(),
            typecontext: Rc::new(RefCell::new(builtins::builtin_types())),
        }
    }

    fn init(&mut self, function: Rc<Function>) {
        let closure = Box::new(Closure::new(function));
        let frame = CallFrame::new(closure, 0, 0);
        self.frames.push(frame);
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
    fn read_constant(&mut self) -> Value {
        let ip = unsafe { &mut (*self.frame_ptr()).ip };
        self.chunk().read_constant(ip)
    }
    fn read_heap_constant(&mut self) -> &HeapValue {
        let ip = unsafe { &mut (*self.frame_ptr()).ip };
        self.chunk().read_heap_constant(ip)
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

    fn binary_bool_op(&mut self, op: fn(bool, bool) -> bool) {
        let r = self.stack.pop().expect("Attempted to perform binary operation with empty stack");
        let l = self.stack.pop().expect("Attempted to perform binary operation without enough values on the stack");
        let (l, r) = unsafe { (l.b, r.b) };
        self.stack.push(Value { b: op(l, r) });
    }
    fn unary_bool_op(&mut self, op: fn(bool) -> bool) {
        let r = self.stack.pop().expect("Attempted to perform unary operation with empty stack");
        let r = unsafe { r.b };
        self.stack.push(Value { b: op(r) });
    }

    fn binary_int_comp(&mut self, op: fn(&i64, &i64) -> bool) {
        let r = self.stack.pop().expect("Attempted to perform binary comparison with empty stack");
        let l = self.stack.pop().expect("Attempted to perform binary comparison without enough values on the stack");
        let (l, r) = unsafe { (l.i, r.i) };
        self.stack.push(Value { b: op(&l, &r) });
    }
    fn binary_int_op(&mut self, op: fn(i64, i64) -> i64) {
        let r = self.stack.pop().expect("Attempted to perform binary operation with empty stack");
        let l = self.stack.pop().expect("Attempted to perform binary operation without enough values on the stack");
        let (l, r) = unsafe { (l.i, r.i) };
        self.stack.push(Value { i: op(l, r) });
    }
    fn unary_int_op(&mut self, op: fn(i64) -> i64) {
        let r = self.stack.pop().expect("Attempted to perform unary operation with empty stack");
        let r = unsafe { r.i };
        self.stack.push(Value { i: op(r) });
    }

    fn binary_float_comp(&mut self, op: fn(&f64, &f64) -> bool) {
        let r = self.stack.pop().expect("Attempted to perform binary comparison with empty stack");
        let l = self.stack.pop().expect("Attempted to perform binary comparison without enough values on the stack");
        let (l, r) = unsafe { (l.f, r.f) };
        self.stack.push(Value { b: op(&l, &r) });
    }
    fn binary_float_op(&mut self, op: fn(f64, f64) -> f64) {
        let r = self.stack.pop().expect("Attempted to perform binary operation with empty stack");
        let l = self.stack.pop().expect("Attempted to perform binary operation without enough values on the stack");
        let (l, r) = unsafe { (l.f, r.f) };
        self.stack.push(Value { f: op(l, r) });
    }
    fn unary_float_op(&mut self, op: fn(f64) -> f64) {
        let r = self.stack.pop().expect("Attempted to perform unary operation with empty stack");
        let r = unsafe { r.f };
        self.stack.push(Value { f: op(r) });
    }

    pub fn call_function(&mut self, closure: Box<Closure>) -> Result<(), InterpreterError> {
        let n_args = closure.function.arity as usize;
        let n_heap_args = closure.function.heap_arity as usize;
        let new_frame = CallFrame::new(closure, self.stack.len() - n_args, self.heap_stack.len() - n_heap_args);
        self.frames.push(new_frame);
        let result = self.call()?;
        // clear stack used by function and function args
        self.stack.truncate(self.stack.len() - n_args);
        self.heap_stack.truncate(self.heap_stack.len() - n_heap_args);
        match result {
            ReturnValue::Value(v) => self.stack.push(v),
            ReturnValue::HeapValue(v) => self.heap_stack.push(v),
        };
        Ok(())
    }

    pub fn call_native_function(&mut self, function: &'static NativeFunction) -> Result<(), InterpreterError> {
        let args = self.stack.split_off(self.stack.len() - function.arity as usize);
        let heap_args = self.heap_stack.split_off(self.heap_stack.len() - function.heap_arity as usize);
        let result = (function.function)(self, &args, &heap_args)?;
        match result {
            ReturnValue::Value(v) => self.stack.push(v),
            ReturnValue::HeapValue(v) => self.heap_stack.push(v),
        };
        Ok(())
    }

    pub fn array_index(&mut self, arr: &Vec<Value>) -> Result<(), InterpreterError> {
        unimplemented!("array_index")
        // let result = match self.stack.pop().expect("Attempted to access array with empty stack") {
        //     Value::Int(mut idx) => {
        //         if idx < 0 {
        //             idx = arr.len() as i64 + idx;
        //         }
        //         if idx < 0 || idx >= arr.len() as i64 {
        //             return Err(self.runtime_err(
        //                 format!("Index {} out of bounds for array of length {}", idx, arr.len())
        //             ));
        //         }
        //         arr[idx as usize].clone()
        //     },
        //     x => return Err(self.runtime_err(
        //         format!("When accessing array, expected index to be an integer, got {}", x)
        //     )),
        // };
        // self.stack.push(result);
        // Ok(())
    }

    pub fn create_object(&mut self, typedef: Rc<TypeDef>) -> Result<(), InterpreterError> {
        unimplemented!("create_object")
        // let mut fields = HashMap::new();
        // for name in typedef.fieldnames.iter().cloned().rev() {
        //     let value = self.stack.pop().expect("Call to create_object resulted in empty stack");
        //     fields.insert(name, value);
        // }
        // self.stack.push(Value::Object(Rc::new(Object::new(typedef, fields))));
        // Ok(())
    }

    pub fn get_field(&mut self, obj: Rc<Object>) -> Result<(), InterpreterError> {
        unimplemented!("get_field")
        // let result = match self.stack.pop().expect("Attempted to access field with empty stack") {
        //     Value::String(name) => {
        //         match obj.fields.get(name.as_ref()) {
        //             Some(x) => x.clone(),
        //             None => return Err(self.runtime_err(
        //                 format!("Field {} not found in {}", name, obj)
        //             )),
        //         }
        //     },
        //     Value::Int(mut idx) => {
        //         if idx < 0 {
        //             idx = obj.fields.len() as i64 + idx;
        //         }
        //         if idx < 0 || idx >= obj.fields.len() as i64 {
        //             return Err(self.runtime_err(
        //                 format!("Field index {} out of bounds for object of size {}", idx, obj.fields.len())
        //             ));
        //         }
        //         let fieldname = &obj.typedef.fieldnames[idx as usize];
        //         obj.fields.get(fieldname).unwrap().clone()
        //     },
        //     x => return Err(self.runtime_err(
        //         format!("When accessing field, expected field name to be a string or integer, got {}", x)
        //     )),
        // };
        // self.stack.push(result);
        // Ok(())
    }

    fn call(&mut self) -> Result<ReturnValue, InterpreterError> {
        if self.frames.is_empty() {
            panic!("Attempted to call with no active call frame");
        }
        #[cfg(feature = "debug")]
        {
            // println!("==call {:?}==", self.frame().closure);
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
                print!("heap_stack: {:?}, ", &self.heap_stack[self.frame().heap_stack_idx..]);
                print!("globals: {:?}, ", self.globals);
                print!("heap globals: {:?}, ", self.heap_globals);
                let mut ip_copy = self.frame().ip;
                self.chunk().disassemble_instruction(&mut ip_copy);
            }
            let opcode = OpCode::from(self.read_u8());
            match opcode {
                OpCode::Return => {
                    // pop function
                    self.frames.pop();
                    return Ok(ReturnValue::Value(self.stack.pop().unwrap()));
                },
                OpCode::ReturnHeap => {
                    // pop function
                    self.frames.pop();
                    return Ok(ReturnValue::HeapValue(self.heap_stack.pop().unwrap()));
                },


                OpCode::True => self.stack.push(Value::from_bool(true)),
                OpCode::False => self.stack.push(Value::from_bool(false)),
                OpCode::Constant => {
                    let constant = self.read_constant();
                    self.stack.push(constant);
                },
                OpCode::HeapConstant => {
                    let constant = self.read_heap_constant().clone();
                    self.heap_stack.push(constant);
                },

                // Boolean ops
                OpCode::And => self.binary_bool_op(|x, y| x && y),
                OpCode::Or => self.binary_bool_op(|x, y| x || y),
                OpCode::Not => self.unary_bool_op(|x| !x),

                // Int ops
                OpCode::IntEqual => self.binary_int_comp(i64::eq),
                OpCode::IntNotEqual => self.binary_int_comp(i64::ne),
                OpCode::IntLess => self.binary_int_comp(i64::lt),
                OpCode::IntLessEqual => self.binary_int_comp(i64::le),
                OpCode::IntGreater => self.binary_int_comp(i64::gt),
                OpCode::IntGreaterEqual => self.binary_int_comp(i64::ge),
                OpCode::IntAdd => self.binary_int_op(i64::add),
                OpCode::IntSubtract => self.binary_int_op(i64::sub),
                OpCode::IntMultiply => self.binary_int_op(i64::mul),
                OpCode::IntDivide => self.binary_int_op(i64::div),
                OpCode::IntNegate => self.unary_int_op(i64::neg),
                OpCode::To => {
                    let r = self.stack.pop().expect("Expected int on stack");
                    let l = self.stack.pop().expect("Expected int on stack");
                    let (r, l) = unsafe {
                        (r.i, l.i)
                    };
                    let arr: Vec<_> = if r >= l {
                        (l..=r).map(|i| Value { i }).collect()
                    }
                    else {
                        (r..=l).rev().map(|i| Value { i }).collect()
                    };
                    self.heap_stack.push(HeapValue::Array(Rc::from(arr)));
                }

                // Float ops
                OpCode::FloatEqual => self.binary_float_comp(f64::eq),
                OpCode::FloatNotEqual => self.binary_float_comp(f64::ne),
                OpCode::FloatLess => self.binary_float_comp(f64::lt),
                OpCode::FloatLessEqual => self.binary_float_comp(f64::le),
                OpCode::FloatGreater => self.binary_float_comp(f64::gt),
                OpCode::FloatGreaterEqual => self.binary_float_comp(f64::ge),
                OpCode::FloatAdd => self.binary_float_op(f64::add),
                OpCode::FloatSubtract => self.binary_float_op(f64::sub),
                OpCode::FloatMultiply => self.binary_float_op(f64::mul),
                OpCode::FloatDivide => self.binary_float_op(f64::div),
                OpCode::FloatNegate => self.unary_float_op(f64::neg),

                // Array ops
                OpCode::Concat => {
                    let r = self.heap_stack.pop().expect("Expected array on heap stack");
                    let l = self.heap_stack.pop().expect("Expected array on stack");
                    match (r, l) {
                        (HeapValue::Array(r), HeapValue::Array(l)) => {
                            let new_arr = Rc::from([l, r].concat());
                            self.heap_stack.push(HeapValue::Array(new_arr));
                        },
                        (HeapValue::String(r), HeapValue::String(l)) => {
                            let new_string = Rc::from(format!("{}{}", l, r));
                            self.heap_stack.push(HeapValue::String(new_string));
                        },
                        (HeapValue::ArrayHeap(r), HeapValue::ArrayHeap(l)) => {
                            let new_arr = Rc::from([l, r].concat());
                            self.heap_stack.push(HeapValue::ArrayHeap(new_arr));
                        },
                        (r, l) => panic!("Expected two arrays or array-likes of same type on heap stack, got {:?} and {:?}", r, l)
                    };
                },
                
                OpCode::EndExpr => {
                    self.stack.pop();
                },
                OpCode::EndHeapExpr => {
                    self.heap_stack.pop();
                },
                OpCode::EndBlock => {
                    let n_pops = self.read_u16();
                    let n_heap_pops = self.read_u16();
                    let last_value = self.stack.pop().expect("Attempted to end block with nothing on the stack");
                    self.stack.truncate(self.stack.len() - n_pops as usize);
                    self.heap_stack.truncate(self.heap_stack.len() - n_heap_pops as usize);
                    self.stack.push(last_value);
                },
                OpCode::EndHeapBlock => {
                    let n_pops = self.read_u16();
                    let n_heap_pops = self.read_u16();
                    let last_value = self.heap_stack.pop().expect("Attempted to end block with nothing on the stack");
                    self.stack.truncate(self.stack.len() - n_pops as usize);
                    self.heap_stack.truncate(self.heap_stack.len() - n_heap_pops as usize);
                    self.heap_stack.push(last_value);
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
                        None => unreachable!("Attempted to jump with null condition"),
                    };
                    let condition = unsafe { condition.b };
                    if !condition {
                        let ip = &mut self.frame_mut().ip;
                        *ip += offset as usize;
                    }
                },
                OpCode::Call => {
                    match self.heap_stack.pop().expect("Attempted to call with empty stack") {
                        HeapValue::Closure(f) => self.call_function(f)?,
                        HeapValue::NativeFunction(f) => self.call_native_function(f)?,
                        // HeapValue::Array(arr) => self.array_index(arr.as_ref())?,
                        // HeapValue::TypeDef(td) => self.create_object(td)?,
                        // HeapValue::Object(obj) => self.get_field(obj)?,
                        _ => unreachable!()
                    };
                },

                OpCode::Array => {
                    let n_elems = self.read_u16();
                    let arr = Rc::from(self.stack.split_off(self.stack.len() - n_elems as usize));
                    self.heap_stack.push(HeapValue::Array(arr));
                },
                OpCode::ArrayHeap => {
                    let n_elems = self.read_u16();
                    let arr = Rc::from(self.heap_stack.split_off(self.heap_stack.len() - n_elems as usize));
                    self.heap_stack.push(HeapValue::ArrayHeap(arr));
                }
                // OpCode::Map => {
                //     let args = self.stack.split_off(self.stack.len() - 2);
                //     let result = (builtins::MAP.function)(self, &args)?;
                //     self.stack.push(result);
                // },
                
                OpCode::SetGlobal => {
                    let name = self.read_heap_constant();
                    let name = match name {
                        HeapValue::String(name) => name.as_ref().clone(),
                        _ => unreachable!("Global name was not a string"),
                    };
                    let value = match self.stack.last() {
                        Some(x) => x.clone(),
                        None => panic!("Attempted to set global with empty stack"),
                    };
                    self.globals.insert(name, value);
                },
                OpCode::SetHeapGlobal => {
                    let name = self.read_heap_constant();
                    let name = match name {
                        HeapValue::String(name) => name.as_ref().clone(),
                        _ => unreachable!("Global name was not a string"),
                    };
                    let value = match self.heap_stack.last() {
                        Some(x) => x.clone(),
                        None => panic!("Attempted to set global with empty stack"),
                    };
                    self.heap_globals.insert(name, value);
                }
                OpCode::GetGlobal => {
                    let name = match self.read_heap_constant() {
                        HeapValue::String(name) => name.clone(),
                        _ => unreachable!("Global name was not a string"),
                    };
                    match self.globals.get(name.as_ref()) {
                        Some(x) => self.stack.push(*x),
                        None => unreachable!("Attempted to get global that does not exist"),
                    };
                },
                OpCode::GetHeapGlobal => {
                    let name = match self.read_heap_constant() {
                        HeapValue::String(name) => name.clone(),
                        _ => unreachable!("Global name was not a string"),
                    };
                    match self.heap_globals.get(name.as_ref()) {
                        Some(x) => self.heap_stack.push(x.clone()),
                        None => unreachable!("Attempted to get global that does not exist"),
                    };
                }
                OpCode::SetLocal => {
                    let value = self.stack.last();
                    match value {
                        Some(x) => self.stack.push(*x),
                        None => unreachable!("Attempted to set local with empty stack"),
                    }
                },
                OpCode::SetHeapLocal => {
                    let value = self.heap_stack.last();
                    match value {
                        Some(x) => self.heap_stack.push(x.clone()),
                        None => unreachable!("Attempted to set local with empty stack"),
                    }
                },
                OpCode::GetLocal => {
                    let idx = self.read_u16();
                    let value = self.stack[self.frame().stack_idx + idx as usize].clone();
                    self.stack.push(value);
                },
                OpCode::GetHeapLocal => {
                    let idx = self.read_u16();
                    let value = self.heap_stack[self.frame().heap_stack_idx + idx as usize].clone();
                    self.heap_stack.push(value);
                },
                OpCode::Closure => {
                    let mut closure = match self.read_heap_constant() {
                        HeapValue::Closure(c) => c.clone(),
                        _ => unreachable!("Closure was not a function"),
                    };
                    for _ in 0..closure.function.num_upvalues {
                        let is_local = self.read_u8() == 1;
                        let index = self.read_u16();
                        closure.upvalues.push(
                            if is_local {
                                self.stack[self.frame().stack_idx + index as usize].clone()
                            }
                            else {
                                self.frame().closure.upvalues[index as usize].clone()
                            }
                        )
                    }
                    for _ in 0..closure.function.num_heap_upvalues {
                        let is_local = self.read_u8() == 1;
                        let index = self.read_u16();
                        closure.heap_upvalues.push(
                            if is_local {
                                self.heap_stack[self.frame().heap_stack_idx + index as usize].clone()
                            }
                            else {
                                self.frame().closure.heap_upvalues[index as usize].clone()
                            }
                        )
                    }
                    self.heap_stack.push(HeapValue::Closure(closure));
                },
                OpCode::GetUpvalue => {
                    let idx = self.read_u16();
                    let value = self.frame().closure.upvalues[idx as usize].clone();
                    self.stack.push(value);
                },
                OpCode::GetHeapUpvalue => {
                    let idx = self.read_u16();
                    let value = self.frame().closure.heap_upvalues[idx as usize].clone();
                    self.heap_stack.push(value);
                }
                OpCode::WrapSome => {
                    let value = self.stack.pop().expect("Attempted to wrap with empty stack");
                    self.heap_stack.push(HeapValue::Maybe(Some(value)));
                },
                OpCode::WrapHeapSome => {
                    let value = self.heap_stack.pop().expect("Attempted to wrap with empty stack");
                    self.heap_stack.push(HeapValue::MaybeHeap(Some(Box::new(value))));
                },
                _ => unimplemented!("Opcode {:?} not implemented", opcode),
            }
        }
    }

    pub fn interpret(&mut self, source: String) -> Result<TaggedValue, InterpreterError> {
        let (function, return_type) = 
            compiler::compile(source, self.typecontext.clone())
            .map_err(|e| InterpreterError::CompileError(e))?
            ;
        let function = Rc::new(function);
        self.init(function);
        let result = self.call().map_err(|e| {
            // in case of error, clean up before returning
            self.stack.clear();
            self.frames.clear();
            e
        })?;
        unpack_result(result, &return_type).map_err(|e| InterpreterError::RuntimeError(e))
    }
}

fn unpack_result(result: ReturnValue, return_type: &ast::Type) -> Result<TaggedValue, String> {
    let hvalue = match result {
        ReturnValue::HeapValue(x) => x,
        ReturnValue::Value(x) => return Ok(
            TaggedValue::from_value(x, &return_type)?
        )
    };
    unpack_heapvalue(hvalue, return_type)
}

fn unpack_heapvalue(hvalue: HeapValue, return_type: &ast::Type) -> Result<TaggedValue, String> {
    match (hvalue, return_type) {
        (HeapValue::Array(arr), ast::Type::Array(typ)) => {
            Ok(TaggedValue::from_array(&arr, typ.as_ref()))
        },
        (HeapValue::String(s), ast::Type::String) => {
            Ok(TaggedValue::String(s.as_ref().clone()))
        },
        (HeapValue::Maybe(x), ast::Type::Maybe(typ)) => {
            TaggedValue::from_maybe(x, typ.as_ref())
        },
        (HeapValue::MaybeHeap(_x), ast::Type::Maybe(_typ)) => {
            Err("Unimplemented: unpack_heapvalue: MaybeHeap".to_string())
        },
        (HeapValue::ArrayHeap(arr), ast::Type::Array(typ)) => {
            match typ.as_ref() {
                ast::Type::String => {
                    let mut arr_s = Vec::new();
                    for inner in arr.iter() {
                        let s = match inner {
                            HeapValue::String(s) => s,
                            _ => unreachable!()
                        };
                        arr_s.push(TaggedValue::String(s.as_ref().clone()));
                    }
                    Ok(TaggedValue::Array(arr_s))
                },
                typ => {
                    let mut arr_arr = Vec::new();
                    for inner_arr in arr.iter().cloned() {
                        // let inner_arr = match inner_arr {
                        //     HeapValue::Array(arr) => arr,
                        //     _ => unreachable!()
                        // };
                        arr_arr.push(unpack_heapvalue(inner_arr, typ)?);
                    }
                    Ok(TaggedValue::Array(arr_arr))
                }
                // ast::Type::Array(typ) => {
                //     let mut arr_arr = Vec::new();
                //     for inner_arr in arr.iter().cloned() {
                //         // let inner_arr = match inner_arr {
                //         //     HeapValue::Array(arr) => arr,
                //         //     _ => unreachable!()
                //         // };
                //         arr_arr.push(unpack_heapvalue(inner_arr, typ.as_ref())?);
                //     }
                //     Ok(TaggedValue::Array(arr_arr))
                // },
                // _ => unreachable!("Expected an array of arrays but got something else"),
            }
        },
        (HeapValue::Closure(f), ast::Type::Function(..)) => {
            Ok(TaggedValue::Closure(f))
        },
        (x, rt) => return Err(
            format!(
                "Got unexpected return value: {:?}; expected {:?}",
                x, rt
            )
        ),
    }
}