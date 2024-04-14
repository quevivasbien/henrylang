use std::collections::HashMap;

use lazy_static::lazy_static;

use crate::ast::Type;
use crate::values::{HeapValue, NativeFunction, ReturnValue, Value};

lazy_static! {
    static ref PRINT: NativeFunction = NativeFunction {
        name: "print",
        arity: 0,
        heap_arity: 1,
        return_is_heap: true,
        function: |_vm, _args, heap_args| {
            Ok(match &heap_args[0] {
                HeapValue::String(x) => {
                    println!("{}", x);
                    ReturnValue::HeapValue(HeapValue::String(x.clone()))
                },
                _ => unreachable!()
            })
        }
    };

    static ref ITOF: NativeFunction = NativeFunction {
        name: "itof",
        arity: 1,
        heap_arity: 0,
        return_is_heap: false,
        function: |_vm, args, _heap_args| {
            Ok(ReturnValue::Value(unsafe { Value { f: (args[0].i as f64) } }))
        }
    };
    static ref FTOI: NativeFunction = NativeFunction {
        name: "ftoi",
        arity: 1,
        heap_arity: 0,
        return_is_heap: false,
        function: |_vm, args, _heap_args| {
            Ok(ReturnValue::Value(unsafe { Value { i: (args[0].f as i64) } }))
        }
    };

    static ref MOD: NativeFunction = NativeFunction {
        name: "mod",
        arity: 2,
        heap_arity: 0,
        return_is_heap: false,
        function: |_vm, args, _heap_args| {
            Ok(ReturnValue::Value(unsafe { Value { i: (args[0].i.rem_euclid(args[1].i)) } }))
        }
    };
    static ref POWI: NativeFunction = NativeFunction {
        name: "powi",
        arity: 2,
        heap_arity: 0,
        return_is_heap: false,
        function: |_vm, args, _heap_args| {
            Ok(ReturnValue::Value(unsafe { Value { i: (args[0].i.pow(args[1].i as u32)) } }))
        }
    };
    static ref POWF: NativeFunction = NativeFunction {
        name: "powf",
        arity: 2,
        heap_arity: 0,
        return_is_heap: false,
        function: |_vm, args, _heap_args| {
            Ok(ReturnValue::Value(unsafe { Value { f: (args[0].f.powf(args[1].f)) } }))
        }
    };

    static ref SUMI: NativeFunction = NativeFunction {
        name: "sumi",
        arity: 0,
        heap_arity: 1,
        return_is_heap: false,
        function: |_vm, _args, heap_args| {
            Ok(match &heap_args[0] {
                HeapValue::Array(arr) => unsafe {
                    ReturnValue::Value(Value { i: arr.into_iter().map(|x| x.i).sum() })
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
        function: |_vm, _args, heap_args| {
            Ok(match &heap_args[0] {
                HeapValue::Array(arr) => unsafe {
                    ReturnValue::Value(Value { i: arr.into_iter().map(|x| x.i).product() })
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
        function: |_vm, _args, heap_args| {
            Ok(match &heap_args[0] {
                HeapValue::Array(arr) => unsafe {
                    ReturnValue::Value(Value { f: arr.into_iter().map(|x| x.f).sum() })
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
        function: |_vm, _args, heap_args| {
            Ok(match &heap_args[0] {
                HeapValue::Array(arr) => unsafe {
                    ReturnValue::Value(Value { f: arr.into_iter().map(|x| x.f).product() })
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
        function: |_vm, _args, heap_args| {
            Ok(match &heap_args[0] {
                HeapValue::Array(arr) => unsafe {
                    ReturnValue::Value(Value { b: arr.into_iter().all(|x| x.b) })
                },
                _ => unreachable!()
            })
        }
    };
    static ref ANY: NativeFunction = NativeFunction {
        name: "any",
        arity: 0,
        heap_arity: 1,
        return_is_heap: false,
        function: |_vm, _args, heap_args| {
            Ok(match &heap_args[0] {
                HeapValue::Array(arr) => unsafe {
                    ReturnValue::Value(Value { b: arr.into_iter().any(|x| x.b) })
                },
                _ => unreachable!()
            })
        }
    };
}

pub fn builtin_types() -> HashMap<String, Type> {
    let mut map = HashMap::new();
    map.insert("print".to_string(), Type::Function(vec![Type::String], Box::new(Type::String)));
    map.insert("itof".to_string(), Type::Function(vec![Type::Int], Box::new(Type::Float)));
    map.insert("ftoi".to_string(), Type::Function(vec![Type::Float], Box::new(Type::Int)));

    map.insert("mod".to_string(), Type::Function(vec![Type::Int, Type::Int], Box::new(Type::Int)));
    map.insert("powi".to_string(), Type::Function(vec![Type::Int, Type::Int], Box::new(Type::Int)));
    map.insert("powf".to_string(), Type::Function(vec![Type::Float, Type::Float], Box::new(Type::Float)));

    map.insert("sumi".to_string(), Type::Function(vec![Type::Array(Box::new(Type::Int))], Box::new(Type::Int)));
    map.insert("prodi".to_string(), Type::Function(vec![Type::Array(Box::new(Type::Int))], Box::new(Type::Int)));

    map.insert("sumf".to_string(), Type::Function(vec![Type::Array(Box::new(Type::Float))], Box::new(Type::Float)));
    map.insert("prodf".to_string(), Type::Function(vec![Type::Array(Box::new(Type::Float))], Box::new(Type::Float)));

    map.insert("all".to_string(), Type::Function(vec![Type::Array(Box::new(Type::Bool))], Box::new(Type::Bool)));
    map.insert("any".to_string(), Type::Function(vec![Type::Array(Box::new(Type::Bool))], Box::new(Type::Bool)));
    
    map.insert("E".to_string(), Type::Float);

    map
}

pub fn builtins() -> HashMap<String, Value> {
    let mut map = HashMap::new();
    map.insert("E".to_string(), Value { f: std::f64::consts::E });

    map
}

pub fn heap_builtins() -> HashMap<String, HeapValue> {
    let mut map = HashMap::new();
    map.insert("print".to_string(), HeapValue::NativeFunction(&PRINT));
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
