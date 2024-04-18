use std::{fmt::Display, rc::Rc};

use rustc_hash::FxHashMap;

use crate::ast;

use super::{Closure, TypeDef, Value};


#[derive(Debug)]
pub enum TaggedValue {
    Int(i64),
    Float(f64),
    Bool(bool),
    Str(String),
    Arr(Vec<TaggedValue>),
    Maybe(Option<Box<TaggedValue>>),
    Closure(Box<Closure>),
    TypeDef(Rc<TypeDef>),
    Object(String, FxHashMap<String, TaggedValue>),
}

impl Display for TaggedValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TaggedValue::Int(i) => write!(f, "{}", i),
            TaggedValue::Float(fl) => write!(f, "{:.?}", fl),
            TaggedValue::Bool(b) => write!(f, "{}", b),
            TaggedValue::Str(s) => write!(f, "{}", s),
            TaggedValue::Arr(arr) => {
                write!(f, "[")?;
                for (i, v) in arr.iter().enumerate() {
                    if i != 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", v)?;
                }
                write!(f, "]")
            },
            TaggedValue::Maybe(maybe) => {
                match maybe {
                    Some(v) => write!(f, "Some({})", v),
                    None => write!(f, "Null"),
                }
            },
            TaggedValue::Closure(closure) => write!(f, "{}", closure),
            TaggedValue::TypeDef(typedef) => write!(f, "{}", typedef),
            TaggedValue::Object(name, fields) => {
                write!(f, "{} {{ ", name)?;
                for (i, (k, v)) in fields.iter().enumerate() {
                    if i != 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}: {}", k, v)?;
                }
                write!(f, " }}")
            }
        }
    }
}

impl TaggedValue {
    pub fn from_value(value: Value, typ: &ast::Type) -> Result<TaggedValue, String> {
        Ok(match typ {
            ast::Type::Int => TaggedValue::Int(unsafe { value.i }),
            ast::Type::Float => TaggedValue::Float(unsafe { value.f }),
            ast::Type::Bool => TaggedValue::Bool(unsafe { value.b }),
            t => return Err(
                format!(
                    "Cannot extract value from type {:?}", t
                )
            ),
        })
    }
    pub fn from_array(arr: &[Value], typ: &ast::Type) -> TaggedValue {
        TaggedValue::Arr(arr.iter().map(|x| {
            match typ {
                ast::Type::Int => TaggedValue::Int(unsafe { x.i }),
                ast::Type::Float => TaggedValue::Float(unsafe { x.f }),
                ast::Type::Bool => TaggedValue::Bool(unsafe { x.b }),
                _ => unimplemented!(),
            }
        }).collect())
    }
    pub fn from_maybe(maybe: Option<Value>, typ: &ast::Type) -> Result<TaggedValue, String> {
        Ok(match maybe {
            Some(x) => TaggedValue::Maybe(Some(Box::new(TaggedValue::from_value(x, typ)?))),
            None => TaggedValue::Maybe(None),
        })
    }
}
