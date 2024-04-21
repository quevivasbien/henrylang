use crate::{chunk::OpCode, values::Value};

use super::*;

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
                typ => return Err(format!("Cannot map with type {:?}", typ)),
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
            Type::Func(arg, ret) | Type::TypeDef(arg, ret) => (arg, *ret),
            x => return Err(format!(
                "ZipMap function must be a function or type definition; got a {:?}", x
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
            let vartype = var.get_type();
            if !matches!(vartype, Ok(Type::TypeDef(..))) {
                var.set_template_types(argtypes)?;
            }
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
