use crate::{ast, compiler::TypeContext, env::Import, parser, scanner};
use crate::{env, wasmtypes::*};

const MAGIC: [u8; 4] = [0x00, 0x61, 0x73, 0x6d];
const VERSION: [u8; 4] = [0x01, 0x00, 0x00, 0x00];

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
    // bytecode for each import
    imports: Vec<Vec<u8>>,
}

impl Default for ModuleBuilder {
    fn default() -> Self {
        Self {
            functypes: Vec::new(),
            funcs: vec![],
            func_bodies: vec![],
            exports: vec![],
            imports: vec![],
        }
    }
}

impl ModuleBuilder {
    fn add_function(&mut self, sig: &FuncTypeSignature, local_types: Vec<u8>, code: Vec<u8>, export_name: Option<String>) -> Result<(), String> {
        let ftype = self.get_functype_idx(sig);
        let func_idx = (self.imports.len() + self.funcs.len()) as u32;
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

    fn add_import(&mut self, import: &Import) {
        let module = encode_string(import.module);
        let field = encode_string(import.field);
        let ftype = unsigned_leb128(self.get_functype_idx(&import.sig));
        let bytes = [
            module,
            field,
            vec![0x00],
            ftype,
        ].concat();
        self.imports.push(bytes);
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

    fn import_section(&self) -> Vec<u8> {
        section_from_chunks(SectionType::Import, &self.imports)
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
                [
                    &[0x00, Opcode::I32Const as u8],
                    unsigned_leb128(i as u32).as_slice(),
                    &[Opcode::End as u8, 0x01],
                    unsigned_leb128((i + self.imports.len()) as u32).as_slice()
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
            self.import_section().as_slice(),
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
    pub global_vars: env::GlobalVars,
    builder: ModuleBuilder,
    frames: Vec<WasmFunc>,
}

impl Wasmizer {
    fn new(global_env: env::Env) -> Self {
        let mut builder: ModuleBuilder = Default::default();
        global_env.imports.iter().for_each(
            |import| {
                builder.add_import(import);
            }
        );
        Self {
            typecontext: global_env.global_types,
            global_vars: global_env.global_vars,
            builder,
            frames: vec![]
        }
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

    pub fn write_const(&mut self, value: &str, typ: &ast::Type) -> Result<i32, String> {
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
        Ok(0)
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
    pub fn get_variable(&mut self, name: String) -> Result<i32, String> {
        // can't yet deal with upvalues
        // first look in local variables
        let idx = self.locals().get_idx(&name);
        if let Some(idx) = idx {
            self.bytes_mut().push(Opcode::LocalGet as u8);
            self.bytes_mut().append(&mut unsigned_leb128(idx));
            return Ok(0);
        }
        // next look in function parameters
        let idx = self.current_func().get_param_idx(&name);
        if let Some(idx) = idx {
            self.bytes_mut().push(Opcode::LocalGet as u8);
            self.bytes_mut().append(&mut unsigned_leb128(idx));
            return Ok(0);
        }
        // finally, look in global scope
        let maybe_value = {
            let globals = self.global_vars.borrow();
            globals.get(&name).cloned()
        };
        if let Some(value) = maybe_value {
            self.bytes_mut().append(&mut unsigned_leb128(value as u32));
            return Ok(1)  // 1 denotes found variable is global
        }
        Err(format!("variable {} not found", name))
    }

    pub fn call_indirect(&mut self, typ: &ast::Type) -> Result<(), String> {
        let idx = self.builder.get_functype_idx(&FuncTypeSignature::from_ast_type(typ)?);
        self.bytes_mut().push(Opcode::CallIndirect as u8);
        self.bytes_mut().append(&mut unsigned_leb128(idx));  // signature index
        self.bytes_mut().push(0x00);  // table index
        Ok(())
    }
    pub fn call(&mut self) -> Result<(), String> {
        let fn_idx = match self.bytes_mut().pop() {
            Some(idx) => idx,
            None => return Err("Call called on empty stack".to_string()),
        };
        self.bytes_mut().push(Opcode::Call as u8);
        self.bytes_mut().push(fn_idx);
        Ok(())
    }

    fn to_bytes(&self) -> Vec<u8> {
        self.builder.program()
    }
}

pub fn wasmize(source: String, global_env: env::Env) -> Result<(Vec<u8>, ast::Type), String> {
    let tokens = scanner::scan(source);
    let ast = parser::parse(tokens, global_env.global_types.clone()).map_err(|_| "Compilation halted due to parsing error.")?;
    #[cfg(feature = "debug")]
    println!("{:?}", ast);
    let mut wasmizer = Wasmizer::new(global_env);
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