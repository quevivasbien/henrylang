use crate::{token::TokenType, values::Type};

struct CompileError(String);

pub trait Expression: std::fmt::Debug {
    // fn compile(&self, chunk: &mut Chunk) -> Result<(), CompileError>;
    fn get_type(&self) -> Type;
}

#[derive(Debug)]
pub struct ErrorExpression;

impl Expression for ErrorExpression {
    fn get_type(&self) -> Type {
        Type::Maybe
    }
}

#[derive(Debug)]
pub struct Block {
    typ: Type,
    pub expressions: Vec<Box<dyn Expression>>,
}

impl Block {
    pub fn new(expressions: Vec<Box<dyn Expression>>) -> Result<Self, String> {
        let typ = match expressions.last() {
            Some(expr) => expr.get_type(),
            None => return Err("Block must have at least one expression".to_string()),
        };
        Ok(Self { typ, expressions })
    }
}

impl Expression for Block {
    fn get_type(&self) -> Type {
        self.typ
    }
}

#[derive(Debug)]
pub struct Parameter {
    pub name: String,
    pub typ: Type, 
}

#[derive(Debug)]
pub struct Function {
    pub name: String,
    pub params: Vec<Parameter>,
    pub expressions: Vec<Box<dyn Expression>>,
    pub typ: Type,
}

impl Function {
    pub fn new(name: String, params: Vec<Parameter>, typ: Type) -> Self {
        Self { name, params, expressions: Vec::new(), typ }
    }
}

impl Expression for Function {
    fn get_type(&self) -> Type {
        self.typ
    }
}

#[derive(Debug)]
pub struct Literal {
    typ: Type,
    value: String,
}

impl Literal {
    pub fn new(typ: Type, value: String) -> Self {
        Self { typ, value }
    }
}

impl Expression for Literal {
    fn get_type(&self) -> Type {
        self.typ
    }
}

#[derive(Debug)]
pub struct Unary {
    op: TokenType,
    right: Box<dyn Expression>,
}

impl Unary {
    pub fn new(op: TokenType, right: Box<dyn Expression>) -> Result<Self, String> {
        // todo: Validate that operation is valid on given type
        Ok(Self { op, right })
    }
}

impl Expression for Unary {
    fn get_type(&self) -> Type {
        self.right.get_type()
    }
}

#[derive(Debug)]
pub struct Binary {
    left: Box<dyn Expression>,
    op: TokenType,
    right: Box<dyn Expression>,
    typ: Type,
}

impl Binary {
    pub fn new(left: Box<dyn Expression>, op: TokenType, right: Box<dyn Expression>) -> Result<Self, String> {
        if left.get_type() != right.get_type() {
            // TODO: Update this to reflect actual rules for the given operator
            return Err("Operands must be of the same type".to_string());
        }
        let typ = left.get_type();
        Ok(Self {
            left,
            op,
            right,
            typ,
        })
    }
}

impl Expression for Binary {
    fn get_type(&self) -> Type {
        self.typ
    }
}

struct Call {
    callee: Box<dyn Expression>,
    args: Vec<Box<dyn Expression>>,
}

struct Variable {
    name: String,
}

struct Assignment {
    name: String,
    value: Box<dyn Expression>,
}

struct If {
    condition: Box<dyn Expression>,
    consequence: Block,
    alternative: Option<Block>,
}

struct Array {
    elements: Vec<Box<dyn Expression>>,
}

#[derive(Debug)]
pub struct Maybe {
    value: Option<Box<dyn Expression>>,
}

impl Maybe {
    pub fn new(value: Option<Box<dyn Expression>>) -> Self {
        Self { value }
    }
}

impl Expression for Maybe {
    fn get_type(&self) -> Type {
        Type::Maybe
    }
}