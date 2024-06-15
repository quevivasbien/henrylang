use crate::chunk::OpCode;

use super::*;

#[derive(Debug)]
pub struct IfStatement {
    condition: Box<dyn Expression>,
    then_branch: Box<dyn Expression>,
    else_branch: Box<dyn Expression>,
    parent: Option<*const dyn Expression>,
}

impl IfStatement {
    pub fn new(
        condition: Box<dyn Expression>,
        then_branch: Box<dyn Expression>,
        else_branch: Box<dyn Expression>,
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
        let else_branch_type = self.else_branch.get_type()?;
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
    fn set_parent(&mut self, parent: Option<*const dyn Expression>) -> Result<(), String> {
        self.parent = parent;
        let self_ptr = self as *const dyn Expression;
        self.condition.set_parent(Some(self_ptr))?;
        self.then_branch.set_parent(Some(self_ptr))?;
        self.else_branch.set_parent(Some(self_ptr))?;
        Ok(())
    }
    fn get_parent(&self) -> Option<*const dyn Expression> {
        self.parent
    }

    fn compile(&self, compiler: &mut Compiler) -> Result<(), String> {
        let _typ = self.get_type()?; // will error if types don't match or condition is not a bool
        self.condition.compile(compiler)?;
        let jump_if_idx = compiler.write_jump(OpCode::JumpIfFalse)?;
        self.then_branch.compile(compiler)?;
        let jump_else_idx = compiler.write_jump(OpCode::Jump)?;
        compiler.patch_jump(jump_if_idx)?;
        self.else_branch.compile(compiler)?;
        compiler.patch_jump(jump_else_idx)
    }

    fn wasmize(&self, wasmizer: &mut Wasmizer) -> Result<i32, String> {
        let typ = self.get_type()?; // will error if types don't match or condition is not a bool
        self.condition.wasmize(wasmizer)?;
        wasmizer.write_if(&typ)?;
        self.then_branch.wasmize(wasmizer)?;
        wasmizer.write_else()?;
        self.else_branch.wasmize(wasmizer)?;
        wasmizer.write_end()?;
        Ok(0)
    }
}   
