use crate::chunk::OpCode;

use super::*;

#[derive(Debug)]
pub struct Function {
    name: String,
    params: Vec<NameAndType>,
    block: Box<dyn Expression>,
    // return type will be inferred if not explicitly provided
    rtype: Option<TypeAnnotation>,
    parent: Option<*const dyn Expression>,
}

impl Function {
    pub fn new(name: String, params: Vec<NameAndType>, rtype: Option<TypeAnnotation>, block: Box<dyn Expression>) -> Self {
        Self { name, params, block, rtype, parent: None }
    }

    fn param_types(&self) -> Result<Vec<Type>, String> {
        let mut param_types = Vec::new();
        for p in self.params.iter() {
            param_types.push(p.get_type()?);
        }
        Ok(param_types)
    }

    pub fn explicit_type(&self) -> Result<Option<Type>, String> {
        let return_type = match &self.rtype {
            None => return Ok(None),
            Some(rtype) => rtype.get_type()?,
        };
        let param_types = self.param_types()?;
        Ok(Some(Type::Func(param_types, Box::new(return_type))))
    }
}

impl Expression for Function {
    fn get_type(&self) -> Result<Type, String> {
        let param_types = self.param_types()?;
        let return_type = self.block.get_type()?;
        if let Some(rtype) = &self.rtype {
            let rtype = rtype.get_type()?;
            if rtype != return_type {
                return Err(format!("Function return type {:?} does not match type {:?} specified in type annotation", return_type, rtype));
            }
        }
        Ok(Type::Func(param_types, Box::new(return_type)))
    }
    fn set_parent(&mut self, parent: Option<*const dyn Expression>) -> Result<(), String> {
        self.parent = parent;
        let self_ptr = self as *const dyn Expression;
        if let Some(rtype) = &mut self.rtype {
            rtype.set_parent(Some(self_ptr))?;
        }
        for param in self.params.iter_mut() {
            param.typ.set_parent(Some(self_ptr))?;
        }
        self.block.set_parent(Some(self_ptr))?;
        Ok(())
    }
    fn get_parent(&self) -> Option<*const dyn Expression> {
        self.parent
    }
    fn find_vartype(&self, name: &String, _upto: *const dyn Expression) -> Result<Option<Type>, String> {
        // vartypes in block should have been already processed, since block is a child of function
        for p in self.params.iter() {
            if &p.name == name {
                return Ok(Some(p.get_type()?));
            }
        }
        Ok(None)
    }

    fn compile(&self, compiler: &mut Compiler) -> Result<(), String> {
        let mut inner_compiler = Compiler::new_from(compiler);
        let (ptypes, rtype) = match self.get_type()? {
            Type::Func(ptypes, rtype) => (ptypes, *rtype),
            _ => unreachable!(),
        };
        
        inner_compiler.function.name = self.name.clone();
        let mut heap_arity = 0;
        for t in ptypes.iter() {
            if t.is_heap() {
                heap_arity += 1; 
            }
        }
        inner_compiler.function.arity = self.params.len() as u8 - heap_arity;
        inner_compiler.function.heap_arity = heap_arity;
        inner_compiler.function.return_is_heap = rtype.is_heap();
        inner_compiler.function.name = format!("{}{:?}", self.name, self.param_types()?);
        
        for param in self.params.iter() {
            if inner_compiler.create_variable(param.name.clone(), &param.get_type()?)?.is_some() {
                return Err(format!(
                    "Function parameters should be in local scope"
                ));
            }
        }

        self.block.compile(&mut inner_compiler)?;
        inner_compiler.write_opcode(OpCode::Return);

        compiler.write_function(inner_compiler)
    }

    fn wasmize(&self, wasmizer: &mut Wasmizer) -> Result<(), String> {
        let (ptypes, rtype) = match self.get_type()? {
            Type::Func(ptypes, rtype) => (ptypes, *rtype),
            _ => unreachable!(),
        };
        let name = format!("{}{:?}", self.name, self.param_types()?);
        wasmizer.init_func(name, &ptypes, &rtype, false)?;
        for param in self.params.iter() {
            wasmizer.add_param_name(param.name.clone());
        }

        self.block.wasmize(wasmizer)?;
        wasmizer.finish_func()?;

        wasmizer.write_last_func_index();
        Ok(())
    }
}
