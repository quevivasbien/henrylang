use crate::chunk::OpCode;

use super::*;

#[derive(Debug)]
pub struct Block {
    expressions: Vec<Box<dyn Expression>>,
    parent: Option<*const dyn Expression>,
}

impl Block {
    pub fn new(expressions: Vec<Box<dyn Expression>>) -> Result<Self, String> {
        if expressions.is_empty() {
            return Err("Block must have at least one expression".to_string());
        };
        Ok(Self { expressions, parent: None })
    }

    // get the number of functions defined within this block
    pub fn count_function_chidren(&self) -> usize {
        let mut count = 0;
        for e in self.expressions.iter() {
            if let Some(block) = e.downcast_ref::<Block>() {
                count += block.count_function_chidren();
            }
            if let Some(function) = e.downcast_ref::<Function>() {
                count += 1 + function.count_function_chidren();

            }
        }
        count
    }
}

impl Expression for Block {
    fn get_type(&self) -> Result<Type, String> {
        self.expressions.last().unwrap().get_type()
    }
    fn set_parent(&mut self, parent: Option<*const dyn Expression>) -> Result<(), String> {
        self.parent = parent;
        let self_ptr = self as *const dyn Expression;
        for e in self.expressions.iter_mut() {
            e.set_parent(Some(self_ptr))?;
        }
        Ok(())
    }
    fn get_parent(&self) -> Option<*const dyn Expression> {
        self.parent
    }
    fn find_vartype(&self, name: &String, upto: *const dyn Expression) -> Result<Option<Type>, String> {
        for e in self.expressions.iter() {
            if e.as_ref() as *const _ as *const () == upto as *const () {
                break;
            }
            if let Some(t) = e.find_vartype(name, upto)? {
                return Ok(Some(t));
            };
        }
        Ok(None)
    }

    fn compile(&self, compiler: &mut Compiler) -> Result<(), String> {
        compiler.begin_scope();
        for e in self.expressions.iter().take(self.expressions.len() - 1) {
            e.compile(compiler)?;
            compiler.write_opcode(
                if e.get_type()?.is_heap() {
                    OpCode::EndHeapExpr
                }
                else {
                    OpCode::EndExpr
                }
            );
        }
        self.expressions.last().unwrap().compile(compiler)?;
        compiler.end_scope(self.get_type()?.is_heap())
    }

    fn wasmize(&self, wasmizer: &mut Wasmizer) -> Result<i32, String> {
        wasmizer.begin_scope(&self.get_type()?)?;
        for (i, e) in self.expressions.iter().enumerate() {
            e.wasmize(wasmizer)?;
            if i < self.expressions.len() - 1 {
                wasmizer.write_drop();
            }
        }
        wasmizer.end_scope()?;
        Ok(0)
    }
}
