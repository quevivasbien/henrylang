use std::{cell::RefCell, rc::Rc};

use rustc_hash::FxHashMap;

use crate::{ast::Type, compiler::TypeContext, wasmizer::wasmtypes::{FuncTypeSignature, Numtype}};

fn print<T: std::fmt::Display>(x: T) -> T {
    println!("{}", x);
    x
}

pub fn get_wasmer_imports(store: &mut wasmer::Store) -> wasmer::Imports {
    wasmer::imports! {
        "env" => {
            "print[Int]" => wasmer::Function::new_typed(store, print::<i32>),
            "print[Float]" => wasmer::Function::new_typed(store, print::<f32>),
        }
    }
}

pub struct Import {
    pub module: &'static str,
    pub field: &'static str,
    pub sig: FuncTypeSignature,
}

impl Import {
    pub fn new(module: &'static str, field: &'static str, sig: FuncTypeSignature) -> Self {
        Self { module, field, sig }
    }
}

pub type GlobalVars = Rc<RefCell<FxHashMap<String, u64>>>;

pub struct Env {
    pub global_vars: GlobalVars,
    pub global_types: TypeContext,
    pub imports: Vec<Import>,
}

impl Default for Env {
    fn default() -> Self {
        let mut global_vars = FxHashMap::default();
        global_vars.insert("print[Int]".to_string(), 0);
        global_vars.insert("print[Float]".to_string(), 1);
        let global_scope = Rc::new(RefCell::new(global_vars));

        let mut global_types = FxHashMap::default();
        global_types.insert("print[Int]".to_string(), Type::Func(vec![Type::Int], Box::new(Type::Int)));
        global_types.insert("print[Float]".to_string(), Type::Func(vec![Type::Float], Box::new(Type::Float)));
        let global_types = Rc::new(RefCell::new(global_types));

        Self::new(global_scope, global_types)
    }
}

impl Env {
    pub fn new(global_vars: GlobalVars, global_types: TypeContext) -> Self {
        let imports = vec![
            Import::new("env", "print[Int]", FuncTypeSignature::new(vec![Numtype::I32], Some(Numtype::I32))),
            Import::new("env", "print[Float]", FuncTypeSignature::new(vec![Numtype::F32], Some(Numtype::F32))),
        ];
        Self {
            global_vars,
            global_types,
            imports
        }
    }
}


fn view_string(memview: &wasmer::MemoryView, offset: u64, size: u64) -> Result<String, String> {
    let mut buf = vec![0u8; size as usize];
    memview.read(offset, &mut buf).map_err(|e| format!("{}", e))?;
    let out = String::from_utf8(buf).map_err(|e| format!("{}", e))?;
    #[cfg(feature = "debug")]
    println!("String: size: {}, out: {:?}", size, out);
    Ok(out)
}

fn view_object(memview: &wasmer::MemoryView, offset: u64, size: u64, name: String, fields: Vec<(String, Type)>) -> Result<String, String> {
    let mut buf = vec![0u8; size as usize];
    memview.read(offset, &mut buf).map_err(|e| format!("{}", e))?;
    let mut str_comps = Vec::with_capacity(fields.len());
    let mut offset = 0;
    for (name, typ) in fields.into_iter() {
        match typ {
            Type::Bool => {
                let value = i32::from_le_bytes(buf[offset..offset+4].try_into().unwrap()) != 0;
                offset += 4;
                str_comps.push(format!("{}: {}", name, value));
            },
            Type::Int => {
                let value = i32::from_le_bytes(buf[offset..offset+4].try_into().unwrap());
                offset += 4;
                str_comps.push(format!("{}: {}", name, value));
            },
            Type::Float => {
                let value = f32::from_le_bytes(buf[offset..offset+4].try_into().unwrap());
                offset += 4;
                str_comps.push(format!("{}: {:?}", name, value));
            },
            _ => {
                // value should be heap type
                let fatptr = i64::from_le_bytes(buf[offset..offset+8].try_into().unwrap());
                offset += 8;
                str_comps.push(format!("{}: {}", name, view_memory(memview, fatptr, typ)?));
            }
        }
    }
    Ok(format!("{} {{ {} }}", name, str_comps.join(", ")))
}

fn view_memory(memview: &wasmer::MemoryView, fatptr: i64, typ: Type) -> Result<String, String> {
    let offset = (fatptr >> 32) as u64;
    let size = (fatptr & 0xffffffff) as u64;
    let arrtype = match typ {
        Type::Arr(t) => *t,
        Type::Str => return view_string(memview, offset, size),
        Type::Object(name, fields) => return view_object(memview, offset, size, name, fields),
        _ => return Err(format!("Unexpected type: {:?}", typ)),
    };
    let result = match arrtype {
        Type::Int => {
            let mut out = Vec::new();
            for i in (0..size).step_by(4) {
                let mut bytes = [0u8; 4];
                memview.read(offset + i, &mut bytes).map_err(|e| format!("{}", e))?;
                out.push(i32::from_le_bytes(bytes));
            }
            format!("{:?}", out)
        },
        Type::Float => {
            let mut out = Vec::new();
            for i in (0..size).step_by(4) {
                let mut bytes = [0u8; 4];
                memview.read(offset + i, &mut bytes).map_err(|e| format!("{}", e))?;
                out.push(f32::from_le_bytes(bytes));
            }
            format!("{:?}", out)
        },
        Type::Bool => {
            let mut out = Vec::new();
            for i in (0..size).step_by(4) {
                let mut bytes = [0u8; 4];
                memview.read(offset + i, &mut bytes).map_err(|e| format!("{}", e))?;
                out.push(i32::from_le_bytes(bytes) != 0);
            }
            format!("{:?}", out)
        }
        // TODO: Handle nested array types
        _ => return Err(format!("Unexpected array type: {:?}", arrtype)),
    };
    Ok(result)
}

pub fn run_wasm(bytes: &[u8], typ: Type) -> Result<String, String> {
    let mut store = wasmer::Store::default();
    let module = wasmer::Module::new(&store, bytes).map_err(|e| format!("{}", e))?;
    let import_object = get_wasmer_imports(&mut store);
    let instance = wasmer::Instance::new(&mut store, &module, &import_object).map_err(|e| format!("{}", e))?;

    let main = instance.exports.get_function("main").map_err(|e| format!("{}", e))?;
    let result = main.call(&mut store, &[]).map_err(|e| format!("{}", e))?;
    let result = match (&result[0], &typ) {
        (wasmer::Value::I32(i), Type::Int) => format!("{}", i),
        (wasmer::Value::I32(i), Type::Bool) => format!("{}", *i != 0),
        (wasmer::Value::F32(f), Type::Float) => format!("{:?}", f),
        (wasmer::Value::I64(fatptr), _) => {
            let memory = instance.exports.get_memory("memory").map_err(|e| format!("{}", e))?;
            let memview = memory.view(&store);
            view_memory(&memview, *fatptr, typ)?
        }
        (wasmer::Value::I32(_), Type::TypeDef(_, t)) => {
            let typename = match t.as_ref() {
                Type::Object(name, _) => name,
                _ => unreachable!()
            };
            format!("<constructor for type `{}`>", typename)
        }
        _ => format!("Unexpected result: {:?}", result),
    };

    Ok(result)
}
