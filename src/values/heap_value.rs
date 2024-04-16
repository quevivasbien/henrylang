use std::rc::Rc;

use super::{Closure, NativeFunction, LazyIter, Object, TypeDef, Value};


#[derive(Debug, Clone)]
pub enum HeapValue {
    String(Rc<String>),
    Array(Rc<[Value]>),
    ArrayHeap(Rc<[HeapValue]>),
    Maybe(Option<Value>),
    MaybeHeap(Option<Box<HeapValue>>),
    Closure(Box<Closure>),
    NativeFunction(&'static NativeFunction),
    TypeDef(Rc<TypeDef>),
    Object(Rc<Object>),
    LazyIter(Box<dyn LazyIter<Value>>),
    LazyIterHeap(Box<dyn LazyIter<HeapValue>>),
}

impl PartialEq for HeapValue {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (HeapValue::String(l), HeapValue::String(r)) => l == r,
            (HeapValue::Array(l), HeapValue::Array(r)) => l == r,
            (HeapValue::ArrayHeap(l), HeapValue::ArrayHeap(r)) => l == r,
            (HeapValue::Maybe(l), HeapValue::Maybe(r)) => l == r,
            (HeapValue::Closure(l), HeapValue::Closure(r)) => std::ptr::eq(l.function.as_ref(), r.function.as_ref()),
            (HeapValue::NativeFunction(l), HeapValue::NativeFunction(r)) => std::ptr::eq(l, r),
            _ => false
        }
    }
}
