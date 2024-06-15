use crate::ast;
use super::wasmtypes::*;

#[derive(Clone)]
pub struct StructField {
    pub t: ast::Type,
    pub nt: Numtype,
    pub offset: u32,
}

pub struct Struct {
    pub fields: Vec<(String, StructField)>,
    pub size: u32,
}

impl Struct {
    pub fn new(field_names_and_types: Vec<(String, ast::Type)>) -> Self {
        let mut offset = 0;
        let fields = field_names_and_types.into_iter().map(
            |(name, t)|
            {
                let nt = Numtype::from_ast_type(&t).unwrap();
                let field = StructField {
                    t, nt, offset,
                };
                offset += field.nt.size();
                (name, field)
            }
        ).collect();
        Self {
            fields,
            size: offset
        }
    }
}
