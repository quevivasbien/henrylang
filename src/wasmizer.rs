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
    F32 = 0x7d,
    I32 = 0x7f,
}

impl Numtype {
    fn from_ast_type(typ: &ast::Type) -> Result<Self, String> {
        match typ {
            ast::Type::Int => Ok(Self::I32),
            ast::Type::Float => Ok(Self::F32),
            ast::Type::Func(..) => Ok(Self::I32),  // functions are referred to by their table indices
            _ => Err(format!("Cannot convert type {:?} to WASM Numtype", typ)),
        }
    }
}

enum ExportType {
    Func = 0x00,
}

enum Opcode {
    Block = 0x02,
    End = 0x0b,
    CallIndirect = 0x11,
    Drop = 0x1a,
    LocalGet = 0x20,
    LocalTee = 0x22,
    I32Const = 0x41,
    F32Const = 0x43,
    I32Add = 0x6a,
    I32Sub = 0x6b,
    I32Mul = 0x6c,
    I32DivS = 0x6d,
    F32Add = 0x92,
    F32Sub = 0x93,
    F32Mul = 0x94,
    F32Div = 0x95,
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

fn section_from_values(section_type: SectionType, values: &[u32]) -> Vec<u8> {
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

fn function_body(local_types: Vec<u8>, mut code: Vec<u8>) -> Vec<u8> {
    let mut result = unsigned_leb128(local_types.len() as u32);
    for ltype in local_types {
        result.push(0x01);  // count of locals with this type
        result.push(ltype);
    }
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
    fn from_ast_type(typ: &ast::Type) -> Result<Self, String> {
        let (args, ret) = match typ {
            ast::Type::Func(args, ret) => (args, ret),
            _ => return Err(format!("Cannot convert type {:?} to WASM FuncTypeSignature", typ)),
        };
        let args = args.iter().map(|x| Numtype::from_ast_type(x)).collect::<Result<_, _>>()?;
        let ret = Numtype::from_ast_type(ret)?;
        Ok(Self { args, ret })
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
    func_idx: u32,
}

impl Export {
    fn new(name: String, func_idx: u32) -> Self {
        Self { name, func_idx }
    }

    fn as_export(&self) -> Vec<u8> {
        let mut result = encode_string(&self.name);
        result.push(ExportType::Func as u8);
        result.append(&mut unsigned_leb128(self.func_idx));
        result
    }
}

struct ModuleBuilder {
    // stores type signatures for each function
    functypes: Vec<FuncTypeSignature>,
    // indices to functypes for each function
    funcs: Vec<u32>,
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
    fn add_function(&mut self, sig: &FuncTypeSignature, local_types: Vec<u8>, code: Vec<u8>, export_name: Option<String>) -> Result<(), String> {
        let ftype = self.get_functype_idx(sig);
        let func_idx = self.funcs.len() as u32;
        if func_idx == u32::MAX {
            return Err(format!("too many functions"));
        }
        self.funcs.push(ftype);
        self.func_bodies.push(function_body(local_types, code));
        if let Some(name) = export_name {
            self.exports.push(Export::new(name, func_idx));
        }
        Ok(())
    }

    fn get_functype_idx(&mut self, ftype: &FuncTypeSignature) -> u32 {
        match self.functypes.iter().enumerate()
            .find_map(|(i, x)| if x == ftype { Some(i) } else { None })
        {
            Some(i) => i as u32,
            None => {
                self.functypes.push(ftype.clone());
                self.functypes.len() as u32 - 1
            }
        }
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
        section_from_chunks(SectionType::Table, &[vec![
            0x70,    // funcref
            0x00,    // minimum
            n_funcs  // maximum
        ]])
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
            |i| {
                let i = unsigned_leb128(i as u32);
                [
                    &[0x00, Opcode::I32Const as u8],
                    i.as_slice(),
                    &[Opcode::End as u8, 0x01],
                    i.as_slice()
                ].concat()
            }
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

#[derive(Debug)]
struct Local {
    name: String,
    depth: i32,
}

#[derive(Debug)]
struct LocalData {
    locals: Vec<Local>,
    types: Vec<u8>,
    scope_depth: i32,
}

impl Default for LocalData {
    fn default() -> Self {
        Self {
            locals: Vec::new(),
            types: Vec::new(),
            scope_depth: -1,
        }
    }
}

impl LocalData {
    fn add_local(&mut self, name: String, typ: u8) -> u32 {
        self.locals.push(Local { name, depth: self.scope_depth });
        self.types.push(typ);
        self.locals.len() as u32 - 1
    }
    fn get_idx(&self, name: &str) -> Option<u32> {
        for (i, local) in self.locals.iter().enumerate().rev() {
            if local.name == name {
                return Some(i as u32);
            }
        }
        None
    }
}

struct WasmFunc {
    name: String,
    signature: FuncTypeSignature,
    param_names: Vec<String>,
    locals: LocalData,
    bytes: Vec<u8>,
    export: bool,
}

impl WasmFunc {
    fn new(name: String, signature: FuncTypeSignature, export: bool) -> Self {
        Self { name, signature, param_names: vec![], locals: Default::default(), bytes: vec![], export }
    }
    fn n_params(&self) -> u32 {
        self.signature.args.len() as u32
    }
    fn get_param_idx(&self, name: &str) -> Option<u32> {
        self.param_names.iter().position(|x| x == name).map(|x| x as u32)
    }
}

pub struct Wasmizer {
    pub typecontext: TypeContext,
    builder: ModuleBuilder,
    frames: Vec<WasmFunc>,
}

impl Wasmizer {
    fn new(typecontext: TypeContext) -> Self {
        Self { typecontext, builder: Default::default(), frames: vec![] }
    }

    fn current_func(&self) -> &WasmFunc {
        self.frames.last().unwrap()
    }
    fn current_func_mut(&mut self) -> &mut WasmFunc {
        self.frames.last_mut().unwrap()
    }
    fn locals(&self) -> &LocalData {
        &self.current_func().locals
    }
    fn locals_mut(&mut self) -> &mut LocalData {
        &mut self.current_func_mut().locals
    }
    pub fn bytes(&self) -> &Vec<u8> {
        &self.current_func().bytes
    }
    fn bytes_mut(&mut self) -> &mut Vec<u8> {
        &mut self.current_func_mut().bytes
    }

    pub fn init_func(&mut self, name: String, argtypes: &[ast::Type], rettype: &ast::Type, export: bool) -> Result<(), String> {
        let args = argtypes.iter().map(|t| Numtype::from_ast_type(t)).collect::<Result<Vec<_>, _>>()?;
        let ret = Numtype::from_ast_type(rettype)?;
        let func = WasmFunc::new(name, FuncTypeSignature { args, ret }, export);
        self.frames.push(func);
        Ok(())
    }
    pub fn finish_func(&mut self) -> Result<(), String> {
        self.current_func_mut().bytes.push(Opcode::End as u8);
        let export_name = if self.current_func().export {
            Some(self.current_func().name.clone())
        }
        else {
            None
        };
        let func = match self.frames.pop() {
            Some(func) => func,
            None => return Err("Tried to pop function when frames is empty".to_string()),
        };
        self.builder.add_function(
            &func.signature,
            func.locals.types,
            func.bytes,
            export_name
        )
    }
    pub fn write_last_func_index(&mut self) {
        let idx = self.builder.funcs.len() as i32 - 1;
        self.bytes_mut().push(Opcode::I32Const as u8);
        self.bytes_mut().append(&mut signed_leb128(idx));
    }

    pub fn write_const(&mut self, value: &str, typ: &ast::Type) -> Result<(), String> {
        match typ {
            ast::Type::Int => {
                let value = value.parse::<i32>().unwrap();
                self.bytes_mut().push(Opcode::I32Const as u8);
                self.bytes_mut().append(&mut signed_leb128(value));
            },
            ast::Type::Float => {
                let value = value.parse::<f32>().unwrap();
                self.bytes_mut().push(Opcode::F32Const as u8);
                self.bytes_mut().extend_from_slice(&value.to_le_bytes());
            }
            ast::Type::Bool => {
                let value = value.parse::<bool>().unwrap();
                self.bytes_mut().push(Opcode::I32Const as u8);
                self.bytes_mut().push(if value { 1 } else { 0 });
            }
            _ => {
                return Err(format!("unsupported literal of type: {:?}", typ));
            }
        }
        Ok(())
    }
    pub fn write_add(&mut self, typ: &ast::Type) -> Result<(), String> {
        match typ {
            ast::Type::Int => {
                self.bytes_mut().push(Opcode::I32Add as u8);
            },
            ast::Type::Float => {
                self.bytes_mut().push(Opcode::F32Add as u8);
            },
            _ => {
                return Err(format!("Cannot add values of type {:?}", typ));
            }
        }
        Ok(())
    }
    pub fn write_sub(&mut self, typ: &ast::Type) -> Result<(), String> {
        match typ {
            ast::Type::Int => {
                self.bytes_mut().push(Opcode::I32Sub as u8);
            },
            ast::Type::Float => {
                self.bytes_mut().push(Opcode::F32Sub as u8);
            },
            _ => {
                return Err(format!("Cannot subtract values of type {:?}", typ));
            }
        }
        Ok(())
    }
    pub fn write_mul(&mut self, typ: &ast::Type) -> Result<(), String> {
        match typ {
            ast::Type::Int => {
                self.bytes_mut().push(Opcode::I32Mul as u8);
            },
            ast::Type::Float => {
                self.bytes_mut().push(Opcode::F32Mul as u8);
            },
            _ => {
                return Err(format!("Cannot multiply values of type {:?}", typ));
            }
        }
        Ok(())
    }
    pub fn write_div(&mut self, typ: &ast::Type) -> Result<(), String> {
        match typ {
            ast::Type::Int => {
                self.bytes_mut().push(Opcode::I32DivS as u8);
            },
            ast::Type::Float => {
                self.bytes_mut().push(Opcode::F32Div as u8);
            },
            _ => {
                return Err(format!("Cannot divide values of type {:?}", typ));
            }
        }
        Ok(())
    }

    pub fn write_drop(&mut self) {
        self.bytes_mut().push(Opcode::Drop as u8);
    }

    pub fn begin_scope(&mut self, typ: &ast::Type) -> Result<(), String> {
        self.locals_mut().scope_depth += 1;
        if self.locals().scope_depth > 0 {
            self.bytes_mut().push(Opcode::Block as u8);
            self.bytes_mut().push(Numtype::from_ast_type(typ)? as u8)
        }
        Ok(())
    }
    pub fn end_scope(&mut self) {
        if self.locals().scope_depth > 0 {
            self.bytes_mut().push(Opcode::End as u8);
        }
        self.locals_mut().scope_depth -= 1;
        while let Some(local) = self.locals().locals.last() {
            if local.depth <= self.locals().scope_depth {
                break;
            }
            self.locals_mut().locals.pop();
        }
    }

    pub fn add_param_name(&mut self, name: String) {
        self.current_func_mut().param_names.push(name);
    }
    pub fn create_variable(&mut self, name: String, typ: &ast::Type) -> Result<u32, String> {
        // only handle local variables for now
        // create a local variable
        let typ = Numtype::from_ast_type(typ)? as u8;
        Ok(self.locals_mut().add_local(name, typ))
    }
    pub fn set_variable(&mut self, idx: u32) {
        self.bytes_mut().push(Opcode::LocalTee as u8);
        let idx = idx + self.current_func().n_params();
        self.bytes_mut().append(&mut unsigned_leb128(idx));
    }
    pub fn get_variable(&mut self, name: String) -> Result<(), String> {
        // can't yet deal with globals or upvalues
        self.bytes_mut().push(Opcode::LocalGet as u8);
        // first look in local variables
        let idx = self.locals().get_idx(&name);
        if let Some(idx) = idx {
            self.bytes_mut().append(&mut unsigned_leb128(idx));
            return Ok(());
        }
        // next look in function parameters
        let idx = self.current_func().get_param_idx(&name);
        if let Some(idx) = idx {
            self.bytes_mut().append(&mut unsigned_leb128(idx));
            return Ok(());
        }
        Err(format!("variable {} not found in local scope", name))
    }

    pub fn call_indirect(&mut self, typ: &ast::Type) -> Result<(), String> {
        let idx = self.builder.get_functype_idx(&FuncTypeSignature::from_ast_type(typ)?);
        self.bytes_mut().push(Opcode::CallIndirect as u8);
        self.bytes_mut().append(&mut unsigned_leb128(idx));  // signature index
        self.bytes_mut().push(0x00);  // table index
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

    let bytes = wasmizer.to_bytes();
    #[cfg(feature = "debug")]
    {
        // print out bytes in form similar to xxd -g 1
        for (i, b) in bytes.iter().enumerate() {
            if i % 16 == 0 {
                print!("{:04x}: ", i);
            }
            print!("{:02x} ", b);
            if i % 16 == 15 || i == bytes.len() - 1 {
                println!();
            }
        }
        println!();

        // dump to file
        std::fs::write("test.wasm", &bytes).unwrap();
    }
    Ok((bytes, return_type))
}