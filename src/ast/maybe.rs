use crate::{chunk::OpCode, values::HeapValue};

use super::*;

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

    fn wasmize(&self, wasmizer: &mut Wasmizer) -> Result<i32, String> {
        match &self.value {
            MaybeValue::Some(e) => {
                e.wasmize(wasmizer)?;
                wasmizer.write_some(&e.get_type()?)?;
            },
            MaybeValue::Null(t) => {
                wasmizer.write_null(&t.get_type()?)?;
            },
        }
        Ok(0)
    }
}

#[derive(Debug)]
pub struct IsSome {
    value: Box<dyn Expression>,
    parent: Option<*const dyn Expression>,
}

impl IsSome {
    pub fn new(value: Box<dyn Expression>) -> Self {
        Self { value, parent: None }
    }
}

impl Expression for IsSome {
    fn get_type(&self) -> Result<Type, String> {
        Ok(Type::Bool)
    }
    fn set_parent(&mut self, parent: Option<*const dyn Expression>) -> Result<(), String> {
        self.parent = parent;
        let self_ptr = self as *const dyn Expression;
        self.value.set_parent(Some(self_ptr))
    }
    fn get_parent(&self) -> Option<*const dyn Expression> {
        self.parent
    }

    fn compile(&self, compiler: &mut Compiler) -> Result<(), String> {
        // check that value is a Maybe
        if !matches!(self.value.get_type()?, Type::Maybe(_)) {
            return Err(format!("IsSome expected Maybe type, got {:?}", self.value.get_type()?));
        }
        self.value.compile(compiler)?;
        compiler.write_opcode(OpCode::IsSome);
        Ok(())
    }

    fn wasmize(&self, wasmizer: &mut Wasmizer) -> Result<i32, String> {
        // check that value is a Maybe, and get inner type
        let inner_type = match self.value.get_type()? {
            Type::Maybe(t) => *t,
            x => return Err(format!("IsSome expected Maybe type, got {:?}", x))
        };
        self.value.wasmize(wasmizer)?;
        wasmizer.write_is_some(&inner_type)?;
        Ok(0)
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

    fn wasmize(&self, wasmizer: &mut Wasmizer) -> Result<i32, String> {
        self.value.wasmize(wasmizer)?;
        self.default.wasmize(wasmizer)?;
        wasmizer.write_unwrap(&self.get_type()?)?;
        
        Ok(0)
    }
}   
