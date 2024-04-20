use crate::chunk::OpCode;

use super::*;

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
