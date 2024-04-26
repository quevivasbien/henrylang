use crate::values::Value;

use super::*;

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

    fn wasmize(&self, wasmizer: &mut Wasmizer) -> Result<(), String> {
        wasmizer.write_const(&self.value, &self.typ)
    }
}
