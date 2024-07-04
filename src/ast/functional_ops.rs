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
        Self {
            left,
            right,
            parent: None,
        }
    }

    // Get the types associated with this expression
    // Returns the inner type of the result, the inner type of the object iterated over, and if that object is an array
    // Return format is (result_inner_type, input_inner_type, is_array)
    fn get_type_info(&self) -> Result<(Type, Type, bool), String> {
        let left_type = self.left.get_type()?;
        let right_type = self.right.get_type()?;

        let (input_inner_type, input_is_array) = match &right_type {
            Type::Iter(arr_type) => (*arr_type.clone(), false),
            Type::Arr(arr_type) => (*arr_type.clone(), true),
            _ => {
                return Err(format!(
                    "Operand on right of '->' must be an iterator or array type; got {:?}",
                    right_type
                ));
            }
        };

        let result_inner_type = match &left_type {
            Type::Arr(result_type) => {
                if input_inner_type != Type::Int {
                    return Err(format!(
                        "Cannot map from type {:?} using non-integer type {:?}",
                        left_type, input_inner_type
                    ));
                }
                *result_type.clone()
            }
            Type::Func(arg_type, result_type) => {
                if arg_type.len() != 1 {
                    return Err(format!("Cannot map with a function that does not have a single argument; got a function with {} arguments", arg_type.len()));
                }
                if arg_type[0] != input_inner_type {
                    return Err(format!("Function used for mapping must have an argument of type {:?} to match the array mapped over; got {:?}", input_inner_type, arg_type[0]));
                }
                *result_type.clone()
            }
            typ => return Err(format!("Cannot map with type {:?}", typ)),
        };

        Ok((result_inner_type, input_inner_type, input_is_array))
    }
}

impl Expression for Map {
    fn get_type(&self) -> Result<Type, String> {
        let (result_inner_type, _, _) = self.get_type_info()?;
        Ok(Type::Iter(Box::new(result_inner_type)))
    }

    fn set_parent(&mut self, parent: Option<*const dyn Expression>) -> Result<(), String> {
        self.parent = parent;
        let self_ptr = self as *const dyn Expression;
        self.left.set_parent(Some(self_ptr))?;
        self.right.set_parent(Some(self_ptr))?;

        // name to do same special handling for left side here that we do for callee in Call expression
        let rtype = match self.right.get_type()? {
            Type::Iter(t) | Type::Arr(t) => *t,
            _ => {
                return Err(format!(
                    "Cannot use '->' with type {:?} on right",
                    self.right.get_type()?
                ))
            }
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
        self.get_type_info()?; // check that types are all in order
        self.left.compile(compiler)?;
        self.right.compile(compiler)?;

        compiler.write_opcode(OpCode::Map);
        Ok(())
    }

    fn wasmize(&self, wasmizer: &mut Wasmizer) -> Result<i32, String> {
        let (result_inner_type, input_inner_type, input_is_array) = self.get_type_info()?;

        self.left.wasmize(wasmizer)?;
        self.right.wasmize(wasmizer)?;

        match self.left.get_type()? {
            Type::Arr(_) => {
                // TODO
                return Err(format!(
                    "Mapping from array is not yet implemented in WASM mode"
                ));
            }
            Type::Func(..) => {
                wasmizer.write_map(&result_inner_type, &input_inner_type, input_is_array)?;
            }
            typ => return Err(format!("Cannot map with type {:?}", typ)),
        }
        return Ok(0);
    }
}

#[derive(Debug)]
pub struct Reduce {
    function: Box<dyn Expression>,
    iter_over: Box<dyn Expression>,
    init: Box<dyn Expression>,
    parent: Option<*const dyn Expression>,
}

impl Reduce {
    pub fn new(
        function: Box<dyn Expression>,
        iter_over: Box<dyn Expression>,
        init: Box<dyn Expression>,
    ) -> Self {
        Self {
            function,
            iter_over,
            init,
            parent: None,
        }
    }

    // get the types of the result, the type contained in the array or iterator object iterated over, and whether that object is an array (as opposed to an iterator)
    // returns (result_type, array_type, is_array)
    fn get_type_info(&self) -> Result<(Type, Type, bool), String> {
        let (func_arg_types, func_ret_type) = match self.function.get_type()? {
            Type::Func(arg, ret) => (arg, *ret),
            x => return Err(format!("Reduce function must be a function; got a {:?}", x)),
        };
        if func_arg_types.len() != 2 {
            return Err(format!(
                "Reduce function must take two arguments; got {:?}",
                func_arg_types
            ));
        };
        let acc_type = func_arg_types[0].clone();
        let x_type = func_arg_types[1].clone();
        let iter_over_type = self.iter_over.get_type()?;
        let iter_over_is_array = matches!(iter_over_type, Type::Arr(_));
        let iter_over_inner_type = match iter_over_type {
            Type::Arr(x) | Type::Iter(x) => *x,
            x => {
                return Err(format!(
                    "Second argument of reduce must be an array or iterator; got a {:?}",
                    x
                ))
            }
        };
        if iter_over_inner_type != x_type {
            return Err(format!(
                "Second argument of reduce function and array must have the same type; got {:?} and {:?}", x_type, iter_over_inner_type
            ));
        }
        let init_type = self.init.get_type()?;
        if func_ret_type != init_type || func_ret_type != acc_type {
            return Err(format!(
                "First argument of reduce funtion, reduce function return value, and initial value must all have the same type; got {:?}, {:?}, and {:?}", acc_type, func_ret_type, init_type
            ));
        }
        Ok((acc_type, x_type, iter_over_is_array))
    }
}

impl Expression for Reduce {
    fn get_type(&self) -> Result<Type, String> {
        let (acc_type, _, _) = self.get_type_info()?;
        Ok(acc_type)
    }
    fn set_parent(&mut self, parent: Option<*const dyn Expression>) -> Result<(), String> {
        self.parent = parent;
        let self_ptr = self as *const dyn Expression;
        self.function.set_parent(Some(self_ptr))?;
        self.iter_over.set_parent(Some(self_ptr))?;
        self.init.set_parent(Some(self_ptr))?;

        // name to do same special handling for function that we do for callee in Call expression
        let inittype = self.init.get_type()?;
        let iter_over_type = match self.iter_over.get_type()? {
            Type::Arr(t) | Type::Iter(t) => *t,
            _ => {
                return Err(format!(
                    "Cannot use '->' with type {:?} on right",
                    self.iter_over.get_type()?
                ))
            }
        };
        if let Some(var) = self.function.downcast_mut::<Variable>() {
            var.set_template_types(vec![inittype, iter_over_type])?;
        }
        Ok(())
    }
    fn get_parent(&self) -> Option<*const dyn Expression> {
        self.parent
    }

    fn compile(&self, compiler: &mut Compiler) -> Result<(), String> {
        let _ = self.get_type()?; // check that types are all in order
        self.init.compile(compiler)?;
        self.iter_over.compile(compiler)?;
        self.function.compile(compiler)?;
        compiler.write_opcode(OpCode::Reduce);
        Ok(())
    }

    fn wasmize(&self, wasmizer: &mut Wasmizer) -> Result<i32, String> {
        let (acc_type, x_type, use_array_iter) = self.get_type_info()?;
        self.function.wasmize(wasmizer)?;
        self.init.wasmize(wasmizer)?;
        self.iter_over.wasmize(wasmizer)?;
        wasmizer.write_reduce(&acc_type, &x_type, use_array_iter)?;
        return Ok(0);
    }
}

#[derive(Debug)]
pub struct Filter {
    pub function: Box<dyn Expression>,
    pub iter_over: Box<dyn Expression>,
    pub parent: Option<*const dyn Expression>,
}

impl Filter {
    pub fn new(function: Box<dyn Expression>, iter_over: Box<dyn Expression>) -> Self {
        Self {
            function,
            iter_over,
            parent: None,
        }
    }

    // Gets the type of contained in the result iterator, as well as if the input iterator is an array
    // returns (result_type, is_array)
    fn get_type_info(&self) -> Result<(Type, bool), String> {
        let (func_arg_type, func_ret_type) = match self.function.get_type()? {
            Type::Func(arg, ret) => (arg, *ret),
            x => return Err(format!("Filter function must be a function; got a {:?}", x)),
        };
        if func_arg_type.len() != 1 {
            return Err(format!(
                "Filter function must take one argument; got {:?}",
                func_arg_type
            ));
        }
        let func_arg_type = func_arg_type[0].clone();
        if func_ret_type != Type::Bool {
            return Err(format!(
                "Filter function must return a bool; got {:?}",
                func_ret_type
            ));
        }
        let iter_over_type = self.iter_over.get_type()?;
        let is_array = matches!(iter_over_type, Type::Arr(_));
        let inner_type = match iter_over_type {
            Type::Arr(x) | Type::Iter(x) => *x,
            x => {
                return Err(format!(
                    "Second filter argument must be an array or iterator; got a {:?}",
                    x
                ))
            }
        };
        if inner_type != func_arg_type {
            return Err(format!(
                "Filter function argument and array must have the same type; got {:?} and {:?}",
                func_arg_type, inner_type
            ));
        }
        Ok((inner_type, is_array))
    }
}

impl Expression for Filter {
    fn get_type(&self) -> Result<Type, String> {
        let (inner_type, _) = self.get_type_info()?;
        Ok(Type::Iter(Box::new(inner_type)))
    }
    fn set_parent(&mut self, parent: Option<*const dyn Expression>) -> Result<(), String> {
        self.parent = parent;
        let self_ptr = self as *const dyn Expression;
        self.function.set_parent(Some(self_ptr))?;
        self.iter_over.set_parent(Some(self_ptr))?;

        // name to do same special handling for function that we do for callee in Call expression
        let arrtype = match self.iter_over.get_type()? {
            Type::Arr(t) | Type::Iter(t) => *t,
            _ => {
                return Err(format!(
                    "Cannot use '->' with type {:?} on right",
                    self.iter_over.get_type()?
                ))
            }
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
        let _ = self.get_type()?; // check that types are all in order
        self.function.compile(compiler)?;
        self.iter_over.compile(compiler)?;
        compiler.write_opcode(OpCode::Filter);
        Ok(())
    }

    fn wasmize(&self, wasmizer: &mut Wasmizer) -> Result<i32, String> {
        let (typ, use_array_iter) = self.get_type_info()?;
        self.function.wasmize(wasmizer)?;
        self.iter_over.wasmize(wasmizer)?;
        wasmizer.write_filter(&typ, use_array_iter)?;
        return Ok(0);
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
        Self {
            function: fn_expr,
            exprs,
            parent: None,
        }
    }

    // get the inner type of the result, the inner types of each of the objects iterated over, and whether each of the objects iterated over are arrays.
    // returns (result_type, iter_over_types, iter_overs_are_arrays)
    fn get_type_info(&self) -> Result<(Type, Vec<Type>, Vec<bool>), String> {
        let (func_arg_types, func_ret_type) = match self.function.get_type()? {
            Type::Func(arg, ret) | Type::TypeDef(arg, ret) => (arg, *ret),
            x => {
                return Err(format!(
                    "ZipMap function must be a function or type definition; got a {:?}",
                    x
                ))
            }
        };
        let mut iter_over_types = Vec::new();
        let mut iter_overs_are_arrays = Vec::new();
        for expr in self.exprs.iter() {
            let typ = expr.get_type()?;
            iter_overs_are_arrays.push(matches!(typ, Type::Arr(_)));
            match typ {
                Type::Arr(t) | Type::Iter(t) => iter_over_types.push(*t),
                x => {
                    return Err(format!(
                        "ZipMap expression must be an array or iterator; got a {:?}",
                        x
                    ))
                }
            }
        }
        if func_arg_types != iter_over_types {
            return Err(format!(
                "ZipMap function argument and arrays must have matching types; got {:?} and {:?}",
                func_arg_types, iter_over_types
            ));
        }

        Ok((func_ret_type, iter_over_types, iter_overs_are_arrays))
    }
}

impl Expression for ZipMap {
    fn get_type(&self) -> Result<Type, String> {
        let (func_ret_type, _, _) = self.get_type_info()?;
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
        let argtypes = self
            .exprs
            .iter()
            .map(|expr| match expr.get_type() {
                Ok(Type::Arr(t) | Type::Iter(t)) => Ok(*t),
                t => Err(format!("Cannot use zipmap with type {:?}", t)),
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
        self.get_type()?; // check that types are all in order
        for expr in self.exprs.iter() {
            expr.compile(compiler)?;
        }
        compiler.write_constant(Value {
            i: self.exprs.len() as i64,
        })?;
        self.function.compile(compiler)?;
        compiler.write_opcode(OpCode::ZipMap);
        Ok(())
    }

    fn wasmize(&self, wasmizer: &mut Wasmizer) -> Result<i32, String> {
        let (func_ret_type, iter_over_types, iter_overs_are_arrays) = self.get_type_info()?;

        self.function.wasmize(wasmizer)?;
        for ((expr, is_array), inner_type) in self
            .exprs
            .iter()
            .zip(iter_overs_are_arrays.into_iter())
            .zip(iter_over_types.iter())
        {
            expr.wasmize(wasmizer)?;
            if is_array {
                wasmizer.make_array_iter(inner_type)?;
            }
        }
        wasmizer.write_zipmap(&func_ret_type, &iter_over_types)?;
        return Ok(0);
    }
}
