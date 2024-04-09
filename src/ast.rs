use crate::token::TokenType;

// struct CompileError(String);


#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Type {
    Int,
    Float,
    String,
    Bool,
    Maybe,
    Function(Vec<Type>, Box<Type>),
}

pub trait Expression: std::fmt::Debug {
    // fn compile(&self, chunk: &mut Chunk) -> Result<(), CompileError>;
    fn get_type(&self) -> Type;

    // set parent should set the parent for this expression,
    // then call set_parent on all of its children
    fn set_parent(&mut self, parent: *const dyn Expression);
    fn get_parent(&self) -> Option<*const dyn Expression>;
}

#[derive(Debug)]
pub struct ErrorExpression;

impl Expression for ErrorExpression {
    fn set_parent(&mut self, _parent: *const dyn Expression) {
        // Do nothing
    }
    fn get_parent(&self) -> Option<*const dyn Expression> {
        None
    }
    fn get_type(&self) -> Type {
        Type::Maybe
    }
}

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
}

impl Expression for Block {
    fn get_type(&self) -> Type {
        self.expressions.last().unwrap().get_type()
    }
    fn set_parent(&mut self, parent: *const dyn Expression) {
        self.parent = Some(parent);
        let self_ptr = self as *const dyn Expression;
        for e in self.expressions.iter_mut() {
            e.set_parent(self_ptr)
        }
    }
    fn get_parent(&self) -> Option<*const dyn Expression> {
        self.parent
    }
}

#[derive(Debug)]
pub struct Parameter {
    pub name: String,
    pub typ: Type, 
}

#[derive(Debug)]
pub struct Function {
    name: String,
    params: Vec<Parameter>,
    pub expressions: Vec<Box<dyn Expression>>,
    parent: Option<*const dyn Expression>,
}

impl Function {
    pub fn new(name: String, params: Vec<Parameter>, expressions: Vec<Box<dyn Expression>>) -> Self {
        Self { name, params, expressions, parent: None }
    }
}

impl Expression for Function {
    fn get_type(&self) -> Type {
        let param_types = self.params.iter().map(|p| p.typ.clone()).collect();
        let return_type = match self.expressions.last() {
            Some(expr) => expr.get_type(),
            None => Type::Maybe,
        };
        Type::Function(param_types, Box::new(return_type))
    }
    fn set_parent(&mut self, parent: *const dyn Expression) {
        self.parent = Some(parent);
        let self_ptr = self as *const dyn Expression;
        for e in self.expressions.iter_mut() {
            e.set_parent(self_ptr)
        }
    }
    fn get_parent(&self) -> Option<*const dyn Expression> {
        self.parent
    }
}

#[derive(Debug)]
pub struct Literal {
    typ: Type,
    value: String,
    parent: Option<*const dyn Expression>,
}

impl Literal {
    pub fn new(typ: Type, value: String) -> Self {
        Self { typ, value, parent: None }
    }
}

impl Expression for Literal {
    fn get_type(&self) -> Type {
        self.typ.clone()
    }
    fn set_parent(&mut self, parent: *const dyn Expression) {
        self.parent = Some(parent);
    }
    fn get_parent(&self) -> Option<*const dyn Expression> {
        self.parent
    }
}

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
    fn get_type(&self) -> Type {
        self.right.get_type()
    }
    fn set_parent(&mut self, parent: *const dyn Expression) {
        self.parent = Some(parent);
        let self_ptr = self as *const dyn Expression;
        self.right.set_parent(self_ptr);
    }
    fn get_parent(&self) -> Option<*const dyn Expression> {
        self.parent
    }
}

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
    fn get_type(&self) -> Type {
        self.right.get_type()
    }
    fn set_parent(&mut self, parent: *const dyn Expression) {
        self.parent = Some(parent);
        let self_ptr = self as *const dyn Expression;
        self.left.set_parent(self_ptr);
        self.right.set_parent(self_ptr);
    }
    fn get_parent(&self) -> Option<*const dyn Expression> {
        self.parent
    }
}

#[derive(Debug)]
pub struct Call {
    callee: Box<dyn Expression>,
    args: Vec<Box<dyn Expression>>,
    parent: Option<*const dyn Expression>,
}

impl Call {
    pub fn new(callee: Box<dyn Expression>, args: Vec<Box<dyn Expression>>) -> Result<Self, String> {
        // let (argtypes, return_type) = match callee.get_type() {
        //     Type::Function(argtypes, return_type) => (argtypes, *return_type),
        //     _ => return Err("Cannot call this expression of this type".to_string()),
        // };
        // if argtypes.len() != args.len() {
        //     return Err(format!("Wrong number of arguments; expected {} but got {}", argtypes.len(), args.len()));
        // }
        // if argtypes.iter().zip(args.iter()).any(|(a, b)| a != &b.get_type()) {
        //     return Err("Argument types do not match".to_string());
        // }
        Ok(Self { callee, args, parent: None })
    }
}

impl Expression for Call {
    fn get_type(&self) -> Type {
        match self.callee.get_type() {
            Type::Function(_, return_type) => {
                *return_type
            },
            _ => Type::Maybe, // this is not right; should be an error
        }
    }
    fn set_parent(&mut self, parent: *const dyn Expression) {
        self.parent = Some(parent);
        let self_ptr = self as *const dyn Expression;
        for e in self.args.iter_mut() {
            e.set_parent(self_ptr)
        }
        self.callee.set_parent(self_ptr);
    }
    fn get_parent(&self) -> Option<*const dyn Expression> {
        self.parent
    }
}

struct Variable {
    name: String,
    typ: Type,
    parent: Option<*const dyn Expression>,
}

struct Assignment {
    name: String,
    value: Box<dyn Expression>,
    parent: Option<*const dyn Expression>,
}

struct If {
    condition: Box<dyn Expression>,
    consequence: Block,
    alternative: Option<Block>,
    parent: Option<*const dyn Expression>,
}

struct Array {
    elements: Vec<Box<dyn Expression>>,
    parent: Option<*const dyn Expression>,
}

#[derive(Debug)]
pub struct Maybe {
    value: Option<Box<dyn Expression>>,
    parent: Option<*const dyn Expression>,
}

impl Maybe {
    pub fn new(value: Option<Box<dyn Expression>>) -> Self {
        Self { value, parent: None }
    }
}

impl Expression for Maybe {
    fn get_type(&self) -> Type {
        Type::Maybe
    }
    fn set_parent(&mut self, parent: *const dyn Expression) {
        self.parent = Some(parent);
        let self_ptr = self as *const dyn Expression;
        if let Some(value) = &mut self.value {
            value.set_parent(self_ptr)
        }
    }
    fn get_parent(&self) -> Option<*const dyn Expression> {
        self.parent
    }
}