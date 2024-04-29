use crate::{chunk::OpCode, token::TokenType};

use super::*;

#[derive(Debug)]
pub struct Unary {
    op: TokenType,
    right: Box<dyn Expression>,
    parent: Option<*const dyn Expression>,
}

impl Unary {
    pub fn new(op: TokenType, right: Box<dyn Expression>) -> Result<Self, String> {
        // todo: Validate that operation is valid on given type
        Ok(Self { op, right, parent: None })
    }
}

impl Expression for Unary {
    fn get_type(&self) -> Result<Type, String> {
        let right_type = self.right.get_type()?;
        if self.op == TokenType::At {
            match right_type {
                Type::Iter(typ) => Ok(Type::Arr(typ)),
                x => Err(format!("@ operator must be used with an iterator, got {:?}", x)),
            }
        }
        else {
            Ok(right_type)
        }
    }
    fn set_parent(&mut self, parent: Option<*const dyn Expression>) -> Result<(), String> {
        self.parent = parent;
        let self_ptr = self as *const dyn Expression;
        self.right.set_parent(Some(self_ptr))
    }
    fn get_parent(&self) -> Option<*const dyn Expression> {
        self.parent
    }

    fn compile(&self, compiler: &mut Compiler) -> Result<(), String> {
        self.right.compile(compiler)?;
        match self.right.get_type()? {
            Type::Int => match self.op {
                TokenType::Minus => compiler.write_opcode(OpCode::IntNegate),
                x => return Err(format!(
                    "Unary operator {:?} is not defined for type Int",
                    x
                )),
            },
            Type::Float => match self.op {
                TokenType::Minus => compiler.write_opcode(OpCode::FloatNegate),
                x => return Err(format!(
                    "Unary operator {:?} is not defined for type Float",
                    x
                )),
            },
            Type::Bool => match self.op {
                TokenType::Bang => compiler.write_opcode(OpCode::Not),
                x => return Err(format!(
                    "Unary operator {:?} is not defined for type Bool",
                    x
                ))
            },
            Type::Iter(_) => match self.op {
                TokenType::At => compiler.write_opcode(OpCode::Collect),
                x => return Err(format!(
                    "Unary operator {:?} is not defined for type Iterator",
                    x
                ))
            }
            x => return Err(format!("Type {:?} not yet supported for unary operation", x)),
        };
        Ok(())
    }

    fn wasmize(&self, wasmizer: &mut Wasmizer) -> Result<i32, String> {
        self.right.wasmize(wasmizer)?;
        let right_type = self.right.get_type()?;
        match (&self.op, &right_type) {
            (TokenType::Bang, Type::Bool) => wasmizer.write_negate(&right_type)?,
            (TokenType::Minus, Type::Int | Type::Float) => wasmizer.write_negate(&right_type)?,
            _ => return Err(format!("Unary operator {:?} not supported for type {:?}", self.op, right_type)),
        }
        Ok(0)
    }
}
