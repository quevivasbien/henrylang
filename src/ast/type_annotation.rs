use super::*;

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
            Type::TypeDef(_, t) => *t,
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
            _ => Err(format!("Unknown type annotation: {}", self.typename))
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
pub struct NameAndType {
    pub name: String,
    pub typ: TypeAnnotation,
}

impl NameAndType {
    pub fn new(name: String, typ: TypeAnnotation) -> Self {
        Self { name, typ }
    }
    pub fn get_type(&self) -> Result<Type, String> {
        self.typ.get_type()
    }
}
