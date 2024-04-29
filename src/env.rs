use std::{cell::RefCell, rc::Rc};

use rustc_hash::FxHashMap;

use crate::{ast::Type, compiler::TypeContext, wasmtypes::{FuncTypeSignature, Numtype}};

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
            Import::new("env", "print[Int]", FuncTypeSignature::new(vec![Numtype::I32], Numtype::I32)),
            Import::new("env", "print[Float]", FuncTypeSignature::new(vec![Numtype::F32], Numtype::F32)),
        ];
        Self {
            global_vars,
            global_types,
            imports
        }
    }
}
