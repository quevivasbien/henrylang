use std::collections::HashMap;

use lazy_static::lazy_static;

use crate::{NativeFunction, Value};

lazy_static! {
    static ref PRINT: NativeFunction = NativeFunction {
        name: "print",
        arity: 1,
        function: |args| {
            println!("{}", args[0]);
            Ok(Value::String(format!("{}", args[0])))
        }
    };

    static ref MAX: NativeFunction = NativeFunction {
        name: "max",
        arity: 1,
        function: |args| {
            let list = match &args[0] {
                Value::Array(x) => x,
                _ => return Err("Cannot call max on non-array type"),
            };
            list.iter().cloned().max_by(
                |x, y| match x.clone().gt(y.clone()) {
                    Ok(Value::Bool(true)) => std::cmp::Ordering::Greater,
                    Ok(Value::Bool(false)) => std::cmp::Ordering::Less,
                    _ => std::cmp::Ordering::Equal,
                }
            ).ok_or("Cannot call max on empty array")
        }
    };

    static ref MIN: NativeFunction = NativeFunction {
        name: "min",
        arity: 1,
        function: |args| {
            let list = match &args[0] {
                Value::Array(x) => x,
                _ => return Err("Cannot call max on non-array type"),
            };
            list.iter().cloned().min_by(
                |x, y| match x.clone().gt(y.clone()) {
                    Ok(Value::Bool(true)) => std::cmp::Ordering::Greater,
                    Ok(Value::Bool(false)) => std::cmp::Ordering::Less,
                    _ => std::cmp::Ordering::Equal,
                }
            ).ok_or("Cannot call min on empty array")
        }
    };

    static ref INT: NativeFunction = NativeFunction {
        name: "int",
        arity: 1,
        function: |args| {
            let i = match &args[0] {
                Value::String(x) => match x.parse::<i64>() {
                    Ok(x) => x,
                    Err(_) => return Err("Unable to parse int from string"),
                },
                Value::Int(x) => *x,
                Value::Float(x) => *x as i64,
                _ => return Err("Cannot call int on non-numeric or string type"),
            };
            Ok(Value::Int(i))
        }
    };

    static ref FLOAT: NativeFunction = NativeFunction {
        name: "float",
        arity: 1,
        function: |args| {
            let f = match &args[0] {
                Value::String(x) => match x.parse::<f64>() {
                    Ok(x) => x,
                    Err(_) => return Err("Unable to parse float from string"),
                }
                Value::Int(x) => *x as f64,
                Value::Float(x) => *x,
                _ => return Err("Cannot call float on non-numeric or string type"),
            };
            Ok(Value::Float(f))
        }
    };
}

pub fn builtins() -> HashMap<String, Value> {
    let mut map = HashMap::new();
    map.insert("print".to_string(), Value::NativeFunction(&PRINT));
    map.insert("int".to_string(), Value::NativeFunction(&INT));
    map.insert("float".to_string(), Value::NativeFunction(&FLOAT));
    map.insert("max".to_string(), Value::NativeFunction(&MAX));
    map.insert("min".to_string(), Value::NativeFunction(&MIN));

    map
}