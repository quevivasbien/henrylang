use crate::{ast, compiler::TypeContext, env::Import, parser, scanner};
use crate::{env, wasmtypes::*};

const MAGIC: [u8; 4] = [0x00, 0x61, 0x73, 0x6d];
const VERSION: [u8; 4] = [0x01, 0x00, 0x00, 0x00];

// a value to be exported
struct Export {
    name: String,
    idx: u32,
    export_type: ExportType,
}

impl Export {
    fn new(name: String, idx: u32, export_type: ExportType) -> Self {
        Self { name, idx, export_type }
    }

    fn as_export(&self) -> Vec<u8> {
        let mut result = encode_string(&self.name);
        result.push(self.export_type as u8);
        result.append(&mut unsigned_leb128(self.idx));
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
    // information about functions, memory, globals, etc. to export
    exports: Vec<Export>,
    // bytecode for each import
    imports: Vec<Vec<u8>>,
}

impl Default for ModuleBuilder {
    fn default() -> Self {
        let mem_export = Export::new("memory".to_string(), 0, ExportType::Memory);
        Self {
            functypes: Vec::new(),
            funcs: vec![],
            func_bodies: vec![],
            exports: vec![mem_export],
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
            self.exports.push(Export::new(name, func_idx, ExportType::Func));
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

    fn memory_section(&self) -> Vec<u8> {
        section_from_chunks(
            SectionType::Memory,
            &[vec![
                0x00,  // limit flag
                0x01,  // initial size
            ]]
        )
    }

    fn global_section(&self) -> Vec<u8> {
        // for now, just the memptr global
        section_from_chunks(
            SectionType::Global,
            &[vec![
                Numtype::I32 as u8,  // data type
                0x01,  // mutability (1 means mutable)
                Opcode::I32Const as u8,
                0x00,  // initial value
                Opcode::End as u8,
            ]]
        )
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
            self.memory_section().as_slice(),
            self.global_section().as_slice(),
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
    allocs: Vec<u32>,  // number of allocs for each scope depth
    bytes: Vec<u8>,
    export: bool,
}

impl WasmFunc {
    fn new(name: String, signature: FuncTypeSignature, export: bool) -> Self {
        Self {
            name,
            signature,
            param_names: vec![],
            locals: Default::default(),
            allocs: vec![],
            bytes: vec![],
            export
        }
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

        // add hard-coded memory allocation functions
        // note: these two functions assume that global[0] is the $memptr
        let alloc_code = vec![
            // get memptr + 4 (start of next memory chunk)
            Opcode::GlobalGet as u8,
            0x00,  // global index
            Opcode::I32Const as u8,
            0x04,  // i32 literal
            Opcode::I32Add as u8,
            // save that location as value to return
            Opcode::LocalSet as u8,
            0x01,  // local index
            // calculate end of new memory chunk
            Opcode::GlobalGet as u8,
            0x00,  // global index
            Opcode::LocalGet as u8,
            0x00,  // local index
            Opcode::I32Const as u8,
            0x04,  // i32 literal
            Opcode::I32Add as u8,
            Opcode::I32Add as u8,
            // set that as new value of memptr
            Opcode::GlobalSet as u8,
            0x00,  // global index
            // write size of allocation at end of block
            Opcode::GlobalGet as u8,
            0x00,  // global index
            Opcode::LocalGet as u8,
            0x00,  // local index
            Opcode::I32Store as u8,
            0x02,  // alignment
            0x00,  // store offset
            // return the start index of the new memory chunk
            Opcode::LocalGet as u8,
            0x01,  // local index
            // 0x23, 0x00, 0x10, 0x00, 0x1a, // print memptr
            Opcode::End as u8,
        ];
        builder.add_function(
            &FuncTypeSignature::new(vec![Numtype::I32], Numtype::I32),
            vec![Numtype::I32 as u8],
            alloc_code,
            None
        ).unwrap();

        let free_code = vec![
            Opcode::I32Const as u8, 88, Opcode::Call as u8, 0x00, Opcode::Drop as u8,
            0x23, 0x00, 0x10, 0x00, 0x1a, // print memptr
            // get memptr
            Opcode::GlobalGet as u8,
            0x00,  // global index
            // load size of allocation (stored at *memptr)
            Opcode::GlobalGet as u8,
            0x00,  // global index
            Opcode::I32Load as u8,
            0x02,  // alignment
            0x00,  // load offset
            0x10, 0x00,  // print chunk size
            // add 4 to size of allocation (since size itself takes 4 bytes)
            Opcode::I32Const as u8,
            0x04,  // i32 literal
            Opcode::I32Add as u8,
            // subtract allocation size from memptr; set as new memptr value
            Opcode::I32Sub as u8,
            Opcode::GlobalSet as u8,
            0x00,  // global index
            Opcode::End as u8,
        ];
        builder.add_function(
            &FuncTypeSignature::default(),
            vec![],
            free_code,
            None
        ).unwrap();

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

    fn write_opcode(&mut self, opcode: Opcode) {
        self.write_byte(opcode as u8);
    }
    fn write_byte(&mut self, byte: u8) {
        self.bytes_mut().push(byte);
    }

    // for debugging
    #[allow(dead_code)]
    fn print_i32_local(&mut self, local_idx: &[u8]) {
        self.write_opcode(Opcode::LocalGet);
        self.bytes_mut().extend_from_slice(&local_idx);
        self.write_opcode(Opcode::Call);
        self.write_byte(0x00);
        self.write_opcode(Opcode::Drop);
    }

    // for debugging
    #[allow(dead_code)]
    fn print_mem(&mut self) {
        // print 8888 twice to show that this is what's going on
        self.write_opcode(Opcode::I32Const);
        self.bytes_mut().extend_from_slice(&signed_leb128(8888));
        self.write_opcode(Opcode::Call);
        self.write_byte(0x00);
        self.write_opcode(Opcode::Call);
        self.write_byte(0x00);
        self.write_opcode(Opcode::Drop);

        let counter_idx = unsigned_leb128(self.locals_mut().add_local(format!("<counter>"), Numtype::I32 as u8));
        // set counter = 0
        self.write_opcode(Opcode::I32Const);
        self.write_byte(0x00);
        self.write_opcode(Opcode::LocalSet);
        self.bytes_mut().extend_from_slice(&counter_idx);

        // start loop
        self.write_opcode(Opcode::Loop);
        self.write_byte(0x40);  // void type

        // read memory at counter
        self.write_opcode(Opcode::LocalGet);
        self.bytes_mut().extend_from_slice(&counter_idx);
        self.write_opcode(Opcode::I32Load);
        self.write_byte(0x02);  // alignment
        self.write_byte(0x00);  // load offset

        // print it
        self.write_opcode(Opcode::Call);
        self.write_byte(0x00);
        self.write_opcode(Opcode::Drop);

        // add 4 to counter
        self.write_opcode(Opcode::LocalGet);
        self.bytes_mut().extend_from_slice(&counter_idx);
        self.write_opcode(Opcode::I32Const);
        self.write_byte(0x04);
        self.write_opcode(Opcode::I32Add);
        self.write_opcode(Opcode::LocalTee);
        self.bytes_mut().extend_from_slice(&counter_idx);

        // continue if counter <= memptr
        self.write_opcode(Opcode::GlobalGet);
        self.write_byte(0x00);
        self.write_opcode(Opcode::I32LeS);
        self.write_opcode(Opcode::BrIf);
        self.write_byte(0x00);  // break depth
        self.write_opcode(Opcode::End);

        // print 8888 twice to show that we're done
        self.write_opcode(Opcode::I32Const);
        self.bytes_mut().extend_from_slice(&signed_leb128(8888));
        self.write_opcode(Opcode::Call);
        self.write_byte(0x00);
        self.write_opcode(Opcode::Call);
        self.write_byte(0x00);
        self.write_opcode(Opcode::Drop);
    }

    pub fn init_func(&mut self, name: String, argtypes: &[ast::Type], rettype: &ast::Type, export: bool) -> Result<(), String> {
        let args = argtypes.iter().map(|t| Numtype::from_ast_type(t)).collect::<Result<Vec<_>, _>>()?;
        let ret = Numtype::from_ast_type(rettype)?;
        let func = WasmFunc::new(name, FuncTypeSignature::new(args, ret), export);
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
        self.write_opcode(Opcode::I32Const);
        self.bytes_mut().append(&mut signed_leb128(idx));
    }

    pub fn write_const(&mut self, value: &str, typ: &ast::Type) -> Result<(), String> {
        match typ {
            ast::Type::Int => {
                let value = value.parse::<i32>().unwrap();
                self.write_opcode(Opcode::I32Const);
                self.bytes_mut().append(&mut signed_leb128(value));
            },
            ast::Type::Float => {
                let value = value.parse::<f32>().unwrap();
                self.write_opcode(Opcode::F32Const);
                self.bytes_mut().extend_from_slice(&value.to_le_bytes());
            }
            ast::Type::Bool => {
                let value = value.parse::<bool>().unwrap();
                self.write_opcode(Opcode::I32Const);
                self.write_byte(if value { 1 } else { 0 });
            }
            ast::Type::Str => {
                // string will be stored in source as sequence of i32 constants
                // i.e. chunks each of size 4 bytes
                let value = value[1..value.len() - 1].to_string();
                let chunks = value.as_bytes().chunks(4);
                let len = chunks.len();
                if len > u16::MAX as usize {
                    return Err(format!("string too long: {}", value));
                }
                for b in chunks.rev() {
                    let x = if b.len() == 4 {
                        u32::from_le_bytes(b.try_into().unwrap())
                    }
                    else {
                        let padding = &[0; 4][0..(4 - b.len())];
                        u32::from_le_bytes([padding, b].concat().as_slice().try_into().unwrap())
                    };
                    self.write_opcode(Opcode::I32Const);
                    self.bytes_mut().append(&mut unsigned_leb128(x));
                }
                self.write_array(len as u16, &ast::Type::Int)?;
            }
            _ => {
                return Err(format!("unsupported literal of type: {:?}", typ));
            }
        }
        Ok(())
    }

    pub fn write_negate(&mut self, typ: &ast::Type) -> Result<(), String> {
        match typ {
            ast::Type::Int => {
                self.write_opcode(Opcode::I32Const);
                self.write_byte(0x7f);  // -1
                self.write_opcode(Opcode::I32Mul);
            }
            ast::Type::Bool => {
                self.write_opcode(Opcode::I32Eqz);
            }
            ast::Type::Float => {
                self.write_opcode(Opcode::F32Neg);
            }
            _ => {
                return Err(format!("Cannot take negative of values of type {:?}", typ));
            }
        }
        Ok(())
    }

    pub fn write_equal(&mut self, typ: &ast::Type) -> Result<(), String> {
        match typ {
            ast::Type::Int | ast::Type::Bool => {
                self.write_opcode(Opcode::I32Eq);
            }
            ast::Type::Float => {
                self.write_opcode(Opcode::F32Eq);
            }
            _ => {
                return Err(format!("Cannot compare values of type {:?}", typ));
            }
        }
        Ok(())
    }
    pub fn write_not_equal(&mut self, typ: &ast::Type) -> Result<(), String> {
        match typ {
            ast::Type::Int | ast::Type::Bool => {
                self.write_opcode(Opcode::I32Ne);
            }
            ast::Type::Float => {
                self.write_opcode(Opcode::F32Ne);
            }
            _ => {
                return Err(format!("Cannot compare values of type {:?}", typ));
            }
        }
        Ok(())
    }
    pub fn write_greater(&mut self, typ: &ast::Type) -> Result<(), String> {
        match typ {
            ast::Type::Int => {
                self.write_opcode(Opcode::I32GtS);
            }
            ast::Type::Float => {
                self.write_opcode(Opcode::F32Gt);
            }
            _ => {
                return Err(format!("Order is not defined for type {:?}", typ));
            }
        }
        Ok(())
    }
    pub fn write_greater_equal(&mut self, typ: &ast::Type) -> Result<(), String> {
        match typ {
            ast::Type::Int => {
                self.write_opcode(Opcode::I32GeS);
            }
            ast::Type::Float => {
                self.write_opcode(Opcode::F32Ge);
            }
            _ => {
                return Err(format!("Order is not defined for type {:?}", typ));
            }
        }
        Ok(())
    }
    pub fn write_less(&mut self, typ: &ast::Type) -> Result<(), String> {
        match typ {
            ast::Type::Int => {
                self.write_opcode(Opcode::I32LtS);
            }
            ast::Type::Float => {
                self.write_opcode(Opcode::F32Lt);
            }
            _ => {
                return Err(format!("Order is not defined for type {:?}", typ));
            }
        }
        Ok(())
    }
    pub fn write_less_equal(&mut self, typ: &ast::Type) -> Result<(), String> {
        match typ {
            ast::Type::Int => {
                self.write_opcode(Opcode::I32LeS);
            }
            ast::Type::Float => {
                self.write_opcode(Opcode::F32Le);
            }
            _ => {
                return Err(format!("Order is not defined for type {:?}", typ));
            }
        }
        Ok(())
    }

    pub fn write_add(&mut self, typ: &ast::Type) -> Result<(), String> {
        match typ {
            ast::Type::Int => {
                self.write_opcode(Opcode::I32Add);
            },
            ast::Type::Float => {
                self.write_opcode(Opcode::F32Add);
            },
            ast::Type::Str | ast::Type::Arr(_) => {
                self.concat_arrays()?;
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
                self.write_opcode(Opcode::I32Sub);
            },
            ast::Type::Float => {
                self.write_opcode(Opcode::F32Sub);
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
                self.write_opcode(Opcode::I32Mul);
            },
            ast::Type::Float => {
                self.write_opcode(Opcode::F32Mul);
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
                self.write_opcode(Opcode::I32DivS);
            },
            ast::Type::Float => {
                self.write_opcode(Opcode::F32Div);
            },
            _ => {
                return Err(format!("Cannot divide values of type {:?}", typ));
            }
        }
        Ok(())
    }
    pub fn write_and(&mut self, typ: &ast::Type) -> Result<(), String> {
        match typ {
            ast::Type::Bool => {
                self.write_opcode(Opcode::I32And);
            },
            _ => {
                return Err(format!("Cannot AND values of type {:?}", typ));
            }
        }
        Ok(())  
    }
    pub fn write_or(&mut self, typ: &ast::Type) -> Result<(), String> {
        match typ {
            ast::Type::Bool => {
                self.write_opcode(Opcode::I32Or);
            },
            _ => {
                return Err(format!("Cannot OR values of type {:?}", typ));
            }
        }
        Ok(())
    }

    fn repeat(&mut self, bytes: &[u8], count: u32) -> Result<(), String> {
        // creates a for loop that repeats instructions in bytes count times
        let i = unsigned_leb128(self.locals_mut().add_local(format!("<i>"), Numtype::I32 as u8));
        // set i = count
        self.write_opcode(Opcode::I32Const);
        self.bytes_mut().append(&mut unsigned_leb128(count));
        self.write_opcode(Opcode::LocalSet);
        self.bytes_mut().extend_from_slice(&i);

        // begin loop
        self.write_opcode(Opcode::Loop);
        self.write_byte(0x40);  // void type
        
        // insert bytes
        self.bytes_mut().extend_from_slice(bytes);

        // subtract 1 from i
        self.write_opcode(Opcode::LocalGet);
        self.bytes_mut().extend_from_slice(&i);
        self.write_opcode(Opcode::I32Const);
        self.write_byte(0x01);
        self.write_opcode(Opcode::I32Sub);
        self.write_opcode(Opcode::LocalTee);
        self.bytes_mut().extend_from_slice(&i);

        // continue if i != 0
        self.write_opcode(Opcode::BrIf);
        self.write_byte(0x00);  // break depth
        self.write_opcode(Opcode::End);
        Ok(())
    }

    // sets offset = fatptr >> 32 and size = fatptr & 0xFFFFFFFF
    // assumes fatptr is last thing on stack before call
    fn set_offset_and_size(&mut self, fatptr_idx: &[u8], offset_idx: &[u8], size_idx: &[u8]) {
        self.write_opcode(Opcode::I64Const);
        self.write_byte(0x20);
        self.write_opcode(Opcode::I64ShrU);  // shift right 32 bits
        self.write_opcode(Opcode::I32WrapI64);  // discard high 32 bits
        self.write_opcode(Opcode::LocalSet);
        self.bytes_mut().extend_from_slice(offset_idx);  // set as offset
        // size = lowest 32 bits of fatptr
        self.write_opcode(Opcode::LocalGet);
        self.bytes_mut().extend_from_slice(fatptr_idx);
        self.write_opcode(Opcode::I32WrapI64);  // discard high 32 bits
        self.write_opcode(Opcode::LocalSet);
        self.bytes_mut().extend_from_slice(size_idx);  // set as size
    }

    // copy size bytes from offset to memptr
    // sets offset to value of memptr, then increments memptr by size
    // if write_size is true, write the size of the allocation at the end of the block
    fn copy_mem(&mut self, offset_idx: &[u8], size_idx: &[u8], write_size: bool) {
        // destination is current value of memptr
        self.write_opcode(Opcode::GlobalGet);
        self.write_byte(0x00);
        // source address is `offset`
        self.write_opcode(Opcode::LocalGet);
        self.bytes_mut().extend_from_slice(&offset_idx);
        // copy `size` bytes
        self.write_opcode(Opcode::LocalGet);
        self.bytes_mut().extend_from_slice(&size_idx);
        self.bytes_mut().extend_from_slice(&MEMCOPY);

        // increment n allocs
        *self.current_func_mut().allocs.last_mut().unwrap() += 1;

        // set offset to memptr
        self.write_opcode(Opcode::GlobalGet);
        self.write_byte(0x00);
        self.write_opcode(Opcode::LocalSet);
        self.bytes_mut().extend_from_slice(&offset_idx);

        // todo: get rid of these
        // print offset
        self.print_i32_local(&offset_idx);
        // print size
        self.print_i32_local(&size_idx);

        // set memptr to memptr + size (+4 if also writing size)
        self.write_opcode(Opcode::GlobalGet);
        self.write_byte(0x00);
        self.write_opcode(Opcode::LocalGet);
        self.bytes_mut().extend_from_slice(&size_idx);
        if write_size {
            self.write_opcode(Opcode::I32Const);
            self.write_byte(0x04);
            self.write_opcode(Opcode::I32Add);
        }
        self.write_opcode(Opcode::I32Add);
        self.write_opcode(Opcode::GlobalSet);
        self.write_byte(0x00);

        // if writing size at end of block, do that here
        if write_size {
            self.write_opcode(Opcode::GlobalGet);
            self.write_byte(0x00);
            self.write_opcode(Opcode::LocalGet);
            self.bytes_mut().extend_from_slice(&size_idx);
            self.write_opcode(Opcode::I32Store);
            self.write_byte(0x02);  // alignment
            self.write_byte(0x00);  // store offset
        }
    }

    fn copy_array(&mut self, fatptr_idx: u32) -> Result<(), String> {
        let fatptr_idx = unsigned_leb128(fatptr_idx);
        let offset_idx = unsigned_leb128(self.locals_mut().add_local(format!("<offset>"), Numtype::I32 as u8));
        let size_idx = unsigned_leb128(self.locals_mut().add_local(format!("<size>"), Numtype::I32 as u8));

        // sets fatptr, then sets offset and size from fatptr
        self.write_opcode(Opcode::LocalTee);
        self.bytes_mut().extend_from_slice(&fatptr_idx);  // get fatptr
        self.set_offset_and_size(&fatptr_idx, &offset_idx, &size_idx);

        self.copy_mem(&offset_idx, &size_idx, true);

        // return [offset, size]
        self.write_opcode(Opcode::LocalGet);
        self.bytes_mut().extend_from_slice(&offset_idx);
        self.write_opcode(Opcode::I64ExtendI32U);
        self.write_opcode(Opcode::I64Const);
        self.write_byte(32);
        self.write_opcode(Opcode::I64Shl);
        self.write_opcode(Opcode::LocalGet);
        self.bytes_mut().extend_from_slice(&size_idx);
        self.write_opcode(Opcode::I64ExtendI32U);
        self.write_opcode(Opcode::I64Add);

        Ok(())
    }

    // concatenate two arrays/strings
    // the fatptrs indices to the arrays should be the two last things on the stack when calling this
    fn concat_arrays(&mut self) -> Result<(), String> {
        // declare locals to store fatptrs, then store fatptrs
        let fatptr_idx2 = unsigned_leb128(self.locals_mut().add_local(format!("<fatptr2>"), Numtype::I64 as u8));
        let fatptr_idx1 = unsigned_leb128(self.locals_mut().add_local(format!("<fatptr1>"), Numtype::I64 as u8));
        self.write_opcode(Opcode::LocalSet);
        self.bytes_mut().extend_from_slice(&fatptr_idx2);
        self.write_opcode(Opcode::LocalSet);
        self.bytes_mut().extend_from_slice(&fatptr_idx1);

        // declare locals to store offsets and sizes
        let offset_idx2 = unsigned_leb128(self.locals_mut().add_local(format!("<offset>"), Numtype::I32 as u8));
        let size_idx2 = unsigned_leb128(self.locals_mut().add_local(format!("<size>"), Numtype::I32 as u8));
        
        let offset_idx1 = unsigned_leb128(self.locals_mut().add_local(format!("<offset>"), Numtype::I32 as u8));
        let size_idx1 = unsigned_leb128(self.locals_mut().add_local(format!("<size>"), Numtype::I32 as u8));

        // set offset and size for each array
        self.write_opcode(Opcode::LocalGet);
        self.bytes_mut().extend_from_slice(&fatptr_idx2);
        self.set_offset_and_size(&fatptr_idx2, &offset_idx2, &size_idx2);
        self.write_opcode(Opcode::LocalGet);
        self.bytes_mut().extend_from_slice(&fatptr_idx1);
        self.set_offset_and_size(&fatptr_idx1, &offset_idx1, &size_idx1);

        // copy memory from arrays into consecutive memory
        self.copy_mem(&offset_idx1, &size_idx1, false);
        self.copy_mem(&offset_idx2, &size_idx2, false);

        // write combined size at the end
        // first, increment memptr by 4
        self.write_opcode(Opcode::GlobalGet);
        self.write_byte(0x00);
        self.write_opcode(Opcode::I32Const);
        self.write_byte(4);
        self.write_opcode(Opcode::I32Add);
        self.write_opcode(Opcode::GlobalSet);
        self.write_byte(0x00);
        // then write combined size
        self.write_opcode(Opcode::GlobalGet);
        self.write_byte(0x00);
        self.write_opcode(Opcode::LocalGet);
        self.bytes_mut().extend_from_slice(&size_idx1);
        self.write_opcode(Opcode::LocalGet);
        self.bytes_mut().extend_from_slice(&size_idx2);
        self.write_opcode(Opcode::I32Add);
        self.write_opcode(Opcode::I32Store);
        self.write_byte(0x02);  // alignment
        self.write_byte(0x00);  // store offset

        // return [offset1, size1 + size2]
        self.write_opcode(Opcode::LocalGet);
        self.bytes_mut().extend_from_slice(&offset_idx1);
        self.write_opcode(Opcode::I64ExtendI32U);
        self.write_opcode(Opcode::I64Const);
        self.write_byte(32);
        self.write_opcode(Opcode::I64Shl);
        self.write_opcode(Opcode::LocalGet);
        self.bytes_mut().extend_from_slice(&size_idx1);
        self.write_opcode(Opcode::LocalGet);
        self.bytes_mut().extend_from_slice(&size_idx2);
        self.write_opcode(Opcode::I32Add);
        self.write_opcode(Opcode::I64ExtendI32U);
        self.write_opcode(Opcode::I64Add);
        Ok(())
    }

    pub fn write_array(&mut self, len: u16, typ: &ast::Type) -> Result<(), String> {
        // create locals to store info about array as it is constructed
        let arrname = format!("<arr[{}{}]>", len, Numtype::from_ast_type(typ)? as u8);
        let startptr_idx = unsigned_leb128(self.locals_mut().add_local(format!("{}startptr", arrname), Numtype::I32 as u8));
        let memptr_idx = unsigned_leb128(self.locals_mut().add_local(format!("{}memptr", arrname), Numtype::I32 as u8));
        let value_idx = unsigned_leb128(self.locals_mut().add_local(format!("{}value", arrname), Numtype::from_ast_type(typ)? as u8));
        
        // determine the bytes per value needed
        let memsize = match typ {
            ast::Type::Int | ast::Type::Bool | ast::Type::Float => 4,
            _ => return Err(format!("cannot allocate memory for array of type {:?}", typ))
        } as u8;
        // call alloc
        self.write_opcode(Opcode::I32Const);
        self.bytes_mut().append(&mut unsigned_leb128(len as u32 * memsize as u32));
        self.write_opcode(Opcode::Call);
        let alloc_func_idx = self.builder.imports.len() as u8;
        self.write_byte(alloc_func_idx);
        *self.current_func_mut().allocs.last_mut().unwrap() += 1;
        // alloc will return index of start pos -- set startptr and memptr to that index
        self.write_opcode(Opcode::LocalTee);
        self.bytes_mut().extend(&startptr_idx);
        self.write_opcode(Opcode::LocalSet);
        self.bytes_mut().extend(&memptr_idx);

        // write vars on stack to memory
        // figure out what opcode to use to store values in memory
        let store_op = match typ {
            ast::Type::Int | ast::Type::Bool => Opcode::I32Store,
            ast::Type::Float => Opcode::F32Store,
            _ => unreachable!("Should have been filtered out at beginning of function")
        };
        for _ in 0..len {
            self.write_to_memory(&memptr_idx, &value_idx, store_op, memsize)?;
        }

        // return [startptr len]
        self.write_opcode(Opcode::LocalGet);
        self.bytes_mut().extend(&startptr_idx);
        self.write_opcode(Opcode::I64ExtendI32U);
        self.write_opcode(Opcode::I64Const);
        self.write_byte(32);
        self.write_opcode(Opcode::I64Shl);
        self.write_opcode(Opcode::I64Const);
        self.bytes_mut().append(&mut unsigned_leb128(len as u32 * 4));  // multiply by 4 since 4 bytes per 32-bit value
        self.write_opcode(Opcode::I64Add);
        Ok(())
    }

    fn write_to_memory(&mut self, memptr_idx: &[u8], value_idx: &[u8], store_op: Opcode, memsize: u8) -> Result<(), String> {
        // set local var to last value on stack
        self.write_opcode(Opcode::LocalSet);
        self.bytes_mut().extend_from_slice(value_idx);

        // write index of local var that tells where in memory to write this value
        self.write_opcode(Opcode::LocalGet);
        self.bytes_mut().extend_from_slice(memptr_idx);
        // write index of local var that contains the value to be written
        self.write_opcode(Opcode::LocalGet);
        self.bytes_mut().extend_from_slice(value_idx);

        self.write_byte(store_op as u8);
        // todo: figure out what these two lines do.
        self.write_byte(0x02);  // alignment?
        self.write_byte(0x00);

        // increase memptr by memsize
        self.write_opcode(Opcode::LocalGet);
        self.bytes_mut().extend_from_slice(memptr_idx);
        self.write_opcode(Opcode::I32Const);
        self.write_byte(memsize);
        self.write_opcode(Opcode::I32Add);
        self.write_opcode(Opcode::LocalSet);
        self.bytes_mut().extend_from_slice(memptr_idx);

        Ok(())
    }

    fn free_multiple(&mut self, n_frees: u32) -> Result<(), String> {
        #[cfg(feature = "debug")]
        self.print_mem();
        // call free
        let mut bytes = Vec::with_capacity(2);
        bytes.push(Opcode::Call as u8);
        bytes.append(&mut unsigned_leb128(self.builder.imports.len() as u32 + 1));
        self.repeat(&bytes, n_frees)
    }

    pub fn write_drop(&mut self) {
        self.write_opcode(Opcode::Drop);
    }

    pub fn begin_scope(&mut self, typ: &ast::Type) -> Result<(), String> {
        self.locals_mut().scope_depth += 1;
        self.current_func_mut().allocs.push(0);
        if self.locals().scope_depth > 0 {
            self.write_opcode(Opcode::Block);
            self.write_byte(Numtype::from_ast_type(typ)? as u8)
        }
        Ok(())
    }
    pub fn end_scope(&mut self) -> Result<(), String> {
        // free memory
        let n_frees = self.current_func_mut().allocs.pop().unwrap();
        if n_frees > 0 {
            self.free_multiple(n_frees)?;
        }
        // Write END opcode, if we're not at the top level of a function
        if self.locals().scope_depth > 0 {
            self.write_opcode(Opcode::End);
        }
        // pop out-of-scope local variables
        self.locals_mut().scope_depth -= 1;
        while let Some(local) = self.locals().locals.last() {
            if local.depth <= self.locals().scope_depth {
                break;
            }
            self.locals_mut().locals.pop();
        }
        Ok(())
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
    pub fn set_variable(&mut self, idx: u32, typ: &ast::Type) -> Result<(), String> {
        let idx = idx + self.current_func().n_params();
        
        // if typ is a heap-allocated type, we also need to copy the memory
        if matches!(typ, ast::Type::Arr(_) | ast::Type::Str) {
            self.copy_array(idx)?;
        }
        self.write_opcode(Opcode::LocalTee);
        self.bytes_mut().append(&mut unsigned_leb128(idx));

        Ok(())
    }
    pub fn get_variable(&mut self, name: String) -> Result<i32, String> {
        // can't yet deal with upvalues
        // first look in local variables
        let idx = self.locals().get_idx(&name);
        if let Some(idx) = idx {
            self.write_opcode(Opcode::LocalGet);
            self.bytes_mut().append(&mut unsigned_leb128(idx));
            return Ok(0);
        }
        // next look in function parameters
        let idx = self.current_func().get_param_idx(&name);
        if let Some(idx) = idx {
            self.write_opcode(Opcode::LocalGet);
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
        self.write_opcode(Opcode::CallIndirect);
        self.bytes_mut().append(&mut unsigned_leb128(idx));  // signature index
        self.write_byte(0x00);  // table index
        Ok(())
    }
    pub fn call(&mut self) -> Result<(), String> {
        let fn_idx = match self.bytes_mut().pop() {
            Some(idx) => idx,
            None => return Err("Call called on empty stack".to_string()),
        };
        self.write_opcode(Opcode::Call);
        self.write_byte(fn_idx);
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