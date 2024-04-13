use std::collections::HashMap;
use std::fmt::{Debug, Display};
use std::rc::Rc;

use crate::ast;
use crate::vm::{InterpreterError, VM};
use crate::chunk::Chunk;

pub struct Function {
    pub name: String,
    pub num_upvalues: u16,
    pub num_heap_upvalues: u16,
    pub arity: u8,
    pub heap_arity: u8,
    pub chunk: Chunk,
}

impl Default for Function {
    fn default() -> Self {
        Self { name: String::new(), num_upvalues: 0, num_heap_upvalues: 0, arity: 0, heap_arity: 0, chunk: Chunk::new() }
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
    pub function: fn(&mut VM, &[Value]) -> Result<Value, InterpreterError>,
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

pub struct TypeDef {
    pub name: String,
    pub fieldnames: Vec<String>,
}

impl Debug for TypeDef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.name.is_empty() {
            write!(f, "{{ {:?} }}", self.fieldnames)
        }
        else {
            write!(f, "{} {{ {:?} }}", self.name, self.fieldnames)
        }
    }
}

impl Display for TypeDef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.name.is_empty() {
            write!(f, "<anontype> {{")?;
        }
        else {
            write!(f, "{} {{", self.name)?;
        }
        for field in &self.fieldnames {
            write!(f, " {}", field)?;
        }
        write!(f, " }}")
    }
}

impl TypeDef {
    pub fn new(name: String, fieldnames: Vec<String>) -> Self {
        Self { name, fieldnames }
    }
}

pub struct Object {
    pub typedef: Rc<TypeDef>,
    pub fields: HashMap<String, Value>,
}

// impl Debug for Object {
//     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
//         if self.typedef.name.is_empty() {
//             write!(f, "{:?}", self.fields)
//         }
//         else {
//             write!(f, "{} {:?}", self.typedef, self.fields)
//         }
//     }
// }

// impl Display for Object {
//     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
//         if self.typedef.name.is_empty() {
//             write!(f, "{{")?;
//         }
//         else {
//             write!(f, "{} {{", self.typedef.name)?;
//         }
//         for field in &self.typedef.fieldnames {
//             write!(f, " {}: {}", field, self.fields.get(field).unwrap())?;
//         }
//         write!(f, " }}")
//     }
// }

impl Object {
    pub fn new(typedef: Rc<TypeDef>, fields: HashMap<String, Value>) -> Self {
        Self { typedef, fields }
    }
}

#[derive(Copy, Clone)]
pub union Value {
    pub i: i64,
    pub f: f64,
    pub b: bool,
    pub p: *const Rc<[Value]>, // pointer to an value stored in local simulated heap
    pub null: (),
}

impl Value {
    pub fn from_i64(i: i64) -> Self {
        Self { i }
    }
    pub fn from_f64(f: f64) -> Self {
        Self { f }
    }
    pub fn from_bool(b: bool) -> Self {
        Self { b }
    }
    pub fn from_none() -> Self {
        Self { null: () }
    }
}

impl std::fmt::Debug for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let constant = unsafe {
            std::mem::transmute::<&Self, &u64>(self)
        };
        write!(f, "{:#x?}", constant)
    }
}

#[derive(Debug, Clone)]
pub enum HeapValue {
    String(Rc<String>),
    Array(Rc<[Value]>),
    ArrayHeap(Rc<[HeapValue]>),
    Maybe(Option<Value>),
    MaybeHeap(Option<Box<HeapValue>>),
    Closure(Box<Closure>),
}

#[derive(Debug)]
pub enum TaggedValue {
    Int(i64),
    Float(f64),
    Bool(bool),
    String(String),
    Array(Vec<TaggedValue>),
    Maybe(Option<Box<TaggedValue>>),
    Closure(Box<Closure>),
}

impl Display for TaggedValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TaggedValue::Int(i) => write!(f, "{}", i),
            TaggedValue::Float(fl) => write!(f, "{:.}", fl),
            TaggedValue::Bool(b) => write!(f, "{}", b),
            TaggedValue::String(s) => write!(f, "{}", s),
            TaggedValue::Array(arr) => {
                write!(f, "[")?;
                for (i, v) in arr.iter().enumerate() {
                    if i != 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", v)?;
                }
                write!(f, "]")
            },
            TaggedValue::Maybe(maybe) => {
                match maybe {
                    Some(v) => write!(f, "Some({})", v),
                    None => write!(f, "Null"),
                }
            },
            TaggedValue::Closure(closure) => write!(f, "{}", closure),
        }
    }
}

impl TaggedValue {
    pub fn from_value(value: Value, typ: &ast::Type) -> Result<TaggedValue, String> {
        Ok(match typ {
            ast::Type::Int => TaggedValue::Int(unsafe { value.i }),
            ast::Type::Float => TaggedValue::Float(unsafe { value.f }),
            ast::Type::Bool => TaggedValue::Bool(unsafe { value.b }),
            t => return Err(
                format!(
                    "Cannot extract value from type {:?}", t
                )
            ),
        })
    }
    pub fn from_array(arr: &[Value], typ: &ast::Type) -> TaggedValue {
        TaggedValue::Array(arr.iter().map(|x| {
            match typ {
                ast::Type::Int => TaggedValue::Int(unsafe { x.i }),
                ast::Type::Float => TaggedValue::Float(unsafe { x.f }),
                ast::Type::Bool => TaggedValue::Bool(unsafe { x.b }),
                _ => unimplemented!(),
            }
        }).collect())
    }
    pub fn from_maybe(maybe: Option<Value>, typ: &ast::Type) -> Result<TaggedValue, String> {
        Ok(match maybe {
            Some(x) => TaggedValue::Maybe(Some(Box::new(TaggedValue::from_value(x, typ)?))),
            None => TaggedValue::Maybe(None),
        })
    }
}
