use crate::{chunk::OpCode, values::HeapValue};

use super::*;

#[derive(Debug)]
pub struct IfStatement {
    condition: Box<dyn Expression>,
    then_branch: Box<dyn Expression>,
    else_branch: Option<Box<dyn Expression>>,
    parent: Option<*const dyn Expression>,
}

impl IfStatement {
    pub fn new(
        condition: Box<dyn Expression>,
        then_branch: Box<dyn Expression>,
        else_branch: Option<Box<dyn Expression>>,
    ) -> Self {
        Self { condition, then_branch, else_branch, parent: None }
    }
}

impl Expression for IfStatement {
    fn get_type(&self) -> Result<Type, String> {
        let condition_type = self.condition.get_type()?;
        if condition_type != Type::Bool {
            return Err(format!(
                "If condition must be a boolean, but got {:?}",
                condition_type
            ));
        }
        let then_branch_type = self.then_branch.get_type()?;
        match &self.else_branch {
            None => Ok(Type::Maybe(Box::new(then_branch_type))),
            Some(else_branch) => {
                let else_branch_type = else_branch.get_type()?;
                if then_branch_type != else_branch_type {
                    Err(format!(
                        "If and else branches have different types: {:?} and {:?}",
                        then_branch_type, else_branch_type
                    ))
                }
                else {
                    Ok(then_branch_type)
                }
            }
        }
    }
    fn set_parent(&mut self, parent: Option<*const dyn Expression>) -> Result<(), String> {
        self.parent = parent;
        let self_ptr = self as *const dyn Expression;
        self.condition.set_parent(Some(self_ptr))?;
        self.then_branch.set_parent(Some(self_ptr))?;
        if let Some(else_branch) = &mut self.else_branch {
            else_branch.set_parent(Some(self_ptr))?;
        }
        Ok(())
    }
    fn get_parent(&self) -> Option<*const dyn Expression> {
        self.parent
    }

    fn compile(&self, compiler: &mut Compiler) -> Result<(), String> {
        let typ = self.get_type()?; // will error if types don't match or condition is not a bool
        let inner_is_heap = match typ {
            Type::Maybe(t) => t.is_heap(),
            _ => false,
        };
        self.condition.compile(compiler)?;
        let jump_if_idx = compiler.write_jump(OpCode::JumpIfFalse)?;
        self.then_branch.compile(compiler)?;
        if self.else_branch.is_none() {
            compiler.write_opcode(
                if inner_is_heap { OpCode::WrapHeapSome } else { OpCode::WrapSome }
            );
        }
        let jump_else_idx = compiler.write_jump(OpCode::Jump)?;
        compiler.patch_jump(jump_if_idx)?;
        if let Some(else_branch) = &self.else_branch {
            else_branch.compile(compiler)?;
        }
        else {
            compiler.write_heap_constant(
                if inner_is_heap { HeapValue::MaybeHeap(None) } else { HeapValue::Maybe(None) }
            )?;
        }
        compiler.patch_jump(jump_else_idx)
    }
}   
