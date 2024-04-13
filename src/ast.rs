use downcast_rs::{impl_downcast, Downcast};

use crate::compiler::{Compiler, TypeContext};
use crate::chunk::OpCode;
use crate::values::{HeapValue, Value};
use crate::token::TokenType;

// struct CompileError(String);


#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Type {
    Int,
    Float,
    String,
    Bool,
    Array(Box<Type>),
    Maybe(Box<Type>),
    Function(Vec<Type>, Box<Type>),
    Object(String, Vec<(String, Type)>)
}

impl Type {
    fn from_str(s: &str) -> Result<Self, String> {
        match s {
            "Int" => Ok(Self::Int),
            "Float" => Ok(Self::Float),
            "String" => Ok(Self::String),
            "Bool" => Ok(Self::Bool),
            // Cannot resolve compound types with this method
            _ => Err(format!("Unknown type {}", s)),
        }
    }
    pub fn is_heap(&self) -> bool {
        matches!(self, Self::String | Self::Array(_) | Self::Maybe(_) | Self::Function(_, _) | Self::Object(_, _))
    }
}

#[derive(Debug, Clone)]
pub struct VarType {
    name: String,
    typ: Type, 
}

impl VarType {
    pub fn new(name: String, typ: Type) -> Self {
        Self { name, typ }
    }
}

#[derive(Debug)]
pub struct NameAndType {
    name: String,
    typ: TypeAnnotation,
}

impl NameAndType {
    pub fn new(name: String, typ: TypeAnnotation) -> Self {
        Self { name, typ }
    }
    fn get_type(&self) -> Result<Type, String> {
        self.typ.get_type()
    }
}

pub trait Expression: std::fmt::Debug + Downcast {
    fn get_type(&self) -> Result<Type, String>;

    // set parent should set the parent for this expression,
    // then call set_parent on all of its children
    fn set_parent(&mut self, parent: Option<*const dyn Expression>);
    fn get_parent(&self) -> Option<*const dyn Expression>;
    // get a list of variables and their types that are defined in this expression
    // will stop looking if the given expression if reached
    #[allow(unused_variables)]
    fn find_vartype(&self, name: &String, upto: *const dyn Expression) -> Result<Option<Type>, String> {
        Ok(None)
    }

    fn compile(&self, compiler: &mut Compiler) -> Result<(), String>;
}

impl_downcast!(Expression);


#[derive(Debug)]
pub struct ErrorExpression;

impl Expression for ErrorExpression {
    fn get_type(&self) -> Result<Type, String> {
        Err("ErrorExpressions have no type".to_string()).unwrap()
    }
    fn set_parent(&mut self, _parent: Option<*const dyn Expression>) {
        // Do nothing
    }
    fn get_parent(&self) -> Option<*const dyn Expression> {
        None
    }
    fn compile(&self, _compiler: &mut Compiler) -> Result<(), String> {
        Err("Tried to compile an ErrorExpression".to_string())
    }
}

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
    fn set_parent(&mut self, _parent: Option<*const dyn Expression>) {
        let self_ptr = self as *const dyn Expression;
        self.child.set_parent(Some(self_ptr));
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
        compiler.write_opcode(
            if self.get_type()?.is_heap() {
                OpCode::ReturnHeap
            }
            else {
                OpCode::Return
            }
        );
        Ok(())
    }
}

#[derive(Debug)]
pub struct TypeAnnotation {
    typename: String,
    children: Vec<TypeAnnotation>,
}

impl TypeAnnotation {
    pub fn new(typename: String, children: Vec<TypeAnnotation>) -> Self {
        Self { typename, children }
    }

    fn get_type(&self) -> Result<Type, String> {
        if self.children.is_empty() {
            return Type::from_str(self.typename.as_str())
        }
        let child_types = self.children.iter().map(|a| a.get_type()).collect::<Result<Vec<Type>, String>>()?;
        match self.typename.as_str() {
            "Function" => {
                if child_types.len() < 1 {
                    return Err(format!(
                        "Function must be annotated with at least a return type"
                    ));
                }
                Ok(Type::Function(
                    child_types[..child_types.len()-1].to_vec(),
                    Box::new(child_types[child_types.len()-1].clone())
                ))
            },
            "Array" => {
                if child_types.len() != 1 {
                    return Err(format!(
                        "Array must be annotated with exactly one type, but got {:?}",
                        child_types
                    ));
                }
                Ok(Type::Array(Box::new(child_types[0].clone())))
            },
            "Maybe" => {
                if child_types.len() != 1 {
                    return Err(format!(
                        "Maybe must be annotated with exactly one type, but got {:?}",
                        child_types
                    ));
                }
                Ok(Type::Maybe(Box::new(child_types[0].clone())))
            },
            _ => unimplemented!("Unknown type annotation: {}", self.typename)
        }
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
    fn get_type(&self) -> Result<Type, String> {
        self.expressions.last().unwrap().get_type()
    }
    fn set_parent(&mut self, parent: Option<*const dyn Expression>) {
        self.parent = parent;
        let self_ptr = self as *const dyn Expression;
        for e in self.expressions.iter_mut() {
            e.set_parent(Some(self_ptr))
        }
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
}

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

    fn explicit_type(&self) -> Result<Option<Type>, String> {
        let return_type = match &self.rtype {
            None => return Ok(None),
            Some(rtype) => rtype.get_type()?,
        };
        let param_types = self.param_types()?;
        Ok(Some(Type::Function(param_types, Box::new(return_type))))
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
        Ok(Type::Function(param_types, Box::new(return_type)))
    }
    fn set_parent(&mut self, parent: Option<*const dyn Expression>) {
        self.parent = parent;
        let self_ptr = self as *const dyn Expression;
        self.block.set_parent(Some(self_ptr));
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
            Type::Function(ptypes, rtype) => (ptypes, *rtype),
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
        
        for param in self.params.iter() {
            if inner_compiler.create_variable(param.name.clone(), &param.get_type()?)?.is_some() {
                return Err(format!(
                    "Function parameters should be in local scope"
                ));
            }
        }

        self.block.compile(&mut inner_compiler)?;
        inner_compiler.write_opcode(
            if rtype.is_heap() {
                OpCode::ReturnHeap
            }
            else {
                OpCode::Return
            }
        );

        compiler.write_function(inner_compiler)
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
    fn get_type(&self) -> Result<Type, String> {
        Ok(self.typ.clone())
    }
    fn set_parent(&mut self, parent: Option<*const dyn Expression>) {
        self.parent = parent;
    }
    fn get_parent(&self) -> Option<*const dyn Expression> {
        self.parent
    }
    fn compile(&self, compiler: &mut Compiler) -> Result<(), String> {
        let value = match self.typ {
            Type::Int => Value::from_i64(self.value.parse::<i64>().unwrap()),
            Type::Float => Value::from_f64(self.value.parse::<f64>().unwrap()),
            Type::Bool => Value::from_bool(self.value.parse::<bool>().unwrap()),
            Type::String => {
                let string = self.value[1..self.value.len() - 1].to_string();
                return compiler.write_string(string);
            },
            _ => unimplemented!()
        };
        compiler.write_constant(value)
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
    fn get_type(&self) -> Result<Type, String> {
        self.right.get_type()
    }
    fn set_parent(&mut self, parent: Option<*const dyn Expression>) {
        self.parent = parent;
        let self_ptr = self as *const dyn Expression;
        self.right.set_parent(Some(self_ptr));
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
            x => return Err(format!("Type {:?} not yet supported for unary operation", x)),
        };
        Ok(())
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
    fn get_type(&self) -> Result<Type, String> {
        match self.op {
            TokenType::Eq
            | TokenType::NEq
            | TokenType::GEq
            | TokenType::LEq
            | TokenType::GT
            | TokenType::LT => Ok(Type::Bool),
            TokenType::To => Ok(Type::Array(Box::new(Type::Int))),
            _ => self.left.get_type(),
        }
    }
    fn set_parent(&mut self, parent: Option<*const dyn Expression>) {
        self.parent = parent;
        let self_ptr = self as *const dyn Expression;
        self.left.set_parent(Some(self_ptr));
        self.right.set_parent(Some(self_ptr));
    }
    fn get_parent(&self) -> Option<*const dyn Expression> {
        self.parent
    }

    fn compile(&self, compiler: &mut Compiler) -> Result<(), String> {
        let left_type = self.left.get_type()?;
        let right_type = self.right.get_type()?;
        if left_type != right_type {
            // todo! this isn't quite the right type checking for all operators
            return Err(format!(
                "Operands must be of the same type; got {:?} and {:?}",
                left_type, right_type
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
            Type::String => {
                compiler.write_opcode(match self.op {
                    TokenType::Eq => OpCode::ArrayEqual,
                    TokenType::NEq => OpCode::ArrayNotEqual,
                    TokenType::Plus => OpCode::Concat,
                    x => return Err(format!(
                        "Operator {:?} not supported for type {:?}",
                        x, left_type
                    ))
                })
            },
            x => return Err(format!(
                "Type {:?} not yet supported for binary operation", x
            ))
        };
        Ok(())
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
        Ok(Self { callee, args, parent: None })
    }
}

impl Expression for Call {
    fn get_type(&self) -> Result<Type, String> {
        match self.callee.get_type() {
            Ok(Type::Function(_, return_type)) => {
                Ok(*return_type)
            },
            Ok(ctype) => {
                Err(format!(
                    "Tried to call an expression of type {:?}, which is not callable", ctype
                ))
            },
            Err(e) => Err(format!(
                "Unable to resolve type of call: {}", e
            ))
        }
    }
    fn set_parent(&mut self, parent: Option<*const dyn Expression>) {
        self.parent = parent;
        let self_ptr = self as *const dyn Expression;
        for e in self.args.iter_mut() {
            e.set_parent(Some(self_ptr))
        }
        self.callee.set_parent(Some(self_ptr));
    }
    fn get_parent(&self) -> Option<*const dyn Expression> {
        self.parent
    }

    fn compile(&self, compiler: &mut Compiler) -> Result<(), String> {
        let callee_type = self.callee.get_type()?;
        let (paramtypes, _return_type) = match callee_type {
            Type::Function(argtypes, return_type) => (argtypes, *return_type),
            _ => return Err(format!(
                "Cannot call an expression of type {:?}", callee_type
            )),
        };
        if paramtypes.len() != self.args.len() {
            return Err(format!("Wrong number of arguments; expected {} but got {}", paramtypes.len(), self.args.len()));
        }
        let argtypes = self.args.iter().map(|e| e.get_type()).collect::<Result<Vec<_>, _>>()?;
        if paramtypes.iter().zip(argtypes.iter()).any(|(a, b)| a != b) {
            return Err(format!(
                "Argument types do not match; expected {:?} but got {:?}",
                paramtypes, argtypes
            ));
        }
        for arg in self.args.iter() {
            arg.compile(compiler)?;
        }
        self.callee.compile(compiler)?;
        compiler.write_opcode(OpCode::Call);
        Ok(())
    }
}

#[derive(Debug)]
pub struct Variable {
    name: String,
    parent: Option<*const dyn Expression>,
}

impl Variable {
    pub fn new(name: String) -> Self {
        Self { name, parent: None }
    }
}

impl Expression for Variable {
    fn get_type(&self) -> Result<Type, String> {
        // climb up the tree looking for a VarType with a matching name
        let mut e: *const dyn Expression = self;
        loop {
            let last_e = e;
            if let Some(parent) = unsafe { (*e).get_parent() } {
                e = parent;
            }
            else {
                // reached top of tree
                return Err(format!(
                    "Could not find definition for variable {}", self.name
                ));
            }
            let typ = unsafe { (*e).find_vartype(&self.name, last_e) };
            if let Some(typ) = typ? {
                return Ok(typ);
            }
        }
    }
    fn set_parent(&mut self, parent: Option<*const dyn Expression>) {
        self.parent = parent;
    }
    fn get_parent(&self) -> Option<*const dyn Expression> {
        self.parent
    }

    fn compile(&self, compiler: &mut Compiler) -> Result<(), String> {
        let is_heap = self.get_type()?.is_heap();
        compiler.get_variable(self.name.clone(), is_heap)
    }
}

#[derive(Debug)]
pub struct Assignment {
    name: String,
    value: Box<dyn Expression>,
    parent: Option<*const dyn Expression>,
}

impl Assignment {
    pub fn new(name: String, value: Box<dyn Expression>) -> Self {
        Self { name, value, parent: None }
    }
}

impl Expression for Assignment {
    fn get_type(&self) -> Result<Type, String> {
        self.value.get_type()
    }
    fn set_parent(&mut self, parent: Option<*const dyn Expression>) {
        self.parent = parent;
        let self_ptr = self as *const dyn Expression;
        self.value.set_parent(Some(self_ptr));
    }
    fn get_parent(&self) -> Option<*const dyn Expression> {
        self.parent
    }
    fn find_vartype(&self, name: &String, upto: *const dyn Expression) -> Result<Option<Type>, String> {
        if &self.name == name {
            if self.value.as_ref() as *const _ as *const () == upto as *const () {
                // recursive definition, not allowed except for annotated functions
                return match self.value.downcast_ref::<Function>() {
                    Some(f) => match f.explicit_type()? {
                        Some(t) => Ok(Some(t)),
                        None => Err(format!(
                            "Variable {} is defined recursively. This is allowed for functions, but the function must have an explicit return type annotation",
                            self.name
                        ))
                    },
                    None => Err(format!(
                        "Variable {} is defined recursively, which is not allowed for non-function types",
                        self.name
                    )),
                }
            }
            else {
                return Ok(Some(self.value.get_type()?));
            }
        }
        Ok(None)
    }

    fn compile(&self, compiler: &mut Compiler) -> Result<(), String> {
        let typ = self.value.get_type()?;
        let idx = compiler.create_variable(self.name.clone(), &typ)?;
        self.value.compile(compiler)?;
        compiler.set_variable(idx, typ.is_heap())
    }
}

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
    fn set_parent(&mut self, parent: Option<*const dyn Expression>) {
        self.parent = parent;
        let self_ptr = self as *const dyn Expression;
        self.condition.set_parent(Some(self_ptr));
        self.then_branch.set_parent(Some(self_ptr));
        if let Some(else_branch) = &mut self.else_branch {
            else_branch.set_parent(Some(self_ptr));
        }
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

#[derive(Debug)]
enum ArrayElems {
    Empty(Type),
    Elements(Vec<Box<dyn Expression>>),
}

#[derive(Debug)]
pub struct Array {
    elements: ArrayElems,
    parent: Option<*const dyn Expression>,
}

impl Array {
    pub fn new(elements: Vec<Box<dyn Expression>>) -> Self {
        Self { elements: ArrayElems::Elements(elements), parent: None }
    }
    pub fn new_empty(typ: TypeAnnotation) -> Result<Self, String> {
        let typ = typ.get_type()?;
        Ok(Self { elements: ArrayElems::Empty(typ), parent: None })
    }
}

impl Expression for Array {
    fn get_type(&self) -> Result<Type, String> {
        match &self.elements {
            ArrayElems::Empty(t) => Ok(Type::Array(Box::new(t.clone()))),
            ArrayElems::Elements(elems) => {
                let first_type = elems[0].get_type()?;
                for elem in elems.iter() {
                    let elem_type = elem.get_type()?;
                    if elem_type != first_type {
                        return Err(format!(
                            "Array elements have different types: {:?} and {:?}", first_type, elem_type
                        ));
                    }
                };
                Ok(Type::Array(Box::new(first_type)))
            }
        }
    }
    fn set_parent(&mut self, parent: Option<*const dyn Expression>) {
        self.parent = parent;
        let self_ptr = self as *const dyn Expression;
        if let ArrayElems::Elements(elems) = &mut self.elements {
            for elem in elems.iter_mut() {
                elem.set_parent(Some(self_ptr))
            }
        }
    }
    fn get_parent(&self) -> Option<*const dyn Expression> {
        self.parent
    }

    fn compile(&self, compiler: &mut Compiler) -> Result<(), String> {
        let len = match &self.elements {
            ArrayElems::Elements(elems) => {
                for elem in elems.iter() {
                    elem.compile(compiler)?;
                }
                elems.len() as u16
            },
            ArrayElems::Empty(_) => 0,
        };
        let typ = match self.get_type()? {
            Type::Array(t) => t,
            _ => unreachable!()
        };
        match typ.as_ref() {
            Type::Array(_) | Type::String => {
                compiler.write_array_array(len)
            },
            _ => compiler.write_array(len),
        }
    }
}

#[derive(Debug)]
pub struct TypeDef {
    name: String,
    fields: Vec<NameAndType>,
    parent: Option<*const dyn Expression>,
}

impl TypeDef {
    pub fn new(name: String, fields: Vec<NameAndType>) -> Self {
        Self { name, fields, parent: None }
    }

    fn field_types(&self) -> Result<Vec<(String, Type)>, String> {
        let mut field_types = Vec::new();
        for p in self.fields.iter() {
            field_types.push((p.name.clone(), p.get_type()?));
        }
        Ok(field_types)
    }
}

impl Expression for TypeDef {
    fn get_type(&self) -> Result<Type, String> {
        let field_types = self.field_types()?;
        let types_only = field_types.iter().map(|(_, t)| t.clone()).collect::<Vec<_>>();
        Ok(Type::Function(
            types_only,
            Box::new(Type::Object(self.name.clone(), field_types))
        ))
    }
    fn set_parent(&mut self, parent: Option<*const dyn Expression>) {
        self.parent = parent;
    }
    fn get_parent(&self) -> Option<*const dyn Expression> {
        self.parent
    }
    fn find_vartype(&self, name: &String, _upto: *const dyn Expression) -> Result<Option<Type>, String> {
        for f in self.fields.iter() {
            if &f.name == name {
                return Ok(Some(f.get_type()?));
            }
        }
        Ok(None)
    }

    fn compile(&self, compiler: &mut Compiler) -> Result<(), String> {
        unimplemented!("TypeDef::compile")
        // let fieldnames = self.fields.iter().map(|p| p.name.clone()).collect::<Vec<_>>();
        // let typedef = Value::TypeDef(Rc::new(
        //     values::TypeDef::new(self.name.clone(), fieldnames)
        // ));
        // compiler.write_constant(typedef)
    }
}

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
    fn set_parent(&mut self, parent: Option<*const dyn Expression>) {
        self.parent = parent;
        let self_ptr = self as *const dyn Expression;
        self.object.set_parent(Some(self_ptr));
    }
    fn get_parent(&self) -> Option<*const dyn Expression> {
        self.parent
    }

    fn compile(&self, compiler: &mut Compiler) -> Result<(), String> {
        let typ = self.get_type()?;  // todo: use this
        self.object.compile(compiler)?;
        compiler.write_string(self.field.clone())
    }
}

#[derive(Debug)]
enum MaybeValue {
    Some(Box<dyn Expression>),
    Null(Type),
}

#[derive(Debug)]
pub struct Maybe {
    value: MaybeValue,
    parent: Option<*const dyn Expression>,
}

impl Maybe {
    pub fn new_some(value: Box<dyn Expression>) -> Self {
        Self { value: MaybeValue::Some(value), parent: None }
    }
    pub fn new_null(typ: Type) -> Self {
        Self { value: MaybeValue::Null(typ), parent: None }
    }
}

impl Expression for Maybe {
    fn get_type(&self) -> Result<Type, String> {
        Ok(Type::Maybe(Box::new(match &self.value {
            MaybeValue::Some(e) => e.get_type()?,
            MaybeValue::Null(t) => t.clone(),
        })))
    }
    fn set_parent(&mut self, parent: Option<*const dyn Expression>) {
        self.parent = parent;
        let self_ptr = self as *const dyn Expression;
        if let MaybeValue::Some(value) = &mut self.value {
            value.set_parent(Some(self_ptr))
        }
    }
    fn get_parent(&self) -> Option<*const dyn Expression> {
        self.parent
    }

    fn compile(&self, compiler: &mut Compiler) -> Result<(), String> {
        match &self.value {
            MaybeValue::Some(e) => {
                e.compile(compiler)?;
                if e.get_type()?.is_heap() {
                    compiler.write_opcode(OpCode::WrapHeapSome);
                }
                else {
                    compiler.write_opcode(OpCode::WrapSome);
                }
                Ok(())
            },
            MaybeValue::Null(t) => {
                if t.is_heap() {
                    compiler.write_heap_constant(HeapValue::MaybeHeap(None))
                }
                else {
                    compiler.write_heap_constant(HeapValue::Maybe(None))
                }
            },
        }
    }
}
