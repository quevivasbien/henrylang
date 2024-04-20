use std::rc::Rc;

use downcast_rs::{impl_downcast, Downcast};

use crate::compiler::{Compiler, TypeContext};
use crate::chunk::OpCode;
use crate::values::{self, HeapValue, Value};
use crate::token::TokenType;

// struct CompileError(String);


#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Type {
    Int,
    Float,
    Str,
    Bool,
    Arr(Box<Type>),
    Iter(Box<Type>),
    Maybe(Box<Type>),
    Func(Vec<Type>, Box<Type>),
    Object(String, Vec<(String, Type)>)
}

impl Type {
    pub fn is_heap(&self) -> bool {
        matches!(self, Self::Str | Self::Arr(_) | Self::Iter(_) | Self::Maybe(_) | Self::Func(_, _) | Self::Object(_, _))
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

fn resolve_type(name: &String, origin: *const dyn Expression) -> Result<Type, String> {
    // climb up the tree looking for a VarType with a matching name
    let mut e = origin;
    loop {
        let last_e = e;
        if let Some(parent) = unsafe { (*e).get_parent() } {
            e = parent;
        }
        else {
            // reached top of tree
            return Err(format!(
                "Could not find definition for variable {}", name
            ));
        }
        let typ = unsafe { (*e).find_vartype(&name, last_e) };
        if let Some(typ) = typ? {
            return Ok(typ);
        }
    }
}

fn truncate_template_types(name: &String) -> &str {
    // cuts off anything that occurs after [ char
    name.split_terminator('[').nth(0).unwrap()
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
    fn set_parent(&mut self, parent: Option<*const dyn Expression>) -> Result<(), String>;
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
    fn set_parent(&mut self, _parent: Option<*const dyn Expression>) -> Result<(), String> {
        Ok(())
    }
    fn get_parent(&self) -> Option<*const dyn Expression> {
        None
    }
    fn compile(&self, _compiler: &mut Compiler) -> Result<(), String> {
        Err("Tried to compile an ErrorExpression".to_string())
    }
}


#[derive(Debug, PartialEq)]
pub struct TypeAnnotation {
    typename: String,
    children: Vec<TypeAnnotation>,
    parent: Option<*const dyn Expression>,
}

impl TypeAnnotation {
    pub fn new(typename: String, children: Vec<TypeAnnotation>) -> Self {
        Self { typename, children, parent: None }
    }

    fn resolve_typedef(&self) -> Result<Type, String> {
        let parent = match self.parent {
            Some(x) => x,
            None => return Err(format!("Could not resolve type for {}; parent is None", self.typename)),
        };
        let typ = resolve_type(&self.typename, parent)?;
        let objtype = match typ {
            Type::Func(_, t) => *t,
            _ => return Err(format!("When resolving type, expected an Object definition, but got {:?}", typ)),
        };
        if let Type::Object(n, _) = &objtype {
            debug_assert_eq!(n, &self.typename);
            Ok(objtype)
        }
        else {
            Err(format!("When resolving type, expected an Object definition, but got {:?}", objtype))
        }
    }
}

impl Expression for TypeAnnotation {
    fn get_type(&self) -> Result<Type, String> {
        if self.children.is_empty() {
            return match self.typename.as_str() {
                "Int" => Ok(Type::Int),
                "Float" => Ok(Type::Float),
                "Str" => Ok(Type::Str),
                "Bool" => Ok(Type::Bool),
                _ => self.resolve_typedef(),
            }
        }
        let child_types = self.children.iter().map(|a| a.get_type()).collect::<Result<Vec<Type>, String>>()?;
        match self.typename.as_str() {
            "Func" => {
                if child_types.len() < 1 {
                    return Err(format!(
                        "Function must be annotated with at least a return type"
                    ));
                }
                Ok(Type::Func(
                    child_types[..child_types.len()-1].to_vec(),
                    Box::new(child_types[child_types.len()-1].clone())
                ))
            },
            "Arr" => {
                if child_types.len() != 1 {
                    return Err(format!(
                        "Array must be annotated with exactly one type, but got {:?}",
                        child_types
                    ));
                }
                Ok(Type::Arr(Box::new(child_types[0].clone())))
            },
            "Iter" => {
                if child_types.len() != 1 {
                    return Err(format!(
                        "Iterator must be annotated with exactly one type, but got {:?}",
                        child_types
                    ));
                }
                Ok(Type::Iter(Box::new(child_types[0].clone())))
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
    
    fn set_parent(&mut self, parent: Option<*const dyn Expression>) -> Result<(), String> {
        self.parent = parent;
        let self_ptr = self as *const dyn Expression;
        for c in self.children.iter_mut() {
            c.set_parent(Some(self_ptr))?;
        }
        Ok(())
    }
    fn get_parent(&self) -> Option<*const dyn Expression> {
        self.parent
    }

    fn compile(&self, _compiler: &mut Compiler) -> Result<(), String> {
        Ok(())
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
        self.block.set_parent(Some(self_ptr))?;
        if let Some(rtype) = &mut self.rtype {
            rtype.set_parent(Some(self_ptr))?;
        }
        for param in self.params.iter_mut() {
            param.typ.set_parent(Some(self_ptr))?;
        }
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
    fn set_parent(&mut self, parent: Option<*const dyn Expression>) -> Result<(), String> {
        self.parent = parent;
        Ok(())
    }
    fn get_parent(&self) -> Option<*const dyn Expression> {
        self.parent
    }
    fn compile(&self, compiler: &mut Compiler) -> Result<(), String> {
        let value = match self.typ {
            Type::Int => Value::from_i64(self.value.parse::<i64>().unwrap()),
            Type::Float => Value::from_f64(self.value.parse::<f64>().unwrap()),
            Type::Bool => Value::from_bool(self.value.parse::<bool>().unwrap()),
            Type::Str => {
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

    fn argtypes(&self) -> Result<Vec<Type>, String> {
        self.args.iter().map(|e| e.get_type()).collect()
    }
}

impl Expression for Call {
    fn get_type(&self) -> Result<Type, String> {
        match self.callee.get_type() {
            Ok(Type::Func(_, return_type)) => {
                Ok(*return_type)
            },
            Ok(Type::Arr(typ)) => Ok(*typ),
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
    fn set_parent(&mut self, parent: Option<*const dyn Expression>) -> Result<(), String> {
        self.parent = parent;
        let self_ptr = self as *const dyn Expression;
        for e in self.args.iter_mut() {
            e.set_parent(Some(self_ptr))?;
        }
        self.callee.set_parent(Some(self_ptr))?;

        // when callee is a Variable, we need to do some special handling so the Variable knows what function signature to use, if it is a function
        let argtypes = self.argtypes();
        if let Some(var) = self.callee.downcast_mut::<Variable>() {
            // try and figure out if the variable refers to a function
            let vtype = var.get_type();
            if matches!(vtype, Ok(Type::Func(_, _)) | Err(_)) {
                var.set_template_types(argtypes?)?;
            }
        }
        Ok(())
    }
    fn get_parent(&self) -> Option<*const dyn Expression> {
        self.parent
    }

    fn compile(&self, compiler: &mut Compiler) -> Result<(), String> {
        let callee_type = self.callee.get_type()?;
        let (paramtypes, _return_type) = match callee_type {
            Type::Func(argtypes, return_type) => (argtypes, *return_type),
            Type::Arr(typ) => (vec![Type::Int], *typ),
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
    template_params: Vec<TypeAnnotation>,
    template_types: Vec<Type>,
    parent: Option<*const dyn Expression>,
}

impl Variable {
    pub fn new(name: String, template_params: Vec<TypeAnnotation>) -> Self {
        Self { name, template_params, template_types: vec![], parent: None }
    }

    fn set_template_types(&mut self, template_params: Vec<Type>) -> Result<(), String> {
        let my_params = self.template_params.iter().map(|a| a.get_type()).collect::<Result<Vec<_>, _>>()?;
        if !my_params.is_empty() && my_params != template_params {
            return Err(format!(
                "Template parameters do not match; expected {:?} but got {:?}",
                my_params, template_params
            ))
        }
        self.template_types = template_params;
        Ok(())
    }

    // get name, appending template types if any
    fn get_expanded_name(&self) -> Result<String, String> {
        Ok(if self.template_types.is_empty() {
            let template_types = self.template_params.iter().map(|a| a.get_type()).collect::<Result<Vec<_>, _>>()?;
            if template_types.is_empty() {
                self.name.clone()
            }
            else {
                format!("{}{:?}", self.name, template_types)
            }
        }
        else {
            format!("{}{:?}", self.name, self.template_types)
        })
    }
}

impl Expression for Variable {
    fn get_type(&self) -> Result<Type, String> {
        let name = self.get_expanded_name()?;
        resolve_type(&name, self)
    }
    fn set_parent(&mut self, parent: Option<*const dyn Expression>) -> Result<(), String> {
        self.parent = parent;
        Ok(())
    }
    fn get_parent(&self) -> Option<*const dyn Expression> {
        self.parent
    }

    fn compile(&self, compiler: &mut Compiler) -> Result<(), String> {
        let is_heap = self.get_type()?.is_heap();
        let name = self.get_expanded_name()?;
        compiler.get_variable(name, is_heap)
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

    fn handle_recursive_def(&self) -> Result<Type, String> {
        // recursive definition, not allowed except for annotated functions
        return match self.value.downcast_ref::<Function>() {
            Some(f) => match f.explicit_type()? {
                Some(t) => Ok(t),
                None => Err(format!(
                    "Variable {} is defined recursively. This is allowed for functions, but the function must have an explicit return type annotation",
                    self.name
                ))
            },
            None => Err(format!(
                "Variable {} is defined recursively, which is not allowed for non-function types or functions with no arguments",
                self.name
            )),
        }
    } 
}

impl Expression for Assignment {
    fn get_type(&self) -> Result<Type, String> {
        self.value.get_type()
    }
    fn set_parent(&mut self, parent: Option<*const dyn Expression>) -> Result<(), String> {
        self.parent = parent;
        let self_ptr = self as *const dyn Expression;
        self.value.set_parent(Some(self_ptr))
    }
    fn get_parent(&self) -> Option<*const dyn Expression> {
        self.parent
    }
    fn find_vartype(&self, name: &String, upto: *const dyn Expression) -> Result<Option<Type>, String> {
        let name_truncated = truncate_template_types(name);
        let no_template = name_truncated == name;
        if no_template {
            if &self.name == name {
                if self.value.as_ref() as *const _ as *const () == upto as *const () {
                    return Err(format!(
                        "Variable {} is defined recursively, which is not allowed for non-function types or functions with no arguments",
                        self.name
                    ));
                }
                else {
                    return Ok(Some(self.value.get_type()?));
                }
            }
            return Ok(None);
        }
        // name has template types attached (presumed to be a function)
        // first check if self.name matches name without template types
        if &self.name != &name_truncated {
            return Ok(None);
        }
        // figure out function type so we can determine expanded name of self
        let ftype = if self.value.as_ref() as *const _ as *const () == upto as *const () {
            self.handle_recursive_def()?
        }
        else {
            self.value.get_type()?
        };
        let argtypes = match &ftype {
            Type::Func(argtypes, _) => argtypes,
            _ => unreachable!("Expected function type"),
        };
        let expanded_name = format!("{}{:?}", self.name, argtypes);
        if &expanded_name == name {
            return Ok(Some(ftype))
        }
        Ok(None)
    }

    fn compile(&self, compiler: &mut Compiler) -> Result<(), String> {
        let typ = self.value.get_type()?;
        let name = match &typ {
            Type::Func(paramtypes, _) => if paramtypes.is_empty() {
                self.name.clone()
            }
            else {
                format!("{}{:?}", self.name, paramtypes)
            },
            // Type::Arr(t) => format!("{}[{}]", self.name, t),
            _ => self.name.clone(),
        };
        let idx = compiler.create_variable(name, &typ)?;
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

#[derive(Debug)]
enum ArrayElems {
    Empty(TypeAnnotation),
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
        Ok(Self { elements: ArrayElems::Empty(typ), parent: None })
    }
}

impl Expression for Array {
    fn get_type(&self) -> Result<Type, String> {
        match &self.elements {
            ArrayElems::Empty(t) => Ok(Type::Arr(Box::new(t.get_type()?))),
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
                Ok(Type::Arr(Box::new(first_type)))
            }
        }
    }
    fn set_parent(&mut self, parent: Option<*const dyn Expression>) -> Result<(), String> {
        self.parent = parent;
        let self_ptr = self as *const dyn Expression;
        match &mut self.elements {
            ArrayElems::Elements(elems) => {
                for elem in elems.iter_mut() {
                    elem.set_parent(Some(self_ptr))?;
                }
            }
            ArrayElems::Empty(t) => {
                t.set_parent(Some(self_ptr))?;
            },
        }
        Ok(())
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
            Type::Arr(t) => t,
            _ => unreachable!()
        };
        if typ.is_heap() {
            compiler.write_array_heap(len)
        }
        else {
            compiler.write_array(len)
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
        Ok(Type::Func(
            types_only,
            Box::new(Type::Object(self.name.clone(), field_types))
        ))
    }
    fn set_parent(&mut self, parent: Option<*const dyn Expression>) -> Result<(), String> {
        self.parent = parent;
        let self_ptr = self as *const dyn Expression;
        for field in self.fields.iter_mut() {
            field.typ.set_parent(Some(self_ptr))?;
        }
        Ok(())
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
        let mut fields = Vec::new();
        for f in self.fields.iter() {
            fields.push((f.name.clone(), f.get_type()?.is_heap()));
        }
        let typedef = HeapValue::TypeDef(Rc::new(
            values::TypeDef::new(self.name.clone(), fields)
        ));
        compiler.write_heap_constant(typedef)
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
    fn set_parent(&mut self, parent: Option<*const dyn Expression>) -> Result<(), String> {
        self.parent = parent;
        let self_ptr = self as *const dyn Expression;
        self.object.set_parent(Some(self_ptr))
    }
    fn get_parent(&self) -> Option<*const dyn Expression> {
        self.parent
    }

    fn compile(&self, compiler: &mut Compiler) -> Result<(), String> {
        let typ = self.get_type()?;
        compiler.write_constant(Value { b: typ.is_heap() })?;
        compiler.write_string(self.field.clone())?;
        self.object.compile(compiler)?;
        compiler.write_opcode(OpCode::Call);
        Ok(())
    }
}

#[derive(Debug)]
enum MaybeValue {
    Some(Box<dyn Expression>),
    Null(TypeAnnotation),
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
    pub fn new_null(typ: TypeAnnotation) -> Self {
        Self { value: MaybeValue::Null(typ), parent: None }
    }
}

impl Expression for Maybe {
    fn get_type(&self) -> Result<Type, String> {
        Ok(Type::Maybe(Box::new(match &self.value {
            MaybeValue::Some(e) => e.get_type()?,
            MaybeValue::Null(t) => t.get_type()?,
        })))
    }
    fn set_parent(&mut self, parent: Option<*const dyn Expression>) -> Result<(), String> {
        self.parent = parent;
        let self_ptr = self as *const dyn Expression;
        match &mut self.value {
            MaybeValue::Some(value) => value.set_parent(Some(self_ptr)),
            MaybeValue::Null(t) => t.set_parent(Some(self_ptr)),
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
                if t.get_type()?.is_heap() {
                    compiler.write_heap_constant(HeapValue::MaybeHeap(None))
                }
                else {
                    compiler.write_heap_constant(HeapValue::Maybe(None))
                }
            },
        }
    }
}

#[derive(Debug)]
pub struct Unwrap {
    value: Box<dyn Expression>,
    default: Box<dyn Expression>,
    parent: Option<*const dyn Expression>,
}

impl Unwrap {
    pub fn new(value: Box<dyn Expression>, default: Box<dyn Expression>) -> Self {
        Self { value, default, parent: None }
    }
}

impl Expression for Unwrap {
    fn get_type(&self) -> Result<Type, String> {
        let inner_type = match self.value.get_type()? {
            Type::Maybe(t) => *t,
            x => return Err(format!("Unwrap expected Maybe, got {:?}", x))
        };
        let default_type = self.default.get_type()?;
        if inner_type != default_type {
            return Err(format!("Unwrap default does not match inner type, got default {:?} and inner type {:?}", default_type, inner_type));
        }
        Ok(inner_type)
    }
    fn set_parent(&mut self, parent: Option<*const dyn Expression>) -> Result<(), String> {
        self.parent = parent;
        let self_ptr = self as *const dyn Expression;
        self.value.set_parent(Some(self_ptr))?;
        self.default.set_parent(Some(self_ptr))?;
        Ok(())
    }
    fn get_parent(&self) -> Option<*const dyn Expression> {
        self.parent
    }

    fn compile(&self, compiler: &mut Compiler) -> Result<(), String> {
        let typ = self.get_type()?;
        self.default.compile(compiler)?;
        self.value.compile(compiler)?;
        if typ.is_heap() {
            compiler.write_opcode(OpCode::UnwrapHeap);
        }
        else {
            compiler.write_opcode(OpCode::Unwrap);
        }
        Ok(())
    }
}


#[derive(Debug)]
pub struct Map {
    left: Box<dyn Expression>,
    right: Box<dyn Expression>,
    parent: Option<*const dyn Expression>,
}

impl Map {
    pub fn new(left: Box<dyn Expression>, right: Box<dyn Expression>) -> Self {
        Self { left, right, parent: None }
    }
}

impl Expression for Map {
    fn get_type(&self) -> Result<Type, String> {
        let left_type = self.left.get_type()?;
        match left_type {
            Type::Func(_, typ) => Ok(Type::Iter(Box::new(*typ))),
            Type::Arr(typ) => Ok(Type::Iter(Box::new(*typ))),
            Type::Str => Ok(Type::Iter(Box::new(Type::Str))),
            typ => Err(format!("Cannot use '->' on type {:?}", typ))
        }
    }

    fn set_parent(&mut self, parent: Option<*const dyn Expression>) -> Result<(), String> {
        self.parent = parent;
        let self_ptr = self as *const dyn Expression;
        self.left.set_parent(Some(self_ptr))?;
        self.right.set_parent(Some(self_ptr))?;

        // name to do same special handling for left side here that we do for callee in Call expression
        let rtype = match self.right.get_type()? {
            Type::Iter(t) | Type::Arr(t) => *t,
            _ => return Err(format!("Cannot use '->' with type {:?} on right", self.right.get_type()?)),
        };
        if let Some(var) = self.left.downcast_mut::<Variable>() {
            let vtype = var.get_type();
            if matches!(vtype, Ok(Type::Func(_, _)) | Err(_)) {
                var.set_template_types(vec![rtype])?;
            }
        }

        Ok(())
    }

    fn get_parent(&self) -> Option<*const dyn Expression> {
        self.parent
    }

    fn compile(&self, compiler: &mut Compiler) -> Result<(), String> {
        let left_type = self.left.get_type()?;
        let right_type = self.right.get_type()?;

        self.left.compile(compiler)?;
        self.right.compile(compiler)?;

        if let Type::Iter(arr_type) | Type::Arr(arr_type) = &right_type {
            match &left_type {
                Type::Arr(_) => {
                    if arr_type.as_ref() != &Type::Int {
                        return Err(format!(
                            "Cannot map from type {:?} using non-integer type {:?}",
                            left_type, arr_type
                        ));
                    }
                },
                Type::Func(arg_type, _) => {
                    if arg_type.len() != 1 {
                        return Err(format!("Cannot map with a function does not have a single argument; got a function with {} arguments", arg_type.len()));
                    }
                    if &arg_type[0] != arr_type.as_ref() {
                        return Err(format!("Function used for mapping must have an argument of type {:?} to match the array mapped over; got {:?}", arr_type, arg_type[0]));
                    }
                },
                typ => return Err(format!("Cannot map with non-callable type {:?}", typ)),
            }
            compiler.write_opcode(OpCode::Map);
            return Ok(());
        }
        return Err(format!("Operand on right of '->' must be an array type; got {:?}", right_type));
    }
}

#[derive(Debug)]
pub struct Reduce {
    function: Box<dyn Expression>,
    array: Box<dyn Expression>,
    init: Box<dyn Expression>,
    parent: Option<*const dyn Expression>,
}

impl Reduce {
    pub fn new(function: Box<dyn Expression>, array: Box<dyn Expression>, init: Box<dyn Expression>) -> Self {
        Self { function, array, init, parent: None }
    }
}

impl Expression for Reduce {
    fn get_type(&self) -> Result<Type, String> {
        let (func_arg_type, func_ret_type) = match self.function.get_type()? {
            Type::Func(arg, ret) => (arg, *ret),
            x => return Err(format!(
                "Reduce function must be a function; got a {:?}", x
            )),
        };
        if func_arg_type.len() != 2 {
            return Err(format!(
                "Reduce function must take two arguments; got {:?}", func_arg_type
            ))
        };
        let func_arg_type = func_arg_type[0].clone();
        let array_type = match self.array.get_type()? {
            Type::Arr(x) | Type::Iter(x) => *x,
            x => return Err(format!(
                "Second argument of reduce must be an array or iterator; got a {:?}", x
            ))
        };
        if array_type != func_arg_type {
            return Err(format!(
                "Reduce function argument and array must have the same type; got {:?} and {:?}", func_arg_type, array_type
            ));
        }
        let init_type = self.init.get_type()?;
        if func_ret_type != init_type {
            return Err(format!(
                "Reduce function return and init must have the same type; got {:?} and {:?}", func_ret_type, init_type
            ));
        }
        Ok(func_ret_type)
    }
    fn set_parent(&mut self, parent: Option<*const dyn Expression>) -> Result<(), String> {
        self.parent = parent;
        let self_ptr = self as *const dyn Expression;
        self.function.set_parent(Some(self_ptr))?;
        self.array.set_parent(Some(self_ptr))?;
        self.init.set_parent(Some(self_ptr))?;

        // name to do same special handling for function that we do for callee in Call expression
        let inittype = self.init.get_type()?;
        let arrtype = match self.array.get_type()? {
            Type::Arr(t) | Type::Iter(t) => *t,
            _ => return Err(format!("Cannot use '->' with type {:?} on right", self.array.get_type()?)),
        };
        if let Some(var) = self.function.downcast_mut::<Variable>() {
            var.set_template_types(vec![inittype, arrtype])?;
        }
        Ok(())
    }
    fn get_parent(&self) -> Option<*const dyn Expression> {
        self.parent
    }

    fn compile(&self, compiler: &mut Compiler) -> Result<(), String> {
        self.init.compile(compiler)?;
        self.array.compile(compiler)?;
        self.function.compile(compiler)?;
        compiler.write_opcode(OpCode::Reduce);
        Ok(())
    }
}

#[derive(Debug)]
pub struct Filter {
    pub function: Box<dyn Expression>,
    pub array: Box<dyn Expression>,
    pub parent: Option<*const dyn Expression>,
}

impl Filter {
    pub fn new(function: Box<dyn Expression>, array: Box<dyn Expression>) -> Self {
        Self { function, array, parent: None }
    }
}

impl Expression for Filter {
    fn get_type(&self) -> Result<Type, String> {
        let (func_arg_type, func_ret_type) = match self.function.get_type()? {
            Type::Func(arg, ret) => (arg, *ret),
            x => return Err(format!(
                "Filter function must be a function; got a {:?}", x
            )),
        };
        if func_arg_type.len() != 1 {
            return Err(format!(
                "Filter function must take one argument; got {:?}", func_arg_type
            ));
        }
        let func_arg_type = func_arg_type[0].clone();
        if func_ret_type != Type::Bool {
            return Err(format!(
                "Filter function must return a bool; got {:?}", func_ret_type
            ));
        }
        let array_type = match self.array.get_type()? {
            Type::Arr(x) | Type::Iter(x) => *x,
            x => return Err(format!(
                "Second filter argumetn must be an array or iterator; got a {:?}", x
            ))
        };
        if array_type != func_arg_type {
            return Err(format!(
                "Filter function argument and array must have the same type; got {:?} and {:?}", func_arg_type, array_type
            ));
        }
        Ok(Type::Iter(Box::new(array_type.clone())))
    }
    fn set_parent(&mut self, parent: Option<*const dyn Expression>) -> Result<(), String> {
        self.parent = parent;
        let self_ptr = self as *const dyn Expression;
        self.function.set_parent(Some(self_ptr))?;
        self.array.set_parent(Some(self_ptr))?;

        // name to do same special handling for function that we do for callee in Call expression
        let arrtype = match self.array.get_type()? {
            Type::Arr(t) | Type::Iter(t) => *t,
            _ => return Err(format!("Cannot use '->' with type {:?} on right", self.array.get_type()?)),
        };
        if let Some(var) = self.function.downcast_mut::<Variable>() {
            var.set_template_types(vec![arrtype])?;
        }
        Ok(())
    }
    fn get_parent(&self) -> Option<*const dyn Expression> {
        self.parent
    }

    fn compile(&self, compiler: &mut Compiler) -> Result<(), String> {
        self.function.compile(compiler)?;
        self.array.compile(compiler)?;
        compiler.write_opcode(OpCode::Filter);
        Ok(())
    }
}

#[derive(Debug)]
pub struct Len {
    expr: Box<dyn Expression>,
    parent: Option<*const dyn Expression>,
}

impl Len {
    pub fn new(expr: Box<dyn Expression>) -> Self {
        Self { expr, parent: None }
    }
}

impl Expression for Len {
    fn get_type(&self) -> Result<Type, String> {
        match self.expr.get_type()? {
            Type::Arr(_) | Type::Str => Ok(Type::Int),
            x => Err(format!(
                "Len expression must be an array or string; got a {:?}", x
            )),
        }
    }
    fn set_parent(&mut self, parent: Option<*const dyn Expression>) -> Result<(), String> {
        self.parent = parent;
        let self_ptr = self as *const dyn Expression;
        self.expr.set_parent(Some(self_ptr))
    }
    fn get_parent(&self) -> Option<*const dyn Expression> {
        self.parent
    }

    fn compile(&self, compiler: &mut Compiler) -> Result<(), String> {
        self.get_type()?;  // just to check that inner type is an array
        self.expr.compile(compiler)?;
        compiler.write_opcode(OpCode::Len);
        Ok(())
    }
}


#[derive(Debug)]
pub struct ZipMap {
    function: Box<dyn Expression>,
    exprs: Vec<Box<dyn Expression>>,
    parent: Option<*const dyn Expression>,
}

impl ZipMap {
    pub fn new(fn_expr: Box<dyn Expression>, exprs: Vec<Box<dyn Expression>>) -> Self {
        Self { function: fn_expr, exprs, parent: None }
    }
}

impl Expression for ZipMap {
    fn get_type(&self) -> Result<Type, String> {
        let (func_arg_types, func_ret_type) = match self.function.get_type()? {
            Type::Func(arg, ret) => (arg, *ret),
            x => return Err(format!(
                "ZipMap function must be a function; got a {:?}", x
            ))
        };
        let mut expr_types = Vec::new();
        for expr in self.exprs.iter() {
            match expr.get_type()? {
                Type::Arr(t) | Type::Iter(t) => expr_types.push(*t),
                x => return Err(format!(
                    "ZipMap expression must be an array or iterator; got a {:?}", x
                ))
            }
        }
        if func_arg_types != expr_types {
            return Err(format!(
                "ZipMap function argument and arrays must have matching types; got {:?} and {:?}", func_arg_types, expr_types
            ));
        }
        Ok(Type::Iter(Box::new(func_ret_type)))
    }
    fn set_parent(&mut self, parent: Option<*const dyn Expression>) -> Result<(), String> {
        self.parent = parent;
        let self_ptr = self as *const dyn Expression;
        self.function.set_parent(Some(self_ptr))?;
        for expr in self.exprs.iter_mut() {
            expr.set_parent(Some(self_ptr))?;
        }

        // name to do same special handling for function that we do for callee in Call expression
        let argtypes = self.exprs.iter()
            .map(|expr| {
                match expr.get_type() {
                    Ok(Type::Arr(t) | Type::Iter(t)) => Ok(*t),
                    t => Err(format!("Cannot use zipmap with type {:?}", t)),
                }
            })
            .collect::<Result<Vec<Type>, String>>()?;
        if let Some(var) = self.function.downcast_mut::<Variable>() {
            var.set_template_types(argtypes)?;
        }
        Ok(())
    }
    fn get_parent(&self) -> Option<*const dyn Expression> {
        self.parent
    }

    fn compile(&self, compiler: &mut Compiler) -> Result<(), String> {
        self.get_type()?;  // check that types are all in order
        for expr in self.exprs.iter() {
            expr.compile(compiler)?;
        }
        compiler.write_constant(Value { i: self.exprs.len() as i64 })?;
        self.function.compile(compiler)?;
        compiler.write_opcode(OpCode::ZipMap);
        Ok(())
    }
}