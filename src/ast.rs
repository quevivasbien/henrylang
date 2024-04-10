use std::rc::Rc;

use crate::compiler::Compiler;
use crate::chunk::OpCode;
use crate::values;
use crate::values::Value;
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
            // todo: resolve compound types
            _ => Err(format!("Unknown type {}", s)),
        }
    }
}

#[derive(Debug, Clone)]
pub struct VarType {
    pub name: String,
    pub typ: Type, 
}

impl VarType {
    pub fn new(name: String, typ: Type) -> Self {
        Self { name, typ }
    }
}

pub trait Expression: std::fmt::Debug {
    fn get_type(&self) -> Result<Type, String>;

    // set parent should set the parent for this expression,
    // then call set_parent on all of its children
    fn set_parent(&mut self, parent: Option<*const dyn Expression>);
    fn get_parent(&self) -> Option<*const dyn Expression>;
    // get a list of variables and their types that are defined in this expression
    // will stop looking if the given expression if reached
    fn vartypes(&self, _upto: *const dyn Expression) -> Vec<VarType> {
        vec![]
    }

    fn compile(&self, compiler: &mut Compiler) -> Result<(), String>;
}

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
    fn compile(&self, compiler: &mut Compiler) -> Result<(), String> {
        Err("Tried to compile an ErrorExpression".to_string())
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
    fn vartypes(&self, upto: *const dyn Expression) -> Vec<VarType> {
        let mut out = Vec::new();
        for e in self.expressions.iter() {
            if e.as_ref() as *const _ as *const () == upto as *const () {
                break;
            }
            out.extend(e.vartypes(upto));
        }
        out
    }

    fn compile(&self, compiler: &mut Compiler) -> Result<(), String> {
        compiler.begin_scope();
        for expr in self.expressions.iter() {
            expr.compile(compiler)?;
            compiler.write_opcode(OpCode::EndExpr);
        }
        compiler.end_scope()
    }
}

#[derive(Debug)]
pub struct NameAndType {
    name: String,
    typename: String,
    parent: Option<*const dyn Expression>,
}

impl NameAndType {
    pub fn new(name: String, typename: String) -> Self {
        Self { name, typename, parent: None }
    }
    fn vartypes(&self, _upto: *const dyn Expression) -> Vec<VarType> {
        vec![VarType::new(self.name.clone(), self.get_type().unwrap())]
    }
}

impl Expression for NameAndType {
    fn get_type(&self) -> Result<Type, String> {
        Type::from_str(self.typename.as_str())
    }
    fn set_parent(&mut self, parent: Option<*const dyn Expression>) {
        self.parent = parent;
    }
    fn get_parent(&self) -> Option<*const dyn Expression> {
        self.parent
    }
    fn vartypes(&self, _upto: *const dyn Expression) -> Vec<VarType> {
        vec![VarType::new(self.name.clone(), self.get_type().unwrap())]
    }

    fn compile(&self, _compiler: &mut Compiler) -> Result<(), String> {
        panic!("NameAndType shouldn't be compiled")
    }
}

#[derive(Debug)]
pub struct Function {
    name: String,
    params: Vec<NameAndType>,
    pub block: Box<dyn Expression>,
    parent: Option<*const dyn Expression>,
}

impl Function {
    pub fn new(name: String, params: Vec<NameAndType>, block: Box<dyn Expression>) -> Self {
        Self { name, params, block, parent: None }
    }

    fn param_types(&self) -> Result<Vec<Type>, String> {
        let mut param_types = Vec::new();
        for p in self.params.iter() {
            param_types.push(p.get_type()?);
        }
        Ok(param_types)
    }
    fn param_vartypes(&self, upto: *const dyn Expression) -> Vec<VarType> {
        self.params.iter().map(|p| p.vartypes(upto)).flatten().collect()
    }
}

impl Expression for Function {
    fn get_type(&self) -> Result<Type, String> {
        let param_types = self.param_types()?;
        let return_type = self.block.get_type()?;
        Ok(Type::Function(param_types, Box::new(return_type)))
    }
    fn set_parent(&mut self, parent: Option<*const dyn Expression>) {
        self.parent = parent;
        let self_ptr = self as *const dyn Expression;
        self.params.iter_mut().for_each(|p| p.set_parent(Some(self_ptr)));
        self.block.set_parent(Some(self_ptr));
    }
    fn get_parent(&self) -> Option<*const dyn Expression> {
        self.parent
    }
    fn vartypes(&self, upto: *const dyn Expression) -> Vec<VarType> {
        // vartypes in block should have been already processed, since block is a child of function
        self.param_vartypes(upto)
    }

    fn compile(&self, compiler: &mut Compiler) -> Result<(), String> {
        let mut inner_compiler = Compiler::new(compiler);
        inner_compiler.function.name = self.name.clone();
        inner_compiler.function.arity = self.params.len() as u8;
        for param in self.params.iter() {
            if inner_compiler.create_variable(param.name.clone())?.is_some() {
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
    fn set_parent(&mut self, parent: Option<*const dyn Expression>) {
        self.parent = parent;
    }
    fn get_parent(&self) -> Option<*const dyn Expression> {
        self.parent
    }
    fn compile(&self, compiler: &mut Compiler) -> Result<(), String> {
        let value = match self.typ {
            Type::Int => Value::Int(self.value.parse::<i64>().unwrap()),
            Type::Float => Value::Float(self.value.parse::<f64>().unwrap()),
            Type::Bool => Value::Bool(self.value.parse::<bool>().unwrap()),
            Type::String => Value::String(Rc::new(self.value.clone())),
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
        compiler.write_opcode(match self.op {
            TokenType::Minus => OpCode::Negate,
            TokenType::Bang => OpCode::Not,
            _ => unreachable!(),
        });
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
        self.right.get_type()
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
        compiler.write_opcode(match self.op {
            TokenType::Eq => OpCode::Equal,
            TokenType::NEq => OpCode::NotEqual,
            TokenType::GT => OpCode::Greater,
            TokenType::GEq => OpCode::GreaterEqual,
            TokenType::LT => OpCode::Less,
            TokenType::LEq => OpCode::LessEqual,
            TokenType::Plus => OpCode::Add,
            TokenType::Minus => OpCode::Subtract,
            TokenType::Star => OpCode::Multiply,
            TokenType::Slash => OpCode::Divide,
            TokenType::And => OpCode::And,
            TokenType::Or => OpCode::Or,
            TokenType::To => OpCode::To,
            TokenType::RightArrow => OpCode::Map,
            _ => unreachable!(),
        });
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
        let (paramtypes, return_type) = match callee_type {
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
        compiler.write_call()
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
            let vartypes = unsafe { (*e).vartypes(last_e) };
            for vartype in vartypes.into_iter() {
                if vartype.name == self.name {
                    return Ok(vartype.typ);
                }
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
        compiler.get_variable(self.name.clone())
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
    fn vartypes(&self, upto: *const dyn Expression) -> Vec<VarType> {
        if self.value.as_ref() as *const _ as *const () == upto as *const () {
            return vec![];
        }
        vec![VarType::new(self.name.clone(), self.value.get_type().unwrap())]
    }

    fn compile(&self, compiler: &mut Compiler) -> Result<(), String> {
        let idx = compiler.create_variable(self.name.clone())?;
        self.value.compile(compiler)?;
        compiler.set_variable(idx)
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
        let then_branch_type = self.then_branch.get_type()?;
        match &self.else_branch {
            None => Ok(then_branch_type),
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
        let typ = self.get_type()?; // will error if types don't match
        self.condition.compile(compiler)?;
        let jump_if_idx = compiler.write_jump(OpCode::JumpIfFalse)?;
        self.then_branch.compile(compiler)?;
        if self.else_branch.is_none() {
            compiler.write_opcode(OpCode::WrapSome);
        }
        let jump_else_idx = compiler.write_jump(OpCode::Jump)?;
        compiler.patch_jump(jump_if_idx)?;
        if let Some(else_branch) = &self.else_branch {
            else_branch.compile(compiler)?;
        }
        else {
            compiler.write_constant(Value::Maybe(Box::new(None)))?;
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
    pub fn new_empty(typename: String) -> Result<Self, String> {
        let typ = Type::from_str(typename.as_str())?;
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
        let typ = self.get_type()?;  // todo: use this
        match &self.elements {
            ArrayElems::Elements(elems) => {
                for elem in elems.iter() {
                    elem.compile(compiler)?;
                }
                compiler.write_array(elems.len() as u16)
            },
            ArrayElems::Empty(_) => compiler.write_array(0),
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
    fn field_vartypes(&self, upto: *const dyn Expression) -> Vec<VarType> {
        self.fields.iter().map(|p| p.vartypes(upto)).flatten().collect()
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
        let self_ptr = self as *const dyn Expression;
        for p in self.fields.iter_mut() {
            p.set_parent(Some(self_ptr))
        }
    }
    fn get_parent(&self) -> Option<*const dyn Expression> {
        self.parent
    }
    fn vartypes(&self, upto: *const dyn Expression) -> Vec<VarType> {
        self.field_vartypes(upto)
    }

    fn compile(&self, compiler: &mut Compiler) -> Result<(), String> {
        let fieldnames = self.fields.iter().map(|p| p.name.clone()).collect::<Vec<_>>();
        let typedef = Value::TypeDef(Rc::new(
            values::TypeDef::new(self.name.clone(), fieldnames)
        ));
        compiler.write_constant(typedef)
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
        let name = Value::String(Rc::new(
            self.field.clone()
        ));
        compiler.write_constant(name)
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
        match &self.value {
            MaybeValue::Some(e) => e.get_type(),
            MaybeValue::Null(t) => Ok(t.clone()),
        }
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
                compiler.write_opcode(OpCode::WrapSome);
                Ok(())
            },
            MaybeValue::Null(_) => compiler.write_constant(
                Value::Maybe(Box::new(None))
            ),
        }
    }
}
