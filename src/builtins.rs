use std::collections::HashMap;
use std::rc::Rc;

use lazy_static::lazy_static;

use crate::{NativeFunction, Value};

lazy_static! {
    static ref PRINT: NativeFunction = NativeFunction {
        name: "print",
        arity: 1,
        function: |_vm, args| {
            println!("{}", args[0]);
            Ok(Value::String(Rc::new(format!("{}", args[0]))))
        }
    };

    static ref INT: NativeFunction = NativeFunction {
        name: "int",
        arity: 1,
        function: |vm, args| {
            let i = match &args[0] {
                Value::String(x) => match x.parse::<i64>() {
                    Ok(x) => x,
                    Err(_) => return Err(vm.runtime_err(
                        format!("Unable to parse int from string \"{}\"", x)
                    )),
                },
                Value::Int(x) => *x,
                Value::Float(x) => *x as i64,
                x => return Err(vm.runtime_err(
                    format!("Cannot call int on non-numeric or string type, got {:?}", x)
                )),
            };
            Ok(Value::Int(i))
        }
    };

    static ref FLOAT: NativeFunction = NativeFunction {
        name: "float",
        arity: 1,
        function: |vm, args| {
            let f = match &args[0] {
                Value::String(x) => match x.parse::<f64>() {
                    Ok(x) => x,
                    Err(_) => return Err(vm.runtime_err(
                        format!("Unable to parse float from string \"{}\"", x)
                    )),
                }
                Value::Int(x) => *x as f64,
                Value::Float(x) => *x,
                x => return Err(vm.runtime_err(
                    format!("Cannot call float on non-numeric type, got {:?}", x)
                )),
            };
            Ok(Value::Float(f))
        }
    };

    static ref STRING: NativeFunction = NativeFunction {
        name: "string",
        arity: 1,
        function: |_vm, args| {
            Ok(Value::String(match &args[0] {
                Value::String(x) => x.clone(),
                Value::Int(x) => Rc::new(x.to_string()),
                Value::Float(x) => Rc::new(x.to_string()),
                Value::Bool(x) => Rc::new(x.to_string()),
                x => Rc::new(format!("{}", x)),
            }))
        }
    };

    static ref LEN: NativeFunction = NativeFunction {
        name: "len",
        arity: 1,
        function: |vm, args| {
            let length = match &args[0] {
                Value::Array(x) => x.len(),
                Value::String(x) => x.len(),
                x => return Err(vm.runtime_err(
                    format!("Cannot call len on non-array or string type, got {:?}", x)
                )),
            };
            Ok(Value::Int(length as i64))
        }
    };

    static ref MAX: NativeFunction = NativeFunction {
        name: "max",
        arity: 1,
        function: |vm, args| {
            let list = match &args[0] {
                Value::Array(x) => x,
                x => return Err(vm.runtime_err(
                    format!("Cannot call max on non-array type, got {:?}", x)
                )),
            };
            list.iter().cloned().max_by(
                |x, y| match x.clone().gt(y.clone()) {
                    Ok(Value::Bool(true)) => std::cmp::Ordering::Greater,
                    Ok(Value::Bool(false)) => std::cmp::Ordering::Less,
                    _ => std::cmp::Ordering::Equal,
                }
            ).ok_or(vm.runtime_err("Cannot call max on empty array".to_string()))
        }
    };

    static ref MIN: NativeFunction = NativeFunction {
        name: "min",
        arity: 1,
        function: |vm, args| {
            let list = match &args[0] {
                Value::Array(x) => x,
                x => return Err(vm.runtime_err(
                    format!("Cannot call min on non-array type, got {:?}", x)
                )),
            };
            list.iter().cloned().min_by(
                |x, y| match x.clone().gt(y.clone()) {
                    Ok(Value::Bool(true)) => std::cmp::Ordering::Greater,
                    Ok(Value::Bool(false)) => std::cmp::Ordering::Less,
                    _ => std::cmp::Ordering::Equal,
                }
            ).ok_or(vm.runtime_err("Cannot call min on empty array".to_string()))
        }
    };

    static ref SUM: NativeFunction = NativeFunction {
        name: "sum",
        arity: 1,
        function: |vm, args| {
            let list = match &args[0] {
                Value::Array(x) => x,
                x => return Err(vm.runtime_err(
                    format!("Cannot call sum on non-array type, got {:?}", x)
                )),
            };
            if list.is_empty() {
                return Err(vm.runtime_err("Cannot call sum on empty array".to_string()));
            }
            list.iter().skip(1).cloned().try_fold(
                list[0].clone(),
                |acc, x| acc + x
            ).map_err(|e| vm.runtime_err(e))
        }
    };

    static ref PROD: NativeFunction = NativeFunction {
        name: "prod",
        arity: 1,
        function: |vm, args| {
            let list = match &args[0] {
                Value::Array(x) => x,
                x => return Err(vm.runtime_err( 
                    format!("Cannot call prod on non-array type, got {:?}", x)
                )),
            };
            if list.is_empty() {
                return Err(vm.runtime_err("Cannot call prod on empty array".to_string()));
            }
            list.iter().skip(1).cloned().try_fold(
                list[0].clone(),
                |acc, x| acc * x
            ).map_err(|e| vm.runtime_err(e))
        }
    };

    static ref ZIP: NativeFunction = NativeFunction {
        name: "zip",
        arity: 2,
        function: |vm, args| {
            let (left, right) = match (&args[0], &args[1]) {
                (Value::Array(x), Value::Array(y)) => (x, y),
                x => return Err(vm.runtime_err(
                    format!("Cannot call zip on non-array type, got {:?}", x)
                )),
            };
            let zipped = left.iter().zip(right.iter()).map(|(x, y)| Value::Array(Rc::new(vec![x.clone(), y.clone()]))).collect::<Vec<Value>>();
            Ok(Value::Array(Rc::new(zipped)))
        }
    };

    static ref FILTER: NativeFunction = NativeFunction {
        name: "filter",
        arity: 2,
        function: |vm, args| {
            let list = match &args[1] {
                Value::Array(x) => x,
                x => return Err(vm.runtime_err(
                    format!("Cannot call filter with non-array type as RHS, got {:?}", x)
                )),
            };
            let mut out = Vec::new();
            match &args[0] {
                Value::Closure(f) => {
                    for value in list.iter() {
                        vm.stack.push(Value::Bool(false));
                        vm.stack.push(value.clone());
                        vm.call_function(1, f.clone())?;
                        let result = vm.stack.pop().expect("Call to predicate resulted in empty stack");
                        match result {
                            Value::Bool(x) => {
                                if x {
                                    out.push(value.clone());
                                }
                            },
                            _ => return Err(vm.runtime_err(
                                format!("In filter, predicate returned a non-boolean value: {}", result)
                            )),
                        }
                    }
                },
                Value::NativeFunction(f) => {
                    for value in list.iter() {
                        vm.stack.push(Value::Bool(false));
                        vm.stack.push(value.clone());
                        vm.call_native_function(1, f)?;
                        let result = vm.stack.pop().expect("Call to predicate resulted in empty stack");
                        match result {
                            Value::Bool(x) => {
                                if x {
                                    out.push(value.clone());
                                }
                            },
                            _ => return Err(vm.runtime_err(
                                format!("In filter, predicate returned a non-boolean value: {}", result)
                            )),
                        }
                    }
                },
                Value::Array(arr) => {
                    for value in list.iter() {
                        vm.stack.push(Value::Bool(false));
                        vm.stack.push(value.clone());
                        vm.array_index(1, arr)?;
                        let result = vm.stack.pop().expect("Call to predicate resulted in empty stack");
                        match result {
                            Value::Bool(x) => {
                                if x {
                                    out.push(value.clone());
                                }
                            },
                            _ => return Err(vm.runtime_err(
                                format!("In filter, predicate returned a non-boolean value: {}", result)
                            )),
                        }
                    }
                },
                x => return Err(vm.runtime_err(
                    format!("Cannot call filter with non-function or array type as LHS, got {:?}", x)
                )),
            };
            Ok(Value::Array(Rc::new(out)))
        } 
    };

    pub static ref MAP: NativeFunction = NativeFunction {
        name: "filter",
        arity: 2,
        function: |vm, args| {
            let right = match &args[1] {
                Value::Array(x) => x,
                x => return Err(vm.runtime_err(
                    format!("Cannot call map with non-array type as RHS, got {:?}", x)
                )),
            };
            let n_elems = right.len();
            match &args[0] {
                Value::Closure(f) => {
                    for value in right.iter() {
                        vm.stack.push(Value::Bool(false)); // just a placeholder
                        vm.stack.push(value.clone());
                        vm.call_function(1, f.clone())?;
                    }
                },
                Value::NativeFunction(f) => {
                    for value in right.iter() {
                        vm.stack.push(Value::Bool(false)); // just a placeholder
                        vm.stack.push(value.clone());
                        vm.call_native_function(1, f)?;
                    }
                },
                Value::Array(arr) => {
                    for value in right.iter() {
                        vm.stack.push(Value::Bool(false)); // just a placeholder
                        vm.stack.push(value.clone());
                        vm.array_index(1, &arr)?;
                    }
                },
                x => return Err(vm.runtime_err(
                    format!("Cannot call map with non-function or array type as LHS, got {:?}", x)
                )),
            };
            let array = Rc::new(
                vm.stack.split_off(vm.stack.len() - n_elems)
            );
            Ok(Value::Array(array))
        }
    };

    static ref REDUCE: NativeFunction = NativeFunction {
        name: "reduce",
        arity: 3,
        function: |vm, args| {
            let list = match &args[1] {
                Value::Array(x) => x,
                x => return Err(vm.runtime_err(
                    format!("Cannot call reduce with non-array type as RHS, got {:?}", x)
                )),
            };
            let mut acc = args[2].clone();
            match &args[0] {
                Value::Closure(c) => {
                    if c.function.arity != 2 {
                        return Err(vm.runtime_err(
                            format!("Reduce function must take two arguments, got {}", c.function.arity)
                        ));
                    }
                    for value in list.iter() {
                        vm.stack.push(Value::Bool(false));
                        vm.stack.push(acc.clone());
                        vm.stack.push(value.clone());
                        vm.call_function(2, c.clone())?;
                        acc = vm.stack.pop().expect("Call to reduce resulted in empty stack");
                    }
                },
                Value::NativeFunction(f) => {
                    if f.arity != 2 {
                        return Err(vm.runtime_err(
                            format!("Reduce function must take two arguments, got {}", f.arity)
                        ));
                    }
                    for value in list.iter() {
                        vm.stack.push(Value::Bool(false));
                        vm.stack.push(acc.clone());
                        vm.stack.push(value.clone());
                        vm.call_native_function(2, f)?;
                        acc = vm.stack.pop().expect("Call to reduce resulted in empty stack");
                    }
                },
                x => return Err(vm.runtime_err(
                    format!("Cannot call reduce with non-function type on LHS, got {:?}", x)
                )),
            }
            Ok(acc)
        }
    };
}

pub fn builtins() -> HashMap<String, Value> {
    let mut map = HashMap::new();
    map.insert("print".to_string(), Value::NativeFunction(&PRINT));
    map.insert("int".to_string(), Value::NativeFunction(&INT));
    map.insert("float".to_string(), Value::NativeFunction(&FLOAT));
    map.insert("string".to_string(), Value::NativeFunction(&STRING));

    map.insert("len".to_string(), Value::NativeFunction(&LEN));
    map.insert("max".to_string(), Value::NativeFunction(&MAX));
    map.insert("min".to_string(), Value::NativeFunction(&MIN));
    map.insert("sum".to_string(), Value::NativeFunction(&SUM));
    map.insert("prod".to_string(), Value::NativeFunction(&PROD));
    map.insert("zip".to_string(), Value::NativeFunction(&ZIP));

    map.insert("filter".to_string(), Value::NativeFunction(&FILTER));
    map.insert("map".to_string(), Value::NativeFunction(&MAP));
    map.insert("reduce".to_string(), Value::NativeFunction(&REDUCE));

    map
}