use crate::{chunk::OpCode, compiler::TypeContext};

use super::*;

#[derive(Debug)]
pub struct ASTTopLevel {
    types: Vec<VarType>,
    child: Box<dyn Expression>,
}

impl ASTTopLevel {
    pub fn new(typecontext: TypeContext, child: Box<dyn Expression>) -> Self {
        let mut types = Vec::new();
        for (name, typ) in typecontext.borrow().iter() {
            types.push(VarType::new(name.clone(), typ.clone()));
        }
        Self { types, child }
    }
}

impl Expression for ASTTopLevel {
    fn get_type(&self) -> Result<Type, String> {
        self.child.get_type()
    }
    fn set_parent(&mut self, _parent: Option<*const dyn Expression>) -> Result<(), String> {
        let self_ptr = self as *const dyn Expression;
        self.child.set_parent(Some(self_ptr))
    }
    fn get_parent(&self) -> Option<*const dyn Expression> {
        None
    }
    fn find_vartype(&self, name: &String, _upto: *const dyn Expression) -> Result<Option<Type>, String> {
        for t in self.types.iter() {
            if &t.name == name {
                return Ok(Some(t.typ.clone()));
            }
        }
        Ok(None)
    }

    fn compile(&self, compiler: &mut Compiler) -> Result<(), String> {
        self.child.compile(compiler)?;
        compiler.write_opcode(OpCode::Return);
        Ok(())
    }

    fn wasmize(&self, wasmizer: &mut Wasmizer) -> Result<(), String> {
        self.child.wasmize(wasmizer)?;
        wasmizer.finish_func();
        Ok(())
    }
}
