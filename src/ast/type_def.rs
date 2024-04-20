use std::rc::Rc;

use crate::values::{self, HeapValue};

use super::*;

#[derive(Debug)]
pub struct TypeDef {
    name: String,
    fields: Vec<NameAndType>,
    parent: Option<*const dyn Expression>,
}

impl TypeDef {
    pub fn new(name: String, fields: Vec<NameAndType>) -> Self {
        Self { name, fields, parent: None }
    }

    fn field_types(&self) -> Result<Vec<(String, Type)>, String> {
        let mut field_types = Vec::new();
        for p in self.fields.iter() {
            field_types.push((p.name.clone(), p.get_type()?));
        }
        Ok(field_types)
    }
}

impl Expression for TypeDef {
    fn get_type(&self) -> Result<Type, String> {
        let field_types = self.field_types()?;
        let types_only = field_types.iter().map(|(_, t)| t.clone()).collect::<Vec<_>>();
        Ok(Type::Func(
            types_only,
            Box::new(Type::Object(self.name.clone(), field_types))
        ))
    }
    fn set_parent(&mut self, parent: Option<*const dyn Expression>) -> Result<(), String> {
        self.parent = parent;
        let self_ptr = self as *const dyn Expression;
        for field in self.fields.iter_mut() {
            field.typ.set_parent(Some(self_ptr))?;
        }
        Ok(())
    }
    fn get_parent(&self) -> Option<*const dyn Expression> {
        self.parent
    }
    fn find_vartype(&self, name: &String, _upto: *const dyn Expression) -> Result<Option<Type>, String> {
        for f in self.fields.iter() {
            if &f.name == name {
                return Ok(Some(f.get_type()?));
            }
        }
        Ok(None)
    }

    fn compile(&self, compiler: &mut Compiler) -> Result<(), String> {
        let mut fields = Vec::new();
        for f in self.fields.iter() {
            fields.push((f.name.clone(), f.get_type()?.is_heap()));
        }
        let typedef = HeapValue::TypeDef(Rc::new(
            values::TypeDef::new(self.name.clone(), fields)
        ));
        compiler.write_heap_constant(typedef)
    }
}
