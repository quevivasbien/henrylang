use std::fmt::Debug;
use std::rc::Rc;

use dyn_clone::DynClone;

use crate::VM;

use super::{Closure, HeapValue, NativeFunction, Value};


pub trait LazyIter<T: Clone>: DynClone + Debug {
    fn next(&mut self) -> Option<T>;
    fn into_array(&mut self) -> Rc<[T]> {
        let mut arr = Vec::new();
        while let Some(x) = self.next() {
            arr.push(x);
        }
        Rc::from(arr)
    }
}
dyn_clone::clone_trait_object!(<T> LazyIter<T>);

impl<T: Clone> Iterator for dyn LazyIter<T> {
    type Item = T;
    fn next(&mut self) -> Option<Self::Item> {
        self.next()
    }
}

#[derive(Clone, Debug)]
pub struct ArrayIter<T: Clone + Debug> {
    array: Rc<[T]>,
    idx: usize,
}

impl<T: Clone + Debug> ArrayIter<T> {
    pub fn new(array: Rc<[T]>,) -> Self {
        Self { array, idx: 0 }
    }
}

impl<T: Clone + Debug> LazyIter<T> for ArrayIter<T> {
    fn next(&mut self) -> Option<T> {
        if self.idx < self.array.len() {
            let value = self.array[self.idx].clone();
            self.idx += 1;
            Some(value)
        }
        else {
            None
        }
    }

    fn into_array(&mut self) -> Rc<[T]> {
        self.array.clone()
    }
}

#[derive(Clone, Debug)]
pub struct RangeIter {
    end: i64,
    current: i64,
}

impl RangeIter {
    pub fn new(start: i64, end: i64) -> Self {
        debug_assert!(start <= end);
        Self { end, current: start }
    }
}

impl LazyIter<Value> for RangeIter {
    fn next(&mut self) -> Option<Value> {
        if self.current != self.end {
            let value = self.current;
            self.current += 1;
            Some(Value::from_i64(value))
        }
        else {
            None
        }
    }
}

#[derive(Clone, Debug)]
pub struct ReverseRangeIter {
    end: i64,
    current: i64,
}

impl ReverseRangeIter {
    pub fn new(start: i64, end: i64) -> Self {
        debug_assert!(start >= end);
        Self { end, current: start }
    }
}

impl LazyIter<Value> for ReverseRangeIter {
    fn next(&mut self) -> Option<Value> {
        if self.current != self.end {
            let value = self.current;
            self.current -= 1;
            Some(Value::from_i64(value))
        }
        else {
            None
        }
    }
}

// An iterator that iterates over some input iterator, calls a closure, and yields the closure's result
// The closure must return a Value (have return_is_heap == false) 
#[derive(Clone, Debug)]
pub struct MapIter<T: Debug + Clone> {
    iter: Box<dyn LazyIter<T>>,
    closure: Box<Closure>,
    vm: *mut VM,
}

impl<T: Debug + Clone> MapIter<T> {
    pub fn new(iter: Box<dyn LazyIter<T>>, closure: Box<Closure>, vm: *mut VM) -> MapIter<T> {
        debug_assert!(!closure.function.return_is_heap);
        Self { iter, closure, vm }
    }
}

impl LazyIter<Value> for MapIter<Value> {
    fn next(&mut self) -> Option<Value> {
        match self.iter.next() {
            None => None,
            Some(x) => {
                let vm = unsafe { &mut *self.vm };
                vm.stack.push(x);
                vm.call_function(self.closure.clone()).expect("Unrecoverable error in map iterator");
                Some(vm.stack.pop().expect("Expected result on stack"))
            }
        }
    }
}

impl LazyIter<Value> for MapIter<HeapValue> {
    fn next(&mut self) -> Option<Value> {
        match self.iter.next() {
            None => None,
            Some(x) => {
                let vm = unsafe { &mut *self.vm };
                vm.heap_stack.push(x);
                vm.call_function(self.closure.clone()).expect("Unrecoverable error in map iterator");
                Some(vm.stack.pop().expect("Expected result on stack"))
            }
        }
    }
}


// An iterator that iterates over some input iterator, calls a closure, and yields the closure's result
// The closure must return a HeapValue (have return_is_heap == true) 
#[derive(Clone, Debug)]
pub struct MapIterHeap<T: Debug + Clone> {
    iter: Box<dyn LazyIter<T>>,
    closure: Box<Closure>,
    vm: *mut VM,
}

impl<T: Debug + Clone> MapIterHeap<T> {
    pub fn new(iter: Box<dyn LazyIter<T>>, closure: Box<Closure>, vm: *mut VM) -> MapIterHeap<T> {
        debug_assert!(closure.function.return_is_heap);
        Self { iter, closure, vm }
    }
}

impl LazyIter<HeapValue> for MapIterHeap<Value> {
    fn next(&mut self) -> Option<HeapValue> {
        match self.iter.next() {
            None => None,
            Some(x) => {
                let vm = unsafe { &mut *self.vm };
                vm.stack.push(x);
                vm.call_function(self.closure.clone()).expect("Unrecoverable error in map iterator");
                Some(vm.heap_stack.pop().expect("Expected result on heap stack"))
            }
        }
    }
}

impl LazyIter<HeapValue> for MapIterHeap<HeapValue> {
    fn next(&mut self) -> Option<HeapValue> {
        match self.iter.next() {
            None => None,
            Some(x) => {
                let vm = unsafe { &mut *self.vm };
                vm.heap_stack.push(x);
                vm.call_function(self.closure.clone()).expect("Unrecoverable error in map iterator");
                Some(vm.heap_stack.pop().expect("Expected result on heap stack"))
            }
        }
    }
}


// same idea as MapIter, but for calling NativeFunctions
#[derive(Clone, Debug)]
pub struct MapIterNative<T: Debug + Clone> {
    iter: Box<dyn LazyIter<T>>,
    function: &'static NativeFunction,
    vm: *mut VM,
}

impl<T: Debug + Clone> MapIterNative<T> {
    pub fn new(iter: Box<dyn LazyIter<T>>, function: &'static NativeFunction, vm: *mut VM) -> MapIterNative<T> {
        debug_assert!(!function.return_is_heap);
        Self { iter, function, vm }
    }
}

impl LazyIter<Value> for MapIterNative<Value> {
    fn next(&mut self) -> Option<Value> {
        match self.iter.next() {
            None => None,
            Some(x) => {
                let vm = unsafe { &mut *self.vm };
                vm.stack.push(x);
                vm.call_native_function(self.function).expect("Unrecoverable error in map iterator");
                Some(vm.stack.pop().expect("Expected result on stack"))
            }
        }
    }
}

impl LazyIter<Value> for MapIterNative<HeapValue> {
    fn next(&mut self) -> Option<Value> {
        match self.iter.next() {
            None => None,
            Some(x) => {
                let vm = unsafe { &mut *self.vm };
                vm.heap_stack.push(x);
                vm.call_native_function(self.function).expect("Unrecoverable error in map iterator");
                Some(vm.stack.pop().expect("Expected result on stack"))
            }
        }
    }
}


#[derive(Clone, Debug)]
pub struct MapIterNativeHeap<T: Debug + Clone> {
    iter: Box<dyn LazyIter<T>>,
    function: &'static NativeFunction,
    vm: *mut VM,
}

impl<T: Debug + Clone> MapIterNativeHeap<T> {
    pub fn new(iter: Box<dyn LazyIter<T>>, function: &'static NativeFunction, vm: *mut VM) -> MapIterNativeHeap<T> {
        debug_assert!(function.return_is_heap);
        Self { iter, function, vm }
    }
}

impl LazyIter<HeapValue> for MapIterNativeHeap<Value> {
    fn next(&mut self) -> Option<HeapValue> {
        match self.iter.next() {
            None => None,
            Some(x) => {
                let vm = unsafe { &mut *self.vm };
                vm.stack.push(x);
                vm.call_native_function(self.function).expect("Unrecoverable error in map iterator");
                Some(vm.heap_stack.pop().expect("Expected result on stack"))
            }
        }
    }
}

impl LazyIter<HeapValue> for MapIterNativeHeap<HeapValue> {
    fn next(&mut self) -> Option<HeapValue> {
        match self.iter.next() {
            None => None,
            Some(x) => {
                let vm = unsafe { &mut *self.vm };
                vm.heap_stack.push(x);
                vm.call_native_function(self.function).expect("Unrecoverable error in map iterator");
                Some(vm.heap_stack.pop().expect("Expected result on stack"))
            }
        }
    }
}
