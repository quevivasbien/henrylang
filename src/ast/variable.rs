use super::*;

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

    pub fn set_template_types(&mut self, template_params: Vec<Type>) -> Result<(), String> {
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

    fn wasmize(&self, wasmizer: &mut Wasmizer) -> Result<i32, String> {
        let name = self.get_expanded_name()?;
        // need to figure out whether or not the variable in question is a function within the function's definition
        // i.e., a recursive function call
        let mut name_matches_func = None;
        let mut parent = self.get_parent();
        while parent.is_some() {
            let e = unsafe { &*(parent.unwrap() as *const dyn Expression) };
            if let Some(func) = e.downcast_ref::<Function>() {
                let func_name = func.get_expanded_name()?;
                if func_name == name {
                    // figure out how many more functions will be defined before the referenced function is completed
                    name_matches_func = Some(func.count_function_chidren());
                    break;
                }
            }
            parent = e.get_parent();
        }
        wasmizer.get_variable(name, &self.get_type()?, name_matches_func)
    }
}
