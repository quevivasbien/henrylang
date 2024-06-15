use super::*;

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
            _ => self.name.clone(),
        };
        let idx = compiler.create_variable(name, &typ)?;
        self.value.compile(compiler)?;
        compiler.set_variable(idx, typ.is_heap())
    }

    fn wasmize(&self, wasmizer: &mut Wasmizer) -> Result<i32, String> {
        let typ = self.value.get_type()?;
        let name = match &typ {
            Type::Func(paramtypes, _) => if paramtypes.is_empty() {
                self.name.clone()
            }
            else {
                format!("{}{:?}", self.name, paramtypes)
            },
            _ => self.name.clone(),
        };
        let idx = wasmizer.create_variable(name, &typ)?;
        self.value.wasmize(wasmizer)?;
        wasmizer.set_variable(idx, &typ)?;
        Ok(0)
    }
}
