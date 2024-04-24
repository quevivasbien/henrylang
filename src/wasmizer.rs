use crate::{ast, compiler::TypeContext, parser, scanner};

const MAGIC: [u8; 4] = [0x00, 0x61, 0x73, 0x6d];
const VERSION: [u8; 4] = [0x01, 0x00, 0x00, 0x00];

const FUNCTYPE: u8 = 0x60;

enum SectionType {
    Type = 0x01,
    Function = 0x03,
    Table = 0x04,
    Export = 0x07,
    Element = 0x09,
    Code = 0x0a,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Numtype {
    I32 = 0x7f,
}

enum ExportType {
    Func = 0x00,
}

enum Opcode {
    End = 0x0b,
    Call = 0x10,
    LocalGet = 0x20,
    I32Const = 0x41,
    I32Add = 0x6a,
    I32Sub = 0x6b,
    I32Mul = 0x6c,
    I32DivS = 0x6d
}

fn unsigned_leb128(value: u32) -> Vec<u8> {
    let mut result = Vec::new();
    let mut value = value;
    loop {
        let mut byte = (value & 0x7f) as u8;
        value >>= 7;
        if value != 0 {
            byte |= 0x80;
        }
        result.push(byte);
        if value == 0 {
            break;
        }
    }
    result
}

fn signed_leb128(value: i32) -> Vec<u8> {
    unsigned_leb128(value as u32)
}

fn encode_string(s: &str) -> Vec<u8> {
    let mut result = vec![s.len() as u8];
    result.append(&mut s.as_bytes().to_vec());
    result
}

fn vector(mut data: Vec<u8>) -> Vec<u8> {
    let mut result = unsigned_leb128(data.len() as u32);
    result.append(&mut data);
    result
}

fn section_from_chunks(section_type: SectionType, chunks: &[Vec<u8>]) -> Vec<u8> {
    let mut result = vec![section_type as u8];
    let data = [
        unsigned_leb128(chunks.len() as u32),
        chunks.concat()
    ].concat();
    result.append(&mut vector(data));
    result
}

fn section_from_values(section_type: SectionType, values: &[u8]) -> Vec<u8> {
    let mut result = vec![section_type as u8];
    let data = [
        unsigned_leb128(values.len() as u32).as_slice(),
        values
    ].concat();
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

fn function_body(n_locals: u8, mut code: Vec<u8>) -> Vec<u8> {
    let mut result = vec![n_locals];
    result.append(&mut code);
    vector(result)
}

#[derive(PartialEq, Eq, Clone)]
struct FuncTypeSignature {
    args: Vec<Numtype>,
    ret: Numtype, 
}

impl Default for FuncTypeSignature {
    fn default() -> Self {
        Self { args: vec![], ret: Numtype::I32 }
    }
}

impl FuncTypeSignature {
    fn new(args: Vec<Numtype>, ret: Numtype) -> Self {
        Self { args, ret }
    }
    // get byte representation
    fn as_functype(&self) -> Vec<u8> {
        function_type(
            self.args.iter().map(|x| *x as u8).collect(),
            vec![self.ret as u8]
        )
    }
}

// a value to be exported; currently only works for functions
struct Export {
    name: String,
    // index of function within funcs
    func_idx: u8,
}

impl Export {
    fn new(name: String, func_idx: u8) -> Self {
        Self { name, func_idx }
    }

    fn as_export(&self) -> Vec<u8> {
        let mut result = encode_string(&self.name);
        result.push(ExportType::Func as u8);
        result.push(self.func_idx);
        result
    }
}

struct ModuleBuilder {
    // stores type signatures for each function
    functypes: Vec<FuncTypeSignature>,
    // indices to functypes for each function
    funcs: Vec<u8>,
    // bytecode for each function's body - order should match that of funcs
    func_bodies: Vec<Vec<u8>>,
    // information about functions to export
    exports: Vec<Export>,
}

impl Default for ModuleBuilder {
    fn default() -> Self {
        Self {
            functypes: Vec::new(),
            funcs: vec![],
            func_bodies: vec![],
            exports: vec![],
        }
    }
}

impl ModuleBuilder {
    fn add_function(&mut self, sig: &FuncTypeSignature, n_locals: u8, code: Vec<u8>, export_name: Option<String>) -> Result<(), String> {
        let ftype = match self.functypes.iter().enumerate()
            .find_map(|(i, x)| if x == sig { Some(i) } else { None })
        {
            Some(i) => i as u8,
            None => {
                self.functypes.push(sig.clone());
                self.functypes.len() as u8 - 1
            }
        };
        let func_idx = self.funcs.len() as u8;
        if func_idx == u8::MAX {
            return Err(format!("too many functions"));
        }
        self.funcs.push(ftype);
        self.func_bodies.push(function_body(n_locals, code));
        if let Some(name) = export_name {
            self.exports.push(Export::new(name, func_idx));
        }
        Ok(())
    }

    fn type_section(&self) -> Vec<u8> {
        section_from_chunks(
            SectionType::Type,
            self.functypes.iter()
                .map(|x| x.as_functype())
                .collect::<Vec<_>>()
                .as_slice()
        )
    }

    fn func_section(&self) -> Vec<u8> {
        section_from_values(SectionType::Function, &self.funcs)
    }

    fn table_section(&self) -> Vec<u8> {
        let n_funcs = self.funcs.len() as u8;
        section_from_chunks(SectionType::Table, &[vec![0x70, 0x00, n_funcs]])
    }

    fn export_section(&self) -> Vec<u8> {
        section_from_chunks(
            SectionType::Export,
            self.exports.iter()
                .map(|x| x.as_export())
                .collect::<Vec<_>>()
                .as_slice()
        )
    }

    fn elem_section(&self) -> Vec<u8> {
        let segments = (0..self.funcs.len()).map(
            |i| vec![0x00, Opcode::I32Const as u8, i as u8, Opcode::End as u8, 0x01, i as u8]
        ).collect::<Vec<_>>();
        section_from_chunks(SectionType::Element, &segments)
    }

    fn code_section(&self) -> Vec<u8> {
        section_from_chunks(SectionType::Code, &self.func_bodies)
    }

    fn program(&self) -> Vec<u8> {
        [
            &MAGIC,
            &VERSION,
            self.type_section().as_slice(),
            self.func_section().as_slice(),
            self.table_section().as_slice(),
            self.export_section().as_slice(),
            self.elem_section().as_slice(),
            self.code_section().as_slice(),
        ].concat()
    }
}

struct WasmFuncCode {
    name: String,
    signature: FuncTypeSignature,
    bytes: Vec<u8>,
    export: bool,
}

impl WasmFuncCode {
    fn new(name: String, export: bool) -> Self {
        Self { name, signature: Default::default(), bytes: vec![], export }
    }
}

pub struct Wasmizer {
    pub typecontext: TypeContext,
    builder: ModuleBuilder,
    current_func: WasmFuncCode,
}

impl Wasmizer {
    fn new(typecontext: TypeContext) -> Self {
        Self { typecontext, builder: Default::default(), current_func: WasmFuncCode::new("main".to_string(), true) }
    }

    pub fn finish_func(&mut self) {
        self.current_func.bytes.push(Opcode::End as u8);
        let export_name = if self.current_func.export {
            Some(self.current_func.name.clone())
        }
        else {
            None
        };
        self.builder.add_function(&self.current_func.signature, 0, self.current_func.bytes.clone(), export_name).unwrap();
    }

    pub fn write_const_i32(&mut self, value: i32) {
        self.current_func.bytes.push(Opcode::I32Const as u8);
        self.current_func.bytes.append(&mut signed_leb128(value));
    }
    pub fn write_add(&mut self, typ: &ast::Type) -> Result<(), String> {
        match typ {
            ast::Type::Int => {
                self.current_func.bytes.push(Opcode::I32Add as u8);
            },
            _ => {
                return Err(format!("unsupported type: {:?}", typ));
            }
        }
        Ok(())
    }
    pub fn write_sub(&mut self, typ: &ast::Type) -> Result<(), String> {
        match typ {
            ast::Type::Int => {
                self.current_func.bytes.push(Opcode::I32Sub as u8);
            },
            _ => {
                return Err(format!("unsupported type: {:?}", typ));
            }
        }
        Ok(())
    }
    pub fn write_mul(&mut self, typ: &ast::Type) -> Result<(), String> {
        match typ {
            ast::Type::Int => {
                self.current_func.bytes.push(Opcode::I32Mul as u8);
            },
            _ => {
                return Err(format!("unsupported type: {:?}", typ));
            }
        }
        Ok(())
    }
    pub fn write_div(&mut self, typ: &ast::Type) -> Result<(), String> {
        match typ {
            ast::Type::Int => {
                self.current_func.bytes.push(Opcode::I32DivS as u8);
            },
            _ => {
                return Err(format!("unsupported type: {:?}", typ));
            }
        }
        Ok(())
    }

    fn to_bytes(&self) -> Vec<u8> {
        self.builder.program()
    }
}

pub fn wasmize(source: String, typecontext: TypeContext) -> Result<(Vec<u8>, ast::Type), String> {
    let tokens = scanner::scan(source);
    let ast = parser::parse(tokens, typecontext.clone()).map_err(|_| "Compilation halted due to parsing error.")?;
    #[cfg(feature = "debug")]
    println!("{:?}", ast);
    let mut wasmizer = Wasmizer::new(typecontext);
    ast.wasmize(&mut wasmizer)?;
    let return_type = ast.get_type()?;

    Ok((wasmizer.to_bytes(), return_type))
}