use crate::{chunk::OpCode, values::Value};

use super::*;

#[derive(Debug)]
pub struct GetField {
    object: Box<dyn Expression>,
    field: String,  
    parent: Option<*const dyn Expression>,
}

impl GetField {
    pub fn new(object: Box<dyn Expression>, field: String) -> Self {
        Self { object, field, parent: None }
    }
}

impl Expression for GetField {
    fn get_type(&self) -> Result<Type, String> {
        let object_type = self.object.get_type()?;
        match &object_type {
            Type::Object(_, fields) => {
                for (name, typ) in fields.iter() {
                    if name == &self.field {
                        return Ok(typ.clone());
                    }
                }
                Err(format!(
                    "Field {:?} not found in type {:?}", self.field, object_type
                ))
            },
            _ => Err(format!(
                "Field access on non-object type {:?}", object_type
            ))
        }
    }
    fn set_parent(&mut self, parent: Option<*const dyn Expression>) -> Result<(), String> {
        self.parent = parent;
        let self_ptr = self as *const dyn Expression;
        self.object.set_parent(Some(self_ptr))
    }
    fn get_parent(&self) -> Option<*const dyn Expression> {
        self.parent
    }

    fn compile(&self, compiler: &mut Compiler) -> Result<(), String> {
        let typ = self.get_type()?;
        compiler.write_constant(Value { b: typ.is_heap() })?;
        compiler.write_string(self.field.clone())?;
        self.object.compile(compiler)?;
        compiler.write_opcode(OpCode::Call);
        Ok(())
    }

    fn wasmize(&self, wasmizer: &mut Wasmizer) -> Result<i32, String> {
        let _typ = self.get_type()?;
        self.object.wasmize(wasmizer)?;
        let object_type = self.object.get_type()?;
        wasmizer.get_field(object_type, &self.field)?;
        Ok(0)
    }
}
