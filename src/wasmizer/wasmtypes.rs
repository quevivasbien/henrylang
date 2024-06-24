use crate::ast;

const FUNCTYPE: u8 = 0x60;

pub enum SectionType {
    Type = 0x01,
    Import = 0x02,
    Function = 0x03,
    Table = 0x04,
    Memory = 0x05,
    Global = 0x06,
    Export = 0x07,
    Element = 0x09,
    Code = 0x0a,
    Data = 0x0b,
    DataCount = 0x0c,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Numtype {
    Void = 0x40,
    F32 = 0x7d,
    I64 = 0x7e,
    I32 = 0x7f,
}

impl Numtype {
    pub fn from_ast_type(typ: &ast::Type) -> Result<Self, String> {
        match typ {
            ast::Type::Int => Ok(Self::I32),
            ast::Type::Bool => Ok(Self::I32),
            ast::Type::Float => Ok(Self::F32),
            ast::Type::Func(..) => Ok(Self::I32), // functions are referred to by their table indices
            ast::Type::TypeDef(..) => Ok(Self::I32), // TypeDef returns a constructor function
            // Heap types are referred to by a fat pointer containing memory loc and length
            ast::Type::Arr(_) => Ok(Self::I64),
            ast::Type::Str => Ok(Self::I64),
            ast::Type::Iter(_) => Ok(Self::I64),
            ast::Type::Object(..) => Ok(Self::I64),
            _ => Err(format!("Cannot convert type {:?} to WASM Numtype", typ)),
        }
    }

    // Return the size of the type in bytes
    pub fn size(&self) -> u32 {
        match self {
            Self::Void => 0,
            Self::F32 | Self::I32 => 4,
            Self::I64 => 8,
        }
    }
}

impl std::fmt::Display for Numtype {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Void => write!(f, "Void"),
            Self::F32 => write!(f, "F32"),
            Self::I32 => write!(f, "I32"),
            Self::I64 => write!(f, "I64"),
        }
    }
}

#[derive(Clone, Copy)]
pub enum ExportType {
    Func = 0x00,
    Memory = 0x02,
}

#[derive(Clone, Copy)]
pub enum Opcode {
    Block = 0x02,
    Loop = 0x03,
    If = 0x04,
    Else = 0x05,
    End = 0x0b,
    BrIf = 0x0d,
    Call = 0x10,
    CallIndirect = 0x11,
    Drop = 0x1a,
    LocalGet = 0x20,
    LocalSet = 0x21,
    LocalTee = 0x22,
    GlobalGet = 0x23,
    GlobalSet = 0x24,
    I32Load = 0x28,
    I64Load = 0x29,
    F32Load = 0x2a,
    I32Store = 0x36,
    I64Store = 0x37,
    F32Store = 0x38,
    I32Const = 0x41,
    I64Const = 0x42,
    F32Const = 0x43,
    I32Eqz = 0x45,
    I32Eq = 0x46,
    I32Ne = 0x47,
    I32LtS = 0x48,
    I32LtU = 0x49,
    I32GtS = 0x4a,
    I32GtU = 0x4b,
    I32LeS = 0x4c,
    // I32LeU = 0x4d,
    I32GeS = 0x4e,
    I32GeU = 0x4f,
    F32Eq = 0x5b,
    F32Ne = 0x5c,
    F32Lt = 0x5d,
    F32Gt = 0x5e,
    F32Le = 0x5f,
    F32Ge = 0x60,
    I32Add = 0x6a,
    I32Sub = 0x6b,
    I32Mul = 0x6c,
    I32DivS = 0x6d,
    I32RemU = 0x70,
    I32And = 0x71,
    I32Or = 0x72,
    I64Add = 0x7c,
    F32Neg = 0x8c,
    I64Shl = 0x86,
    I64ShrU = 0x88,
    F32Add = 0x92,
    F32Sub = 0x93,
    F32Mul = 0x94,
    F32Div = 0x95,
    I32WrapI64 = 0xa7,
    I64ExtendI32U = 0xad,
}

pub const MEMINIT: [u8; 2] = [0xfc, 0x08];
pub const MEMCOPY: [u8; 4] = [0xfc, 0x0a, 0x00, 0x00];

pub fn unsigned_leb128(value: u32) -> Vec<u8> {
    let mut result = Vec::new();
    let mut value = value;
    loop {
        // take low 7 bits of remaining value
        let mut byte = (value & 0x7f) as u8;
        value >>= 7;
        // if there is still something remaining, set the top bit to 1
        if value != 0 {
            byte |= 0x80;
        }
        result.push(byte);
        // if there's nothing left, we're done
        if value == 0 {
            break;
        }
    }
    result
}

pub fn signed_leb128(value: i32) -> Vec<u8> {
    let mut result = Vec::new();
    let mut more = true;
    let mut value = value;
    while more {
        // take low 7 bits of remaining value
        let mut byte = (value & 0x7f) as u8;
        value >>= 7;
        // if value is zero and sign bit is 0, we're done
        // also if value is -1 and sign bit is 1, we're done
        if (value == 0 && (byte & 0x40) == 0) || (value == -1 && (byte & 0x40) != 0) {
            more = false;
        } else {
            byte |= 0x80;
        }
        result.push(byte);
    }
    result
}

pub fn encode_string(s: &str) -> Vec<u8> {
    vector(s.as_bytes().to_vec())
}

pub fn vector(mut data: Vec<u8>) -> Vec<u8> {
    let mut result = unsigned_leb128(data.len() as u32);
    result.append(&mut data);
    result
}

pub fn section_from_chunks(section_type: SectionType, chunks: &[Vec<u8>]) -> Vec<u8> {
    let mut result = vec![section_type as u8];
    let data = [unsigned_leb128(chunks.len() as u32), chunks.concat()].concat();
    result.append(&mut vector(data));
    result
}

pub fn section_from_values(section_type: SectionType, values: &[u32]) -> Vec<u8> {
    let mut result = vec![section_type as u8];
    let mut data = unsigned_leb128(values.len() as u32);
    for value in values {
        data.append(&mut unsigned_leb128(*value));
    }
    result.append(&mut vector(data));
    result
}

// defines a type for a function
// includes types of arguments and of the return value(s)
fn function_type(args: Vec<u8>, ret: Vec<u8>) -> Vec<u8> {
    let mut result = vec![FUNCTYPE];
    result.append(&mut vector(args));
    result.append(&mut vector(ret));
    result
}

pub fn function_body(local_types: Vec<u8>, mut code: Vec<u8>) -> Vec<u8> {
    let mut result = unsigned_leb128(local_types.len() as u32);
    for ltype in local_types {
        result.push(0x01); // count of locals with this type
        result.push(ltype);
    }
    result.append(&mut code);
    vector(result)
}

#[derive(PartialEq, Eq, Clone, Debug)]
pub struct FuncTypeSignature {
    pub args: Vec<Numtype>,
    pub ret: Option<Numtype>,
}

impl Default for FuncTypeSignature {
    fn default() -> Self {
        Self {
            args: vec![],
            ret: None,
        }
    }
}

impl FuncTypeSignature {
    pub fn new(args: Vec<Numtype>, ret: Option<Numtype>) -> Self {
        Self { args, ret }
    }
    pub fn from_ast_type(typ: &ast::Type) -> Result<Self, String> {
        let (args, ret) = match typ {
            ast::Type::Func(args, ret) | ast::Type::TypeDef(args, ret) => (args, ret),
            _ => {
                return Err(format!(
                    "Cannot convert type {:?} to WASM FuncTypeSignature",
                    typ
                ))
            }
        };
        let args = args
            .iter()
            .map(|x| Numtype::from_ast_type(x))
            .collect::<Result<_, _>>()?;
        let ret = Numtype::from_ast_type(ret)?;
        Ok(Self::new(args, Some(ret)))
    }
    // get byte representation
    pub fn as_functype(&self) -> Vec<u8> {
        let ret = match self.ret {
            Some(x) => vec![x as u8],
            None => vec![],
        };
        function_type(self.args.iter().map(|x| *x as u8).collect(), ret)
    }
}
