use std::{fmt::{Debug, Display}, rc::Rc};

use rustc_hash::FxHashMap;

use super::{HeapValue, Value};

pub struct TypeDef {
    pub name: String,
    pub fields: Vec<(String, bool)>,
}

impl Debug for TypeDef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.name.is_empty() {
            write!(f, "{{ {:?} }}", self.fields)
        }
        else {
            write!(f, "{} {{ {:?} }}", self.name, self.fields)
        }
    }
}

impl Display for TypeDef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.name.is_empty() {
            write!(f, "<anontype> {{")?;
        }
        else {
            write!(f, "{} {{", self.name)?;
        }
        for (field, _) in &self.fields {
            write!(f, " {}", field)?;
        }
        write!(f, " }}")
    }
}

impl TypeDef {
    pub fn new(name: String, fields: Vec<(String, bool)>) -> Self {
        Self { name, fields }
    }
}

pub struct Object {
    pub typedef: Rc<TypeDef>,
    pub fields: FxHashMap<String, Value>,
    pub heap_fields: FxHashMap<String, HeapValue>,
}

impl Debug for Object {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.typedef.name.is_empty() {
            write!(f, "{:?} {:?}", self.fields, self.heap_fields)
        }
        else {
            write!(f, "{} {:?} {:?}", self.typedef, self.fields, self.heap_fields)
        }
    }
}

impl Object {
    pub fn new(typedef: Rc<TypeDef>, fields: FxHashMap<String, Value>, heap_fields: FxHashMap<String, HeapValue>) -> Self {
        Self { typedef, fields, heap_fields }
    }
}