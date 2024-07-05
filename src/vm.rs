use std::cell::RefCell;
use rustc_hash::FxHashMap;
use std::ops::{Add, Mul, Sub, Div, Neg};
use std::rc::Rc;

use crate::ast;
use crate::builtins;
use crate::chunk::{Chunk, OpCode};
use crate::compiler;
use crate::values::{ArrayIter, Closure, FilterIter, Function, HeapValue, IndexIter, LazyIter, MapIter, MapIterHeap, MapIterNative, MapIterNativeHeap, NativeFunction, Object, RangeIter, ReturnValue, ReverseRangeIter, TaggedValue, TypeDef, Value, ZipIter, ZipIterNative, ZipIterTypeDef};

#[derive(Debug, Clone)]
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
    pub globals: FxHashMap<String, Value>,
    pub heap_globals: FxHashMap<String, HeapValue>,
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
        let is_heap = closure.function.return_is_heap;
        let new_frame = CallFrame::new(closure, self.stack.len() - n_args, self.heap_stack.len() - n_heap_args);
        self.frames.push(new_frame);
        self.call()?;
        if is_heap {
            let result = self.heap_stack.pop().expect("Expected a heap return value from function");
            // clear stack used by function args
            self.stack.truncate(self.stack.len() - n_args);
            self.heap_stack.truncate(self.heap_stack.len() - n_heap_args);
            // push result back onto stack
            self.heap_stack.push(result);
        }
        else {
            let result = self.stack.pop().expect("Expected a return value from function");
            // clear stack used by function args
            self.stack.truncate(self.stack.len() - n_args);
            self.heap_stack.truncate(self.heap_stack.len() - n_heap_args);
            // push result back onto stack
            self.stack.push(result);
        }
        Ok(())
    }

    pub fn call_native_function(&mut self, function: &'static NativeFunction) -> Result<(), InterpreterError> {
        let args = self.stack.split_off(self.stack.len() - function.arity as usize);
        let heap_args = self.heap_stack.split_off(self.heap_stack.len() - function.heap_arity as usize);
        (function.function)(self, &args, &heap_args)
    }

    fn get_idx(&mut self, arr_len: usize) -> Result<i64, InterpreterError> {
        let idx = self.stack.pop().expect("Attempted to access array with empty stack");
        let mut idx = unsafe { idx.i };
        if idx < 0 {
            idx = arr_len as i64 + idx;
        }
        if idx < 0 || idx >= arr_len as i64 {
            return Err(self.runtime_err(
                format!("Index {} out of bounds for array of length {}", idx, arr_len)
            ));
        }
        Ok(idx)
    }
    pub fn array_index(&mut self, arr: &[Value]) -> Result<(), InterpreterError> {
        let idx = self.get_idx(arr.len())?;
        let result = unsafe { *arr.get_unchecked(idx as usize) };
        self.stack.push(result);
        Ok(())
    }
    pub fn array_heap_index(&mut self, arr: &[HeapValue]) -> Result<(), InterpreterError> {
        let idx = self.get_idx(arr.len())?;
        let result = unsafe { arr.get_unchecked(idx as usize).clone() };
        self.heap_stack.push(result);
        Ok(())
    }

    pub fn create_object(&mut self, typedef: Rc<TypeDef>) -> Result<(), InterpreterError> {
        let mut fields = FxHashMap::default();
        let mut heap_fields = FxHashMap::default();
        for (fieldname, is_heap) in typedef.fields.iter().cloned().rev() {
            if is_heap {
                let value = self.heap_stack.pop().expect("Call to create_object resulted in empty stack");
                heap_fields.insert(fieldname, value);
            }
            else {
                let value = self.stack.pop().expect("Call to create_object resulted in empty stack");
                fields.insert(fieldname, value);
            }
        }
        let obj = Object::new(typedef, fields, heap_fields);
        self.heap_stack.push(HeapValue::Object(Rc::new(obj)));
        Ok(())
    }

    pub fn get_field(&mut self, obj: Rc<Object>) -> Result<(), InterpreterError> {
        let fieldname = self.heap_stack.pop().expect("Attempted to access field with empty stack");
        let fieldname = match fieldname {
            HeapValue::String(name) => name,
            _ => unreachable!()
        };
        let is_heap = unsafe {
            self.stack.pop().expect("Attempted to access heap indicator with empty stack").b
        };
        if is_heap {
            let result = obj.heap_fields.get(fieldname.as_ref()).unwrap().clone();
            self.heap_stack.push(result);
        }
        else {
            let result = *obj.fields.get(fieldname.as_ref()).unwrap();
            self.stack.push(result);
        }
        Ok(())
    }

    fn push_map_result(&mut self, len: usize, is_heap: bool) {
        if is_heap {
            let lazy_iter = Box::new(
                ArrayIter::new(Rc::from(
                    self.heap_stack.split_off(self.heap_stack.len() - len)
                ))
            );
            self.heap_stack.push(HeapValue::LazyIterHeap(lazy_iter));
        }
        else {
            let lazy_iter = Box::new(
                ArrayIter::new(Rc::from(
                    self.stack.split_off(self.stack.len() - len)
                ))
            );
            self.heap_stack.push(HeapValue::LazyIter(lazy_iter));
        }
    }

    fn map(&mut self) -> Result<(), InterpreterError> {
        let arg = self.heap_stack.pop().expect("Expected argument array on heap stack");
        let callee = self.heap_stack.pop().expect("Expected callable on heap stack");

        match (callee, arg) {
            // Closure -> LazyIter
            (HeapValue::Closure(f), HeapValue::LazyIter(iter)) => {
                if f.function.return_is_heap {
                    let map_iter = Box::new(MapIterHeap::new(iter, f, self));
                    self.heap_stack.push(HeapValue::LazyIterHeap(map_iter));
                }
                else {
                    let map_iter = Box::new(MapIter::new(iter, f, self));
                    self.heap_stack.push(HeapValue::LazyIter(map_iter));
                }
            },
            // Closure -> LazyIterHeap
            (HeapValue::Closure(f), HeapValue::LazyIterHeap(iter)) => {
                if f.function.return_is_heap {
                    let map_iter = Box::new(MapIterHeap::new(iter, f, self));
                    self.heap_stack.push(HeapValue::LazyIterHeap(map_iter));
                }
                else {
                    let map_iter = Box::new(MapIter::new(iter, f, self));
                    self.heap_stack.push(HeapValue::LazyIter(map_iter));
                }
            },
            // Closure -> Array
            (HeapValue::Closure(f), HeapValue::Array(a)) => {
                let n_calls = a.len();
                for a in a.iter(){
                    self.stack.push(*a);
                    self.call_function(f.clone())?;
                }
                self.push_map_result(n_calls, f.function.return_is_heap);
            },
            // Closure -> ArrayHeap
            (HeapValue::Closure(f), HeapValue::ArrayHeap(a)) => {
                let n_calls = a.len();
                for a in a.iter(){
                    self.heap_stack.push(a.clone());
                    self.call_function(f.clone())?;
                }
                self.push_map_result(n_calls, f.function.return_is_heap);
            },
            // NativeFunction -> LazyIter
            (HeapValue::NativeFunction(f), HeapValue::LazyIter(iter)) => {
                let map_iter = Box::new(MapIterNative::new(iter, f, self));
                self.heap_stack.push(HeapValue::LazyIter(map_iter));
            },
            // NativeFunction -> LazyIterHeap
            (HeapValue::NativeFunction(f), HeapValue::LazyIterHeap(iter)) => {
                let map_iter = Box::new(MapIterNativeHeap::new(iter, f, self));
                self.heap_stack.push(HeapValue::LazyIterHeap(map_iter));
            },
            // NativeFunction -> Array
            (HeapValue::NativeFunction(f), HeapValue::Array(a)) => {
                let n_calls = a.len();
                for a in a.iter(){
                    self.stack.push(*a);
                    self.call_native_function(f)?;
                }
                self.push_map_result(n_calls, f.return_is_heap);
            },
            // NativeFunction -> ArrayHeap
            (HeapValue::NativeFunction(f), HeapValue::ArrayHeap(a)) => {
                let n_calls = a.len();
                for a in a.iter(){
                    self.heap_stack.push(a.clone());
                    self.call_native_function(f)?;
                }
                self.push_map_result(n_calls, f.return_is_heap);
            },
            // Array -> LazyIter of indices
            (HeapValue::Array(a), HeapValue::LazyIter(iter)) => {
                let index_iter = Box::new(IndexIter::new(
                    iter, a
                ));
                self.heap_stack.push(HeapValue::LazyIter(index_iter));
            },
            // ArrayHeap -> LazyIter of indices
            (HeapValue::ArrayHeap(a), HeapValue::LazyIter(iter)) => {
                let index_iter = Box::new(IndexIter::new(
                    iter, a
                ));
                self.heap_stack.push(HeapValue::LazyIterHeap(index_iter));
            },
            // Array -> Array of indices
            (HeapValue::Array(a), HeapValue::Array(idxs)) => {
                let arr = Rc::from(idxs.into_iter().map(|x| {
                    let idx = unsafe { x.i };
                    a[idx as usize]
                }).collect::<Vec<_>>());
                let iter = Box::new(ArrayIter::new(arr));
                self.heap_stack.push(HeapValue::LazyIter(iter));
            },
            // ArrayHeap -> Array of indices
            (HeapValue::ArrayHeap(a), HeapValue::Array(idxs)) => {
                let arr = Rc::from(idxs.into_iter().map(|x| {
                    let idx = unsafe { x.i };
                    a[idx as usize].clone()
                }).collect::<Vec<_>>());
                let iter = Box::new(ArrayIter::new(arr));
                self.heap_stack.push(HeapValue::LazyIterHeap(iter));
            },
            _ => unreachable!(),
        }
        Ok(())
    }

    fn call(&mut self) -> Result<(), InterpreterError> {
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
            #[cfg(feature = "debug")]
            if self.frame().ip >= self.chunk().len() {
                panic!("Reached end of chunk with no return");
            }
            #[cfg(feature = "debug")]
            {
                print!("frames: {}, ", self.frames.len());
                print!("stack: {:?}, ", &self.stack[self.frame().stack_idx..]);
                print!("heap_stack: {:?}, ", &self.heap_stack[self.frame().heap_stack_idx..]);
                // print!("globals: {:?}, ", self.globals);
                // print!("heap globals: {:?}, ", self.heap_globals);
                let mut ip_copy = self.frame().ip;
                self.chunk().disassemble_instruction(&mut ip_copy);
            }
            let opcode = OpCode::from(self.read_u8());
            match opcode {
                OpCode::Return => {
                    // pop function
                    self.frames.pop();
                    return Ok(());
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
                OpCode::BoolEqual => self.binary_bool_op(|x, y| x == y),
                OpCode::BoolNotEqual => self.binary_bool_op(|x, y| x != y),
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
                    let lazy_iter: Box<dyn LazyIter<Value>> = if r >= l {
                        Box::new(RangeIter::new(l, r))
                    }
                    else {
                        Box::new(ReverseRangeIter::new(l, r))
                    };
                    self.heap_stack.push(HeapValue::LazyIter(lazy_iter));
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
                OpCode::HeapEqual => {
                    let r = self.heap_stack.pop().expect("Expected array on heap stack");
                    let l = self.heap_stack.pop().expect("Expected array on stack");
                    self.stack.push(Value::from_bool(l == r));
                },
                OpCode::HeapNotEqual => {
                    let r = self.heap_stack.pop().expect("Expected array on heap stack");
                    let l = self.heap_stack.pop().expect("Expected array on stack");
                    self.stack.push(Value::from_bool(l != r));
                },
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
                        HeapValue::Array(arr) => self.array_index(arr.as_ref())?,
                        HeapValue::ArrayHeap(arr) => self.array_heap_index(arr.as_ref())?,
                        HeapValue::TypeDef(td) => self.create_object(td)?,
                        HeapValue::Object(obj) => self.get_field(obj)?,
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
                },

                OpCode::Collect => {
                    let mut iter = self.heap_stack.pop().expect("Attempted to collect with empty stack");
                    match &mut iter {
                        HeapValue::LazyIter(iter) => {
                            self.heap_stack.push(HeapValue::Array(iter.into_array()));
                        },
                        HeapValue::LazyIterHeap(iter) => {
                            self.heap_stack.push(HeapValue::ArrayHeap(iter.into_array()));
                        },
                        _ => unreachable!(),
                    }
                }
                
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
                        None => unreachable!("Attempted to get global {} that does not exist", name),
                    };
                },
                OpCode::GetHeapGlobal => {
                    let name = match self.read_heap_constant() {
                        HeapValue::String(name) => name.clone(),
                        _ => unreachable!("Global name was not a string"),
                    };
                    match self.heap_globals.get(name.as_ref()) {
                        Some(x) => self.heap_stack.push(x.clone()),
                        None => unreachable!("Attempted to get global {} that does not exist", name),
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
                },

                OpCode::WrapSome => {
                    let value = self.stack.pop().expect("Attempted to wrap with empty stack");
                    self.heap_stack.push(HeapValue::Maybe(Some(value)));
                },
                OpCode::WrapHeapSome => {
                    let value = self.heap_stack.pop().expect("Attempted to wrap with empty stack");
                    self.heap_stack.push(HeapValue::MaybeHeap(Some(Box::new(value))));
                },
                OpCode::IsSome => {
                    let value = self.heap_stack.pop().expect("Attempted to unwrap with empty stack");
                    let is_some = match value {
                        HeapValue::Maybe(Some(_)) => true,
                        HeapValue::Maybe(None) => false,
                        HeapValue::MaybeHeap(Some(_)) => true,
                        HeapValue::MaybeHeap(None) => false,
                        _ => unreachable!(),
                    };
                    self.stack.push(Value::from_bool(is_some));
                }
                OpCode::Unwrap => {
                    let value = self.heap_stack.pop().expect("Attempted to unwrap with empty stack");
                    match value {
                        HeapValue::Maybe(Some(x)) => {
                            self.stack.pop().expect("Expected default value on top of stack");
                            self.stack.push(x);
                        },
                        HeapValue::Maybe(None) => (),  // default should be on top of stack, so we can just leave it there
                        _ => unreachable!(),
                    }
                },
                OpCode::UnwrapHeap => {
                    let value = self.heap_stack.pop().expect("Attempted to unwrap with empty stack");
                    match value {
                        HeapValue::MaybeHeap(Some(x)) => {
                            self.heap_stack.pop().expect("Expected default value on top of heap stack");
                            self.heap_stack.push(*x);
                        },
                        HeapValue::MaybeHeap(None) => (),  // default should be on top of stack, so we can just leave it there
                        _ => unreachable!(),
                    }
                },

                OpCode::Len => {
                    let value = self.heap_stack.pop().expect("Expected value on stack");
                    let len = match value {
                        HeapValue::Array(a) => a.len(),
                        HeapValue::ArrayHeap(a) => a.len(),
                        HeapValue::String(s) => s.chars().count(),
                        HeapValue::LazyIter(iter) => iter.count(),
                        HeapValue::LazyIterHeap(iter) => iter.count(),
                        _ => unreachable!(),
                    };
                    self.stack.push(Value { i: len as i64 });
                }

                OpCode::Map => self.map()?,

                OpCode::Reduce => {
                    let f = self.heap_stack.pop().expect("Expected function on heap stack");
                    let arr = self.heap_stack.pop().expect("Expected array on heap stack");

                    match (f, arr) {
                        // reduce(closure, iter, init)
                        (HeapValue::Closure(f), HeapValue::LazyIter(iter)) => {
                            for x in iter.into_iter() {
                                self.stack.push(x);
                                self.call_function(f.clone())?;
                            }
                        },
                        // reduce(fnative, iter, init)
                        (HeapValue::NativeFunction(f), HeapValue::LazyIter(iter)) => {
                            for x in iter.into_iter() {
                                self.stack.push(x);
                                self.call_native_function(f)?;
                            }
                        },
                        // reduce(closure, iterheap, init)
                        (HeapValue::Closure(f), HeapValue::LazyIterHeap(iter)) => {
                            for x in iter.into_iter() {
                                self.heap_stack.push(x);
                                self.call_function(f.clone())?;
                            }
                        },
                        // reduce(fnative, iterheap, init)
                        (HeapValue::NativeFunction(f), HeapValue::LazyIterHeap(iter)) => {
                            for x in iter.into_iter() {
                                self.heap_stack.push(x);
                                self.call_native_function(f)?;
                            }
                        },
                        // reduce(closure, array, init)
                        (HeapValue::Closure(f), HeapValue::Array(a)) => {
                            for x in a.iter() {
                                self.stack.push(*x);
                                self.call_function(f.clone())?;
                            }
                        },
                        // reduce(fnative, array, init)
                        (HeapValue::NativeFunction(f), HeapValue::Array(a)) => {
                            for x in a.iter() {
                                self.stack.push(*x);
                                self.call_native_function(f)?;
                            }
                        },
                        // reduce(closure, arrayheap, init)
                        (HeapValue::Closure(f), HeapValue::ArrayHeap(a)) => {
                            for x in a.iter() {
                                self.heap_stack.push(x.clone());
                                self.call_function(f.clone())?;
                            }
                        },
                        // reduce(fnative, arrayheap, init)
                        (HeapValue::NativeFunction(f), HeapValue::ArrayHeap(a)) => {
                            for x in a.iter() {
                                self.heap_stack.push(x.clone());
                                self.call_native_function(f)?;
                            }
                        },
                        _ => unreachable!(),
                    }
                },

                OpCode::Filter => {
                    let arr = self.heap_stack.last().expect("Expected array on top of stack").clone();
                    self.map()?;
                    let bool_iter = match self.heap_stack.pop().expect("Expected bool iterator on stack after mapping through filter function") {
                        HeapValue::LazyIter(a) => a,
                        _ => unreachable!(),
                    };
                    match arr {
                        HeapValue::LazyIter(value_iter) => {
                            let iter = Box::new(FilterIter::new(bool_iter, value_iter));
                            self.heap_stack.push(HeapValue::LazyIter(iter));
                        },
                        HeapValue::LazyIterHeap(value_iter) => {
                            let iter = Box::new(FilterIter::new(bool_iter, value_iter));
                            self.heap_stack.push(HeapValue::LazyIterHeap(iter));
                        },
                        HeapValue::Array(a) => {
                            let res = a.iter().zip(bool_iter.into_iter())
                                .filter(|(_, b)| unsafe { b.b })
                                .map(|(&x, _)| x)
                                .collect::<Vec<_>>();
                            let iter = Box::new(ArrayIter::new(Rc::from(res)));
                            self.heap_stack.push(HeapValue::LazyIter(iter));
                        },
                        HeapValue::ArrayHeap(a) => {
                            let res = a.iter().zip(bool_iter.into_iter())
                                .filter(|(_, b)| unsafe { b.b })
                                .map(|(x, _)| x.clone())
                                .collect::<Vec<_>>();
                            let iter = Box::new(ArrayIter::new(Rc::from(res)));
                            self.heap_stack.push(HeapValue::LazyIterHeap(iter));
                        },
                        _ => unreachable!(),
                    }
                },

                OpCode::ZipMap => {
                    let f = self.heap_stack.pop().expect("Expected function on heap stack");
                    let n_iters = unsafe { self.stack.pop().expect("Expected number of arrays on stack").i };
                    let mut iters = Vec::new();
                    let mut heap_iters = Vec::new();
                    for _ in 0..n_iters {
                        let hv = self.heap_stack.pop().expect("Expected array on heap stack");
                        match hv {
                            HeapValue::LazyIter(i) => iters.push(i),
                            HeapValue::LazyIterHeap(i) => heap_iters.push(i),
                            HeapValue::Array(a) => iters.push(Box::new(ArrayIter::new(Rc::from(a)))),
                            HeapValue::ArrayHeap(a) => heap_iters.push(Box::new(ArrayIter::new(Rc::from(a)))),
                            _ => unreachable!(),
                        }
                    }
                    let zip_iter = match f {
                        HeapValue::Closure(c) => {
                            let is_heap = c.function.return_is_heap;
                            let iter = Box::new(ZipIter::new(iters, heap_iters, c, self));
                            if is_heap {
                                HeapValue::LazyIterHeap(iter)
                            }
                            else {
                                HeapValue::LazyIter(iter)
                            }
                        },
                        HeapValue::NativeFunction(f) => {
                            let iter = Box::new(ZipIterNative::new(iters, heap_iters, f, self));
                            if f.return_is_heap {
                                HeapValue::LazyIterHeap(iter)
                            }
                            else {
                                HeapValue::LazyIter(iter)
                            }
                        },
                        HeapValue::TypeDef(td) => {
                            let iter = Box::new(ZipIterTypeDef::new(iters, heap_iters, td, self));
                            HeapValue::LazyIterHeap(iter)
                        }
                        _ => unreachable!()
                    };
                    self.heap_stack.push(zip_iter);
                },
            }
        }
    }

    pub fn interpret(&mut self, source: &str) -> Result<TaggedValue, InterpreterError> {
        let (function, return_type) = 
            compiler::compile(source, self.typecontext.clone())
            .map_err(|e| InterpreterError::CompileError(e))?
            ;
        let function = Rc::new(function);
        self.init(function);
        self.call().map_err(|e| {
            // in case of error, clean up before returning
            self.stack.clear();
            self.frames.clear();
            e
        })?;
        let result = if return_type.is_heap() {
            ReturnValue::HeapValue(self.heap_stack.pop().unwrap())
        }
        else {
            ReturnValue::Value(self.stack.pop().unwrap())
        };
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
        (HeapValue::Array(arr), ast::Type::Arr(typ)) => {
            Ok(TaggedValue::from_array(&arr, typ.as_ref()))
        },
        (HeapValue::LazyIter(iter), ast::Type::Iter(typ)) => {
            let arr = iter.clone().into_array();
            unpack_heapvalue(HeapValue::Array(arr), &ast::Type::Arr(typ.clone()))
        },
        (HeapValue::LazyIterHeap(iter), ast::Type::Iter(typ)) => {
            let arr = iter.clone().into_array();
            unpack_heapvalue(HeapValue::ArrayHeap(arr), &ast::Type::Arr(typ.clone()))
        },
        (HeapValue::String(s), ast::Type::Str) => {
            Ok(TaggedValue::Str(s.as_ref().clone()))
        },
        (HeapValue::Maybe(x), ast::Type::Maybe(typ)) => {
            TaggedValue::from_maybe(x, typ.as_ref())
        },
        (HeapValue::MaybeHeap(x), ast::Type::Maybe(typ)) => {
            match x {
                Some(x) => Ok(TaggedValue::Maybe(Some(Box::new(unpack_heapvalue(*x, typ.as_ref())?)))),
                None => Ok(TaggedValue::Maybe(None)),
            }
        },
        (HeapValue::ArrayHeap(arr), ast::Type::Arr(typ)) => {
            match typ.as_ref() {
                ast::Type::Str => {
                    let mut arr_s = Vec::new();
                    for inner in arr.iter() {
                        let s = match inner {
                            HeapValue::String(s) => s,
                            _ => unreachable!()
                        };
                        arr_s.push(TaggedValue::Str(s.as_ref().clone()));
                    }
                    Ok(TaggedValue::Arr(arr_s))
                },
                typ => {
                    let mut arr_arr = Vec::new();
                    for inner_arr in arr.iter().cloned() {
                        arr_arr.push(unpack_heapvalue(inner_arr, typ)?);
                    }
                    Ok(TaggedValue::Arr(arr_arr))
                }
            }
        },
        (HeapValue::Closure(f), ast::Type::Func(..)) => {
            Ok(TaggedValue::Closure(f))
        },
        (HeapValue::TypeDef(t), _) => {
            Ok(TaggedValue::TypeDef(t))
        },
        (HeapValue::Object(obj), ast::Type::Object(name, fields)) => {
            let heap_fields = FxHashMap::from_iter(fields.iter().filter(|(_, v)| {
                v.is_heap()
            }).cloned());
            let nonheap_fields = FxHashMap::from_iter(fields.iter().filter(|(_, v)| {
                !v.is_heap()
            }).cloned());
            let mut fields = FxHashMap::default();
            for (n, v) in obj.fields.iter() {
                let t = nonheap_fields.get(n).unwrap();
                let x = TaggedValue::from_value(*v, t)?;
                fields.insert(n.clone(), x);
            }
            for (n, v) in obj.heap_fields.iter() {
                let t = heap_fields.get(n).unwrap();
                let x = unpack_heapvalue(v.clone(), t)?;
                fields.insert(n.clone(), x);
            }

            Ok(TaggedValue::Object(name.clone(), fields))
        },
        (x, rt) => return Err(
            format!(
                "Got unexpected return value: {:?}; expected {:?}",
                x, rt
            )
        ),
    }
}