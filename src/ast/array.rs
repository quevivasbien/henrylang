use crate::chunk::OpCode;

use super::*;

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

    fn wasmize(&self, wasmizer: &mut Wasmizer) -> Result<i32, String> {
        let len = match &self.elements {
            ArrayElems::Elements(elems) => {
                for elem in elems.iter().rev() {
                    elem.wasmize(wasmizer)?;
                }
                elems.len() as u16
            },
            ArrayElems::Empty(_) => 0,
        };
        let typ = match self.get_type()? {
            Type::Arr(t) => t,
            _ => unreachable!()
        };
        wasmizer.write_array(len, &typ)?;
        Ok(0)
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
            Type::Arr(_) | Type::Iter(_) | Type::Str => Ok(Type::Int),
            x => Err(format!(
                "Len expression must be an array, iterator, or string; got a {:?}", x
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
        self.get_type()?;  // just to check that type is valid
        self.expr.compile(compiler)?;
        compiler.write_opcode(OpCode::Len);
        Ok(())
    }

    fn wasmize(&self, wasmizer: &mut Wasmizer) -> Result<i32, String> {
        let expr_type = self.expr.get_type()?;
        self.expr.wasmize(wasmizer)?;
        wasmizer.write_len(&expr_type)?;
        Ok(0)
    }
}
