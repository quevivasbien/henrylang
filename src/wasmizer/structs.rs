use super::wasmtypes::*;
use crate::ast;

#[derive(Clone)]
pub struct StructField {
    pub nt: Numtype,
    pub offset: u32,
}

pub struct Struct {
    pub fields: Vec<(String, StructField)>,
    pub size: u32,
}

impl Struct {
    pub fn new(field_names_and_types: Vec<(String, Numtype)>) -> Self {
        Self::new_from_iter(field_names_and_types.into_iter())
    }

    pub fn from_ast_types(field_names_and_types: Vec<(String, ast::Type)>) -> Self {
        let iter = field_names_and_types
            .into_iter()
            .map(|(name, t)| (name, Numtype::from_ast_type(&t).unwrap()));
        Self::new_from_iter(iter)
    }

    fn new_from_iter(field_names_and_types: impl Iterator<Item = (String, Numtype)>) -> Self {
        let mut offset = 0;
        let fields = field_names_and_types
            .map(|(name, nt)| {
                let field = StructField { nt, offset };
                offset += field.nt.size();
                (name, field)
            })
            .collect();
        Self {
            fields,
            size: offset,
        }
    }
}
