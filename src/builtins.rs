use std::collections::HashMap;
use std::rc::Rc;

use lazy_static::lazy_static;

use crate::{NativeFunction, Value};

lazy_static! {
    static ref PRINT: NativeFunction = NativeFunction {
        name: "print",
        arity: 1,
        function: |_vm, args| {
            Ok(match &args[0] {
                Value::String(x) => {
                    println!("{}", x);
                    Value::String(x.clone())
                },
                x => {
                    println!("{}", x);
                    Value::String(Rc::new(format!("{}", x)))
                }
            })
        }
    };

    static ref TYPEOF: NativeFunction = NativeFunction {
        name: "typeof",
        arity: 1,
        function: |_vm, args| {
            fn recurse_type(x: &Value) -> String {
                match x {
                    Value::Int(_) => "Int".to_string(),
                    Value::Float(_) => "Float".to_string(),
                    Value::String(_) => "String".to_string(),
                    Value::Bool(_) => "Bool".to_string(),
                    Value::Array(_) => "Array".to_string(),
                    Value::Object(x) => {
                        format!("Object({})", x.typedef.name)
                    },
                    Value::Closure(_) => "Function".to_string(),
                    Value::NativeFunction(_) => "Function".to_string(),
                    Value::TypeDef(_) => "TypeDef".to_string(),
                    Value::Maybe(x) => {
                        match x.as_ref() {
                            Some(x) => format!("Some({})", recurse_type(x)),
                            None => "Null".to_string(),
                        }
                    },
                }
            }
            Ok(Value::String(Rc::new(recurse_type(&args[0]))))
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
                Value::Bool(x) => *x as i64,
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
            Ok(match &args[0] {
                Value::String(x) => {
                    Value::String(x.clone())
                },
                x => {
                    Value::String(Rc::new(format!("{}", x)))
                }
            })
        }
    };

    static ref ARRAY: NativeFunction = NativeFunction {
        name: "array",
        arity: 1,
        function: |vm, args| {
            let result = match &args[0] {
                Value::Array(x) => x.clone(),
                Value::String(x) => {
                    Rc::new(x.chars().map(
                        |x| {
                            Value::String(Rc::new(x.to_string()))
                        }
                    ).collect())
                },
                Value::Object(x) => {
                    Rc::new(x.typedef.fieldnames.iter().map(
                        |fieldname| {
                            x.fields.get(fieldname).unwrap().clone()
                        }
                    ).collect())
                },
                x => return Err(vm.runtime_err(
                    format!("Cannot call array on non-iterable type, got {:?}", x)
                ))
            };
            Ok(Value::Array(result))
        }
    };

    static ref IS_NULL: NativeFunction = NativeFunction {
        name: "is_null",
        arity: 1,
        function: |_vm, args| {
            match &args[0] {
                Value::Maybe(x) => Ok(match x.as_ref() {
                    Some(_) => Value::Bool(false),
                    None => Value::Bool(true),
                }),
                _ => Ok(Value::Bool(false)),
            }
        }
    };

    static ref IS_SOME: NativeFunction = NativeFunction {
        name: "is_some",
        arity: 1,
        function: |_vm, args| {
            match &args[0] {
                Value::Maybe(x) => Ok(match x.as_ref() {
                    Some(_) => Value::Bool(true),
                    None => Value::Bool(false),
                }),
                _ => Ok(Value::Bool(true)),
            }
        }
    };

    static ref UNWRAP: NativeFunction = NativeFunction {
        name: "unwrap",
        arity: 2,
        function: |vm, args| {
            match &args[0] {
                Value::Maybe(x) => Ok(match x.as_ref() {
                    Some(x) => x.clone(),
                    None => args[1].clone(),
                }),
                x => return Err(vm.runtime_err(
                    format!("Cannot call unwrap on non-maybe type, got {:?}", x)
                )),
            }
        }
    };

    static ref LEN: NativeFunction = NativeFunction {
        name: "len",
        arity: 1,
        function: |vm, args| {
            let length = match &args[0] {
                Value::Array(x) => x.len(),
                Value::String(x) => x.len(),
                Value::Object(x) => x.fields.len(),
                x => return Err(vm.runtime_err(
                    format!("Cannot call len on non-iterable type, got {:?}", x)
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

    map.insert("typeof".to_string(), Value::NativeFunction(&TYPEOF));
    map.insert("int".to_string(), Value::NativeFunction(&INT));
    map.insert("float".to_string(), Value::NativeFunction(&FLOAT));
    map.insert("string".to_string(), Value::NativeFunction(&STRING));
    map.insert("array".to_string(), Value::NativeFunction(&ARRAY));

    map.insert("is_null".to_string(), Value::NativeFunction(&IS_NULL));
    map.insert("is_some".to_string(), Value::NativeFunction(&IS_SOME));
    map.insert("unwrap".to_string(), Value::NativeFunction(&UNWRAP));

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
