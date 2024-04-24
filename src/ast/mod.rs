mod array;
mod assignment;
mod binary;
mod block;
mod call;
mod function;
mod functional_ops;
mod get_field;
mod if_statement;
mod literal;
mod maybe;
mod top_level;
mod type_annotation;
mod type_def;
mod unary;
mod variable;

pub use array::*;
pub use assignment::*;
pub use binary::*;
pub use block::*;
pub use call::*;
pub use function::*;
pub use functional_ops::*;
pub use get_field::*;
pub use if_statement::*;
pub use literal::*;
pub use maybe::*;
pub use top_level::*;
pub use type_annotation::*;
pub use type_def::*;
pub use unary::*;
pub use variable::*;

use downcast_rs::{Downcast, impl_downcast};

use crate::{compiler::Compiler, wasmizer::Wasmizer};


#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Type {
    Int,
    Float,
    Str,
    Bool,
    Arr(Box<Type>),
    Iter(Box<Type>),
    Maybe(Box<Type>),
    Func(Vec<Type>, Box<Type>),
    TypeDef(Vec<Type>, Box<Type>),
    Object(String, Vec<(String, Type)>)
}

impl Type {
    pub fn is_heap(&self) -> bool {
        matches!(self, Self::Str | Self::Arr(_) | Self::Iter(_) | Self::Maybe(_) | Self::Func(..) | Self::TypeDef(..) | Self::Object(..))
    }
}


#[derive(Debug, Clone)]
pub struct VarType {
    name: String,
    typ: Type, 
}

impl VarType {
    pub fn new(name: String, typ: Type) -> Self {
        Self { name, typ }
    }
}


pub trait Expression: std::fmt::Debug + Downcast {
    fn get_type(&self) -> Result<Type, String>;

    // set parent should set the parent for this expression,
    // then call set_parent on all of its children
    fn set_parent(&mut self, parent: Option<*const dyn Expression>) -> Result<(), String>;
    fn get_parent(&self) -> Option<*const dyn Expression>;
    // get a list of variables and their types that are defined in this expression
    // will stop looking if the given expression if reached
    #[allow(unused_variables)]
    fn find_vartype(&self, name: &String, upto: *const dyn Expression) -> Result<Option<Type>, String> {
        Ok(None)
    }

    fn compile(&self, compiler: &mut Compiler) -> Result<(), String>;
    #[allow(unused_variables)]
    fn wasmize(&self, wasmizer: &mut Wasmizer) -> Result<(), String> {
        Err(format!("wasmize not implemented for {}", std::any::type_name::<Self>()))
    }
}

impl_downcast!(Expression);


fn resolve_type(name: &String, origin: *const dyn Expression) -> Result<Type, String> {
    // climb up the tree looking for a VarType with a matching name
    let mut e = origin;
    loop {
        let last_e = e;
        if let Some(parent) = unsafe { (*e).get_parent() } {
            e = parent;
        }
        else {
            // reached top of tree
            return Err(format!(
                "Could not find definition for variable {}", name
            ));
        }
        let typ = unsafe { (*e).find_vartype(&name, last_e) };
        if let Some(typ) = typ? {
            return Ok(typ);
        }
    }
}

fn truncate_template_types(name: &String) -> &str {
    // cuts off anything that occurs after [ char
    name.split_terminator('[').nth(0).unwrap()
}

#[derive(Debug)]
pub struct ErrorExpression;

impl Expression for ErrorExpression {
    fn get_type(&self) -> Result<Type, String> {
        Err("ErrorExpressions have no type".to_string()).unwrap()
    }
    fn set_parent(&mut self, _parent: Option<*const dyn Expression>) -> Result<(), String> {
        Ok(())
    }
    fn get_parent(&self) -> Option<*const dyn Expression> {
        None
    }
    fn compile(&self, _compiler: &mut Compiler) -> Result<(), String> {
        Err("Tried to compile an ErrorExpression".to_string())
    }
}
