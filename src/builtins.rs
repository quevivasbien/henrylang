use rustc_hash::FxHashMap;
use std::time::UNIX_EPOCH;

use lazy_static::lazy_static;

use crate::ast::Type;
use crate::values::{HeapValue, NativeFunction, Value};

lazy_static! {
    static ref PRINT: NativeFunction = NativeFunction {
        name: "print",
        arity: 0,
        heap_arity: 1,
        return_is_heap: true,
        function: |vm, _args, heap_args| {
            Ok(match &heap_args[0] {
                HeapValue::String(x) => {
                    println!("{}", x);
                    vm.heap_stack.push(HeapValue::String(x.clone()));
                },
                _ => unreachable!()
            })
        }
    };
    static ref TIME: NativeFunction = NativeFunction {
        name: "time",
        arity: 0,
        heap_arity: 0,
        return_is_heap: false,
        function: |vm, _args, _heap_args| {
            let now = UNIX_EPOCH.elapsed().unwrap().as_micros();
            vm.stack.push(Value { i: now as i64 });
            Ok(())
        }
    };

    static ref ITOF: NativeFunction = NativeFunction {
        name: "itof",
        arity: 1,
        heap_arity: 0,
        return_is_heap: false,
        function: |vm, args, _heap_args| {
            vm.stack.push(unsafe { Value { f: (args[0].i as f64) } });
            Ok(())
        }
    };
    static ref FTOI: NativeFunction = NativeFunction {
        name: "ftoi",
        arity: 1,
        heap_arity: 0,
        return_is_heap: false,
        function: |vm, args, _heap_args| {
            vm.stack.push(unsafe { Value { i: (args[0].f as i64) } });
            Ok(())
        }
    };

    static ref MOD: NativeFunction = NativeFunction {
        name: "mod",
        arity: 2,
        heap_arity: 0,
        return_is_heap: false,
        function: |vm, args, _heap_args| {
            vm.stack.push(unsafe { Value { i: (args[0].i.rem_euclid(args[1].i)) } });
            Ok(())
        }
    };
    static ref POWI: NativeFunction = NativeFunction {
        name: "powi",
        arity: 2,
        heap_arity: 0,
        return_is_heap: false,
        function: |vm, args, _heap_args| {
            vm.stack.push(unsafe { Value { i: (args[0].i.pow(args[1].i as u32)) } });
            Ok(())
        }
    };
    static ref POWF: NativeFunction = NativeFunction {
        name: "powf",
        arity: 2,
        heap_arity: 0,
        return_is_heap: false,
        function: |vm, args, _heap_args| {
            vm.stack.push(unsafe { Value { f: (args[0].f.powf(args[1].f)) } });
            Ok(())
        }
    };

    static ref SUMI: NativeFunction = NativeFunction {
        name: "sumi",
        arity: 0,
        heap_arity: 1,
        return_is_heap: false,
        function: |vm, _args, heap_args| {
            Ok(match &heap_args[0] {
                HeapValue::LazyIter(iter) => unsafe {
                    vm.stack.push(Value { i: iter.clone().into_iter().map(|x| x.i).sum() });
                },
                _ => unreachable!()
            })
        }
    };
    static ref PRODI: NativeFunction = NativeFunction {
        name: "prodi",
        arity: 0,
        heap_arity: 1,
        return_is_heap: false,
        function: |vm, _args, heap_args| {
            Ok(match &heap_args[0] {
                HeapValue::LazyIter(iter) => unsafe {
                    vm.stack.push(Value { i: iter.clone().into_iter().map(|x| x.i).product() });
                },
                _ => unreachable!()
            })
        }
    };

    static ref SUMF: NativeFunction = NativeFunction {
        name: "sumf",
        arity: 0,
        heap_arity: 1,
        return_is_heap: false,
        function: |vm, _args, heap_args| {
            Ok(match &heap_args[0] {
                HeapValue::LazyIter(iter) => unsafe {
                    vm.stack.push(Value { f: iter.clone().into_iter().map(|x| x.f).sum() });
                },
                _ => unreachable!()
            })
        }
    };
    static ref PRODF: NativeFunction = NativeFunction {
        name: "prodf",
        arity: 0,
        heap_arity: 1,
        return_is_heap: false,
        function: |vm, _args, heap_args| {
            Ok(match &heap_args[0] {
                HeapValue::LazyIter(iter) => unsafe {
                    vm.stack.push(Value { f: iter.clone().into_iter().map(|x| x.f).product() });
                },
                _ => unreachable!()
            })
        }
    };

    static ref ALL: NativeFunction = NativeFunction {
        name: "all",
        arity: 0,
        heap_arity: 1,
        return_is_heap: false,
        function: |vm, _args, heap_args| {
            match &heap_args[0] {
                HeapValue::LazyIter(iter) => unsafe {
                    let v = iter.clone().into_iter().all(|x| x.b);
                    vm.stack.push(Value::from_bool(v));
                    Ok(())
                }
                _ => unreachable!()
            }
        }
    };
    static ref ANY: NativeFunction = NativeFunction {
        name: "any",
        arity: 0,
        heap_arity: 1,
        return_is_heap: false,
        function: |vm, _args, heap_args| {
            match &heap_args[0] {
                // HeapValue::Array(arr) => unsafe {
                //     vm.stack.push(Value { b: arr.into_iter().any(|x| x.b) });
                // },
                HeapValue::LazyIter(iter) => unsafe {
                    let v = iter.clone().into_iter().any(|x| x.b);
                    vm.stack.push(Value::from_bool(v));
                    Ok(())
                }
                _ => unreachable!()
            }
        }
    };
}

pub fn builtin_types() -> FxHashMap<String, Type> {
    let mut map = FxHashMap::default();
    map.insert("print".to_string(), Type::Function(vec![Type::String], Box::new(Type::String)));
    map.insert("time".to_string(), Type::Function(vec![], Box::new(Type::Int)));
    map.insert("itof".to_string(), Type::Function(vec![Type::Int], Box::new(Type::Float)));
    map.insert("ftoi".to_string(), Type::Function(vec![Type::Float], Box::new(Type::Int)));

    map.insert("mod".to_string(), Type::Function(vec![Type::Int, Type::Int], Box::new(Type::Int)));
    map.insert("powi".to_string(), Type::Function(vec![Type::Int, Type::Int], Box::new(Type::Int)));
    map.insert("powf".to_string(), Type::Function(vec![Type::Float, Type::Float], Box::new(Type::Float)));

    map.insert("sumi".to_string(), Type::Function(vec![Type::Iterator(Box::new(Type::Int))], Box::new(Type::Int)));
    map.insert("prodi".to_string(), Type::Function(vec![Type::Iterator(Box::new(Type::Int))], Box::new(Type::Int)));

    map.insert("sumf".to_string(), Type::Function(vec![Type::Iterator(Box::new(Type::Float))], Box::new(Type::Float)));
    map.insert("prodf".to_string(), Type::Function(vec![Type::Iterator(Box::new(Type::Float))], Box::new(Type::Float)));

    map.insert("all".to_string(), Type::Function(vec![Type::Iterator(Box::new(Type::Bool))], Box::new(Type::Bool)));
    map.insert("any".to_string(), Type::Function(vec![Type::Iterator(Box::new(Type::Bool))], Box::new(Type::Bool)));
    
    map.insert("E".to_string(), Type::Float);

    map
}

pub fn builtins() -> FxHashMap<String, Value> {
    let mut map = FxHashMap::default();
    map.insert("E".to_string(), Value { f: std::f64::consts::E });

    map
}

pub fn heap_builtins() -> FxHashMap<String, HeapValue> {
    let mut map = FxHashMap::default();
    map.insert("print".to_string(), HeapValue::NativeFunction(&PRINT));
    map.insert("time".to_string(), HeapValue::NativeFunction(&TIME));
    map.insert("itof".to_string(), HeapValue::NativeFunction(&ITOF));
    map.insert("ftoi".to_string(), HeapValue::NativeFunction(&FTOI));

    map.insert("mod".to_string(), HeapValue::NativeFunction(&MOD));
    map.insert("powi".to_string(), HeapValue::NativeFunction(&POWI));
    map.insert("powf".to_string(), HeapValue::NativeFunction(&POWF));

    map.insert("sumi".to_string(), HeapValue::NativeFunction(&SUMI));
    map.insert("prodi".to_string(), HeapValue::NativeFunction(&PRODI));

    map.insert("sumf".to_string(), HeapValue::NativeFunction(&SUMF));
    map.insert("prodf".to_string(), HeapValue::NativeFunction(&PRODF));

    map.insert("all".to_string(), HeapValue::NativeFunction(&ALL));
    map.insert("any".to_string(), HeapValue::NativeFunction(&ANY));

    map
}
