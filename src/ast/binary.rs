use crate::{chunk::OpCode, token::TokenType};

use super::*;

#[derive(Debug)]
pub struct Binary {
    left: Box<dyn Expression>,
    op: TokenType,
    right: Box<dyn Expression>,
    parent: Option<*const dyn Expression>,
}

impl Binary {
    pub fn new(left: Box<dyn Expression>, op: TokenType, right: Box<dyn Expression>) -> Result<Self, String> {
        Ok(Self {
            left,
            op,
            right,
            parent: None,
        })
    }
}

impl Expression for Binary {
    fn get_type(&self) -> Result<Type, String> {
        match self.op {
            TokenType::Eq
            | TokenType::NEq
            | TokenType::GEq
            | TokenType::LEq
            | TokenType::GT
            | TokenType::LT => Ok(Type::Bool),
            TokenType::To => Ok(Type::Iter(Box::new(Type::Int))),
            _ => self.left.get_type(),
        }
    }
    fn set_parent(&mut self, parent: Option<*const dyn Expression>) -> Result<(), String> {
        self.parent = parent;
        let self_ptr = self as *const dyn Expression;
        self.left.set_parent(Some(self_ptr))?;
        self.right.set_parent(Some(self_ptr))?;
        Ok(())
    }
    fn get_parent(&self) -> Option<*const dyn Expression> {
        self.parent
    }

    fn compile(&self, compiler: &mut Compiler) -> Result<(), String> {
        let left_type = self.left.get_type()?;
        let right_type = self.right.get_type()?;

        if left_type != right_type {
            return Err(format!(
                "Operands for operator {:?} must be of the same type; got {:?} and {:?}",
                self.op, left_type, right_type
            ));
        }
        self.left.compile(compiler)?;
        self.right.compile(compiler)?;

        match left_type {
            Type::Int => {
                compiler.write_opcode(match self.op {
                    TokenType::Eq => OpCode::IntEqual,
                    TokenType::NEq => OpCode::IntNotEqual,
                    TokenType::GT => OpCode::IntGreater,
                    TokenType::GEq => OpCode::IntGreaterEqual,
                    TokenType::LT => OpCode::IntLess,
                    TokenType::LEq => OpCode::IntLessEqual,
                    TokenType::Plus => OpCode::IntAdd,
                    TokenType::Minus => OpCode::IntSubtract,
                    TokenType::Star => OpCode::IntMultiply,
                    TokenType::Slash => OpCode::IntDivide,
                    TokenType::To => OpCode::To,
                    x => return Err(format!(
                        "Operator {:?} not supported for type {:?}",
                        x, left_type
                    )),
                })
            },
            Type::Float => {
                compiler.write_opcode(match self.op {
                    TokenType::Eq => OpCode::FloatEqual,
                    TokenType::NEq => OpCode::FloatNotEqual,
                    TokenType::GT => OpCode::FloatGreater,
                    TokenType::GEq => OpCode::FloatGreaterEqual,
                    TokenType::LT => OpCode::FloatLess,
                    TokenType::LEq => OpCode::FloatLessEqual,
                    TokenType::Plus => OpCode::FloatAdd,
                    TokenType::Minus => OpCode::FloatSubtract,
                    TokenType::Star => OpCode::FloatMultiply,
                    TokenType::Slash => OpCode::FloatDivide,
                    x => return Err(format!(
                        "Operator {:?} not supported for type {:?}",
                        x, left_type
                    ))
                })
            },
            Type::Bool => {
                compiler.write_opcode(match self.op {
                    TokenType::Eq => OpCode::BoolEqual,
                    TokenType::NEq => OpCode::BoolNotEqual,
                    TokenType::And => OpCode::And,
                    TokenType::Or => OpCode::Or,
                    x => return Err(format!(
                        "Operator {:?} not supported for type {:?}",
                        x, left_type
                    ))
                })
            },
            Type::Str => {
                compiler.write_opcode(match self.op {
                    TokenType::Eq => OpCode::HeapEqual,
                    TokenType::NEq => OpCode::HeapNotEqual,
                    TokenType::Plus => OpCode::Concat,
                    x => return Err(format!(
                        "Operator {:?} not supported for type {:?}",
                        x, left_type
                    ))
                })
            },
            Type::Arr(t) => {
                compiler.write_opcode(match self.op {
                    TokenType::Eq => OpCode::HeapEqual,
                    TokenType::NEq => OpCode::HeapNotEqual,
                    TokenType::Plus => OpCode::Concat,
                    x => return Err(format!(
                        "Operator {:?} not supported for type {:?}",
                        x, t
                    ))
                })
            },
            x => return Err(format!(
                "Type {:?} not yet supported for binary operation", x
            ))
        };
        Ok(())
    }

    fn wasmize(&self, wasmizer: &mut Wasmizer) -> Result<(), String> {
        let left_type = self.left.get_type()?;
        let right_type = self.right.get_type()?;

        if left_type != right_type {
            return Err(format!(
                "Operands for operator {:?} must be of the same type; got {:?} and {:?}",
                self.op, left_type, right_type
            ));
        }

        self.left.wasmize(wasmizer)?;
        self.right.wasmize(wasmizer)?;
        match self.op {
            TokenType::Plus => wasmizer.write_add(&left_type),
            TokenType::Minus => wasmizer.write_sub(&left_type),
            TokenType::Star => wasmizer.write_mul(&left_type),
            TokenType::Slash => wasmizer.write_div(&left_type),
            _ => unimplemented!(),
        }
    }
}
