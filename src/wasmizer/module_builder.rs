use crate::env::Import;

use super::{builtin_funcs::BuiltinFunc, wasmtypes::*};

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
        Self {
            name,
            idx,
            export_type,
        }
    }

    fn as_export(&self) -> Vec<u8> {
        let mut result = encode_string(&self.name);
        result.push(self.export_type as u8);
        result.append(&mut unsigned_leb128(self.idx));
        result
    }
}

// represents a passive memory segment (not automatically placed in memory)
struct DataSegment {
    data: Vec<u8>,
}

impl DataSegment {
    fn new(data: Vec<u8>) -> Self {
        Self { data }
    }

    fn to_bytes(&self) -> Vec<u8> {
        let mut result = vec![0x01]; // 0x01 is flag for passive segment
        result.append(&mut unsigned_leb128(self.data.len() as u32));
        result.extend_from_slice(&self.data);

        result
    }
}

pub struct Global {
    numtype: Numtype,
    mutable: bool,
    initial_value: u32,
}

impl Global {
    pub fn new(numtype: Numtype, mutable: bool, initial_value: u32) -> Self {
        Self {
            numtype,
            mutable,
            initial_value,
        }
    }
    
    fn to_bytes(&self) -> Vec<u8> {
        let mut result = vec![
            self.numtype as u8,
            self.mutable as u8,
            self.numtype.const_op() as u8,
        ];
        result.append(&mut unsigned_leb128(self.initial_value));
        result.push(Opcode::End as u8);
        result
    }
}

pub struct ModuleBuilder {
    // stores type signatures for each function
    pub functypes: Vec<FuncTypeSignature>,
    // indices to functypes for each function
    pub funcs: Vec<u32>,
    // bytecode for each function's body - order should match that of funcs
    pub func_bodies: Vec<Vec<u8>>,
    // information about functions, memory, globals, etc. to export
    exports: Vec<Export>,
    // passive data segments
    data_segments: Vec<DataSegment>,
    // information about globals
    globals: Vec<Global>,
    // bytecode for each import
    pub imports: Vec<Vec<u8>>,
}

impl Default for ModuleBuilder {
    fn default() -> Self {
        let mem_export = Export::new("memory".to_string(), 0, ExportType::Memory);
        Self {
            functypes: Vec::new(),
            funcs: vec![],
            func_bodies: vec![],
            exports: vec![mem_export],
            data_segments: vec![],
            globals: vec![
                // memptr
                Global::new(
                    Numtype::I32,
                    true,
                    0,
                ),
                // memsize
                Global::new(
                    Numtype::I32,
                    true,
                    1,
                ),
            ],
            imports: vec![],
        }
    }
}

impl ModuleBuilder {
    // adds a function definition, returns the function index
    pub fn add_function(
        &mut self,
        sig: &FuncTypeSignature,
        local_types: Vec<u8>,
        code: Vec<u8>,
        export_name: Option<String>,
    ) -> Result<u32, String> {
        let ftype = self.get_functype_idx(sig);
        let func_idx = (self.imports.len() + self.funcs.len()) as u32;
        if func_idx == u32::MAX {
            return Err(format!("too many functions"));
        }
        self.funcs.push(ftype);
        self.func_bodies.push(function_body(local_types, code));
        if let Some(name) = export_name {
            self.exports
                .push(Export::new(name, func_idx, ExportType::Func));
        }
        Ok(func_idx)
    }

    pub fn add_data(&mut self, data: Vec<u8>) -> Result<u32, String> {
        // first check if there is already a matching data segment
        // if so, return its index
        // if not, add it and return its index
        match self.data_segments.iter().enumerate().find_map(|(i, x)| {
            if x.data == data {
                Some(i)
            } else {
                None
            }
        }) {
            Some(i) => Ok(i as u32),
            None => {
                let idx = self.data_segments.len() as u32;
                if idx == u32::MAX {
                    return Err(format!("too many data segments"));
                }
                self.data_segments.push(DataSegment::new(data));
                Ok(idx)
            }
        }
    }

    pub fn add_import(&mut self, import: &Import) -> u32 {
        let module = encode_string(import.module);
        let field = encode_string(import.field);
        let ftype = unsigned_leb128(self.get_functype_idx(&import.sig));
        let bytes = [module, field, vec![0x00], ftype].concat();
        self.imports.push(bytes);
        self.imports.len() as u32 - 1
    }

    pub fn add_builtin(&mut self, func: &BuiltinFunc) -> Result<u32, String> {
        self.add_function(
            func.get_signature(),
            func.get_local_types(),
            func.get_bytes().to_vec(),
            None,
        )
    }

    pub fn get_functype_idx(&mut self, ftype: &FuncTypeSignature) -> u32 {
        match self
            .functypes
            .iter()
            .enumerate()
            .find_map(|(i, x)| if x == ftype { Some(i) } else { None })
        {
            Some(i) => i as u32,
            None => {
                self.functypes.push(ftype.clone());
                self.functypes.len() as u32 - 1
            }
        }
    }

    pub fn add_global(&mut self, global: Global) -> u32 {
        self.globals.push(global);
        self.globals.len() as u32 - 1
    }

    fn type_section(&self) -> Vec<u8> {
        section_from_chunks(
            SectionType::Type,
            self.functypes
                .iter()
                .map(|x| x.as_functype())
                .collect::<Vec<_>>()
                .as_slice(),
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
        section_from_chunks(
            SectionType::Table,
            &[vec![
                0x70,    // funcref
                0x00,    // minimum
                n_funcs, // maximum
            ]],
        )
    }

    fn memory_section(&self) -> Vec<u8> {
        section_from_chunks(
            SectionType::Memory,
            &[vec![
                0x00, // limit flag
                0x01, // initial size
            ]],
        )
    }

    fn global_section(&self) -> Vec<u8> {
        section_from_chunks(
            SectionType::Global,
            &self.globals.iter().map(|x| x.to_bytes()).collect::<Vec<_>>(),
        )
    }

    fn export_section(&self) -> Vec<u8> {
        section_from_chunks(
            SectionType::Export,
            self.exports
                .iter()
                .map(|x| x.as_export())
                .collect::<Vec<_>>()
                .as_slice(),
        )
    }

    fn elem_section(&self) -> Vec<u8> {
        let segments = (0..self.funcs.len())
            .map(|i| {
                [
                    &[0x00, Opcode::I32Const as u8],
                    unsigned_leb128(i as u32).as_slice(),
                    &[Opcode::End as u8, 0x01],
                    unsigned_leb128((i + self.imports.len()) as u32).as_slice(),
                ]
                .concat()
            })
            .collect::<Vec<_>>();
        section_from_chunks(SectionType::Element, &segments)
    }

    fn code_section(&self) -> Vec<u8> {
        section_from_chunks(SectionType::Code, &self.func_bodies)
    }

    fn data_section(&self) -> Vec<u8> {
        let data_chunks = self
            .data_segments
            .iter()
            .map(|d| d.to_bytes())
            .collect::<Vec<_>>();
        section_from_chunks(SectionType::Data, &data_chunks)
    }

    fn data_count_section(&self) -> Vec<u8> {
        let mut bytes = vec![SectionType::DataCount as u8];
        bytes.append(&mut vector(
            unsigned_leb128(self.data_segments.len() as u32),
        ));
        bytes
    }

    pub fn program(&self) -> Vec<u8> {
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
            self.data_count_section().as_slice(), // this needs to come before the code section even though SectionType::DataCount  > SectionType::Code
            self.code_section().as_slice(),
            self.data_section().as_slice(),
        ]
        .concat()
    }
}
