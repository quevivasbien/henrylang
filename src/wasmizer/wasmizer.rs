use rustc_hash::FxHashMap;

use crate::{ast, compiler::TypeContext, parser, scanner};
use crate::env;
use super::builtin_funcs::BuiltinFunc;
use super::module_builder::ModuleBuilder;
use super::{builtin_funcs, structs::Struct, wasmtypes::*};

#[derive(Debug)]
struct Local {
    name: String,
    depth: i32,
    index: u32,
}

#[derive(Debug)]
struct LocalData {
    locals: Vec<Local>,
    types: Vec<u8>,
    scope_depth: i32,
    n_locals: u32, // stores total number of locals declared in this function
}

impl Default for LocalData {
    fn default() -> Self {
        Self {
            locals: Vec::new(),
            types: Vec::new(),
            scope_depth: -1,
            n_locals: 0,
        }
    }
}

impl LocalData {
    fn add_local(&mut self, name: String, typ: u8) -> u32 {
        let index = self.n_locals;
        self.n_locals += 1;
        self.locals.push(Local { name, depth: self.scope_depth, index });
        self.types.push(typ);
        index
    }
    fn get_idx(&self, name: &str) -> Option<u32> {
        self.locals.iter().rev().find_map(|local| if local.name == name { Some(local.index) } else { None })
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
    fn get_local_idx(&self, name: &str) -> Option<u32> {
        self.locals.get_idx(name).map(|x| x + self.n_params())
    }
}

pub struct Wasmizer {
    pub typecontext: TypeContext,
    pub global_vars: env::GlobalVars,
    builder: ModuleBuilder,
    frames: Vec<WasmFunc>,
    builtins: FxHashMap<String, u32>,  // name -> func index
    structs: FxHashMap<String, Struct>,
}

impl Wasmizer {
    fn new(global_env: env::Env) -> Result<Self, String> {
        let mut builder: ModuleBuilder = Default::default();

        let mut builtins = FxHashMap::default();
        for import in global_env.imports.iter() {
            let idx = builder.add_import(import);
            builtins.insert(import.field.to_string(), idx);
        }

        // add hard-coded builtin functions
        for (name, func) in builtin_funcs::BUILTINS.iter() {
            let idx = builder.add_builtin(func)?;
            builtins.insert(name.to_string(), idx);
        }

        Ok(Self {
            typecontext: global_env.global_types,
            global_vars: global_env.global_vars,
            builder,
            frames: vec![],
            builtins,
            structs: FxHashMap::default(),
        })
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
    fn write_slice(&mut self, slice: &[u8]) {
        self.bytes_mut().extend_from_slice(slice);
    }

    fn call_builtin(&mut self, name: &str) -> Result<(), String> {
        let idx = match self.builtins.get(name) {
            Some(idx) => *idx,
            None => return Err(format!("Unknown builtin function: {}", name))
        };
        self.write_opcode(Opcode::Call);
        self.bytes_mut().append(&mut unsigned_leb128(idx));
        Ok(())
    }

    fn increment_n_allocs(&mut self) {
        *self.current_func_mut().allocs.last_mut().unwrap() += 1;
    }

    fn align_memptr(&mut self) {
        // Align memptr to 4 bytes
        // 3 + memptr
        self.write_opcode(Opcode::GlobalGet);
        self.write_byte(0x00);
        self.write_opcode(Opcode::I32Const);
        self.write_byte(0x03);
        self.write_opcode(Opcode::I32Add);
        // (3 + memptr) % 4
        self.write_opcode(Opcode::GlobalGet);
        self.write_byte(0x00);
        self.write_opcode(Opcode::I32Const);
        self.write_byte(0x03);
        self.write_opcode(Opcode::I32Add);
        self.write_opcode(Opcode::I32Const);
        self.write_byte(0x04);
        self.write_opcode(Opcode::I32RemU);
        // memptr = 3 + memptr - (3 + memptr) % 4
        self.write_opcode(Opcode::I32Sub);
        self.write_opcode(Opcode::GlobalSet);
        self.write_byte(0x00);
    }

    // for debugging
    #[allow(dead_code)]
    fn print_i32_local(&mut self, local_idx: &[u8]) {
        self.write_opcode(Opcode::LocalGet);
        self.write_slice(&local_idx);
        self.call_builtin("print[Int]").unwrap();
        self.write_opcode(Opcode::Drop);
    }

    // for debugging
    #[allow(dead_code)]
    fn print_mem(&mut self) {
        // print 8888 twice to show that this is what's going on
        self.write_opcode(Opcode::I32Const);
        self.write_slice(&signed_leb128(8888));
        self.call_builtin("print[Int]").unwrap();
        self.call_builtin("print[Int]").unwrap();
        self.write_opcode(Opcode::Drop);

        let counter_idx = self.add_local("<counter>", Numtype::I32);
        // set counter = 0
        self.write_opcode(Opcode::I32Const);
        self.write_byte(0x00);
        self.write_opcode(Opcode::LocalSet);
        self.write_slice(&counter_idx);

        // start loop
        self.write_opcode(Opcode::Loop);
        self.write_byte(Numtype::Void as u8);

        // read memory at counter
        self.write_opcode(Opcode::LocalGet);
        self.write_slice(&counter_idx);
        self.write_opcode(Opcode::I32Load);
        self.write_byte(0x02);  // alignment
        self.write_byte(0x00);  // load offset

        // print it
        self.call_builtin("print[Int]").unwrap();
        self.write_opcode(Opcode::Drop);

        // add 4 to counter
        self.write_opcode(Opcode::LocalGet);
        self.write_slice(&counter_idx);
        self.write_opcode(Opcode::I32Const);
        self.write_byte(0x04);
        self.write_opcode(Opcode::I32Add);
        self.write_opcode(Opcode::LocalTee);
        self.write_slice(&counter_idx);

        // continue if counter < memptr
        self.write_opcode(Opcode::GlobalGet);
        self.write_byte(0x00);
        self.write_opcode(Opcode::I32LtU);
        self.write_opcode(Opcode::BrIf);
        self.write_byte(0x00);  // break depth
        self.write_opcode(Opcode::End);

        // print -8888 twice to show that we're done
        self.write_opcode(Opcode::I32Const);
        self.write_slice(&signed_leb128(-8888));
        self.call_builtin("print[Int]").unwrap();
        self.call_builtin("print[Int]").unwrap();
        self.write_opcode(Opcode::Drop);
    }

    pub fn init_func(&mut self, name: String, argtypes: &[ast::Type], rettype: &ast::Type, export: bool) -> Result<(), String> {
        let args = argtypes.iter().map(|t| Numtype::from_ast_type(t)).collect::<Result<Vec<_>, _>>()?;
        let ret = Numtype::from_ast_type(rettype)?;
        let func = WasmFunc::new(name, FuncTypeSignature::new(args, Some(ret)), export);
        self.frames.push(func);

        // create a local to store the value of the memptr at the start of the function.
        // at the end of the function, the memptr will be set back to this value
        let idx = self.add_local("<memptr_at_start>", Numtype::I32);
        self.write_opcode(Opcode::GlobalGet);
        self.write_byte(0x00);
        self.write_opcode(Opcode::LocalSet);
        self.write_slice(&idx);

        Ok(())
    }
    pub fn finish_func(&mut self) -> Result<u32, String> {
        #[cfg(feature = "debug")]
        self.print_mem();
        // set memptr back to what it was at the start of the function
        let memptr_at_start_idx = self.current_func().get_local_idx("<memptr_at_start>").unwrap();
        self.write_opcode(Opcode::LocalGet);
        self.write_slice(&unsigned_leb128(memptr_at_start_idx));
        self.write_opcode(Opcode::GlobalSet);
        self.write_byte(0x00);
        #[cfg(feature = "debug")]
        {
            // print memptr
            self.write_opcode(Opcode::GlobalGet);
            self.write_byte(0x00);
            self.call_builtin("print[Int]").unwrap();
            self.write_opcode(Opcode::Drop);
        }

        // write end
        self.current_func_mut().bytes.push(Opcode::End as u8);

        // add to exports if needed
        let export_name = if self.current_func().export {
            Some(self.current_func().name.clone())
        }
        else {
            None
        };

        // pop from frames and add to ModuleBuilder
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
                self.write_slice(&value.to_le_bytes());
            }
            ast::Type::Bool => {
                let value = value.parse::<bool>().unwrap();
                self.write_opcode(Opcode::I32Const);
                self.write_byte(if value { 1 } else { 0 });
            }
            ast::Type::Str => {
                self.write_string(value)?
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

    pub fn write_collect(&mut self, typ: &ast::Type) -> Result<(), String> {
        let numtype = Numtype::from_ast_type(typ)?;
        match numtype {
            Numtype::I32 => {
                if self.builtins.get("collect_i32").is_none() {
                    self.init_collect_i32()?;
                }
                self.call_builtin("collect_i32").unwrap();
            }
            _ => unimplemented!()
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
            ast::Type::Str | ast::Type::Arr(_) => {
                self.arrays_equal()?;
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
                self.concat_heap_objects()?;
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

    pub fn write_range(&mut self, typ: &ast::Type) -> Result<(), String> {
        if typ != &ast::Type::Int {
            return Err(format!("Cannot create range of values of type {:?}. Ranges must be between integers.", typ));
        }

        if self.builtins.get("<RangeIter>").is_none() {
            self.init_range_iter()?;
        };
        let factory = unsigned_leb128(*self.builtins.get("<RangeIterFactory>").unwrap());

        self.write_opcode(Opcode::Call);
        self.write_slice(&factory);

        Ok(())
    }

    fn add_local(&mut self, name: &str, numtype: Numtype) -> Vec<u8> {
        let i = self.locals_mut().add_local(name.to_string(), numtype as u8) + self.current_func().n_params();
        unsigned_leb128(i)
    }

    // fn repeat(&mut self, bytes: &[u8], count: u32) -> Result<(), String> {
    //     // creates a for loop that repeats instructions in bytes count times
    //     let i = self.add_local("<i>", Numtype::I32);
    //     // set i = count
    //     self.write_opcode(Opcode::I32Const);
    //     self.bytes_mut().append(&mut unsigned_leb128(count));
    //     self.write_opcode(Opcode::LocalSet);
    //     self.write_slice(&i);

    //     // begin loop
    //     self.write_opcode(Opcode::Loop);
    //     self.write_byte(Numtype::Void as u8);  // loop shouldn't add anything to stack
        
    //     // insert bytes
    //     self.write_slice(bytes);

    //     // subtract 1 from i
    //     self.write_opcode(Opcode::LocalGet);
    //     self.write_slice(&i);
    //     self.write_opcode(Opcode::I32Const);
    //     self.write_byte(0x01);
    //     self.write_opcode(Opcode::I32Sub);
    //     self.write_opcode(Opcode::LocalTee);
    //     self.write_slice(&i);

    //     // continue if i != 0
    //     self.write_opcode(Opcode::BrIf);
    //     self.write_byte(0x00);  // break depth
    //     self.write_opcode(Opcode::End);
    //     Ok(())
    // }

    // assumes fatptr to object to copy is last thing on stack
    fn copy_heap_object(&mut self) -> Result<(), String> {
        self.call_builtin("copy_heap_obj")?;
        self.increment_n_allocs();

        Ok(())
    }

    // concatenate two heap objects (e.g. arrays, strings)
    // the fatptrs indices to the objects should be the two last things on the stack when calling this
    fn concat_heap_objects(&mut self) -> Result<(), String> {
        self.call_builtin("concat_heap_objs")?;
        self.increment_n_allocs();
        Ok(())
    }

    // check if two arrays or strings are equal
    // expects two previous values on the stack to be both i64 fatptrs
    fn arrays_equal(&mut self) -> Result<(), String> {
        self.call_builtin("heap_objs_equal")
    }

    pub fn write_array(&mut self, len: u16, typ: &ast::Type) -> Result<(), String> {
        let numtype = Numtype::from_ast_type(typ)?;
        // create locals to store info about array as it is constructed
        let startptr_idx = self.add_local("<startptr>", Numtype::I32);
        let memptr_idx = self.add_local("<memptr>", Numtype::I32);
        let value_idx = self.add_local("<value>", numtype);
        
        // determine the bytes per value needed
        let memsize = numtype.size();
        // call alloc
        self.write_opcode(Opcode::I32Const);
        self.bytes_mut().append(&mut unsigned_leb128(len as u32 * memsize));
        self.call_builtin("alloc")?;
        self.increment_n_allocs();
        // alloc will return index of start pos -- set startptr and memptr to that index
        self.write_opcode(Opcode::LocalTee);
        self.bytes_mut().extend(&startptr_idx);
        self.write_opcode(Opcode::LocalSet);
        self.bytes_mut().extend(&memptr_idx);

        // write vars on stack to memory
        // figure out what opcode to use to store values in memory
        let store_op = match numtype {
            Numtype::I32 => Opcode::I32Store,
            Numtype::F32 => Opcode::F32Store,
            _ => Opcode::I64Store,
        };
        // TODO: If values are heap types, they need to be copied here
        for _ in 0..len {
            self.write_to_memory(&memptr_idx, &value_idx, store_op, memsize as u8)?;
        }

        // return [startptr len]
        self.write_opcode(Opcode::LocalGet);
        self.bytes_mut().extend(&startptr_idx);
        self.write_opcode(Opcode::I64ExtendI32U);
        self.write_opcode(Opcode::I64Const);
        self.write_byte(32);
        self.write_opcode(Opcode::I64Shl);
        self.write_opcode(Opcode::I64Const);
        self.bytes_mut().append(&mut unsigned_leb128(len as u32 * memsize));
        self.write_opcode(Opcode::I64Add);
        Ok(())
    }

    fn write_string(&mut self, value: &str) -> Result<(), String> {
        let data = value[1..value.len() - 1].as_bytes().to_vec();
        let size = unsigned_leb128(data.len() as u32);
        let segment_idx = self.builder.add_data(data)?;
        let offset_idx = self.add_local("<offset>", Numtype::I32);
        // destination is memptr
        self.write_opcode(Opcode::GlobalGet);
        self.write_byte(0x00);
        self.write_opcode(Opcode::LocalTee);
        self.write_slice(&offset_idx);  // we'll use this later
        // offset in source is 0
        self.write_opcode(Opcode::I32Const);
        self.write_byte(0x00);
        // size of memory region is data.len()
        self.write_opcode(Opcode::I32Const);
        self.write_slice(&size);
        // write memory.init
        self.write_slice(&MEMINIT);
        self.write_slice(&unsigned_leb128(segment_idx));
        self.write_byte(0x00);  // idx of memory (always zero)

        // update memptr
        self.write_opcode(Opcode::GlobalGet);
        self.write_byte(0x00);
        self.write_opcode(Opcode::I32Const);
        self.write_slice(&size);
        self.write_opcode(Opcode::I32Add);
        self.write_opcode(Opcode::GlobalSet);
        self.write_byte(0x00);

        self.align_memptr();

        self.increment_n_allocs();

        // return [offset, size]
        self.write_opcode(Opcode::LocalGet);
        self.write_slice(&offset_idx);
        self.write_opcode(Opcode::I64ExtendI32U);
        self.write_opcode(Opcode::I64Const);
        self.write_byte(32);
        self.write_opcode(Opcode::I64Shl);
        self.write_opcode(Opcode::I64Const);
        self.write_slice(&size);
        self.write_opcode(Opcode::I64Add);
        Ok(())
    }

    fn write_to_memory(&mut self, memptr_idx: &[u8], value_idx: &[u8], store_op: Opcode, memsize: u8) -> Result<(), String> {
        // set local var to last value on stack
        self.write_opcode(Opcode::LocalSet);
        self.write_slice(value_idx);

        // write index of local var that tells where in memory to write this value
        self.write_opcode(Opcode::LocalGet);
        self.write_slice(memptr_idx);
        // write index of local var that contains the value to be written
        self.write_opcode(Opcode::LocalGet);
        self.write_slice(value_idx);

        self.write_byte(store_op as u8);
        self.write_byte(0x02);  // alignment
        self.write_byte(0x00);  // offset

        // increase memptr by memsize
        self.write_opcode(Opcode::LocalGet);
        self.write_slice(memptr_idx);
        self.write_opcode(Opcode::I32Const);
        self.write_byte(memsize);
        self.write_opcode(Opcode::I32Add);
        self.write_opcode(Opcode::LocalSet);
        self.write_slice(memptr_idx);

        Ok(())
    }

    // fn free_multiple(&mut self, n_frees: u32) -> Result<(), String> {
    //     #[cfg(feature = "debug")]
    //     {
    //         self.print_mem();
    //     }

    //     // call free
    //     let mut bytes = Vec::with_capacity(2);
    //     let free_idx = match self.builtins.get("free") {
    //         Some(idx) => unsigned_leb128(*idx),
    //         None => return Err("Could not find 'free' builtin function".to_string()),
    //     };
    //     bytes.push(Opcode::Call as u8);
    //     bytes.extend_from_slice(&free_idx);
    //     self.repeat(&bytes, n_frees)
    // }

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
        // // free memory
        // let n_frees = self.current_func_mut().allocs.pop().unwrap();
        // if n_frees > 0 {
        //     self.free_multiple(n_frees)?;
        // }
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
        Ok(self.locals_mut().add_local(name, typ) + self.current_func().n_params())
    }
    pub fn set_variable(&mut self, idx: u32, typ: &ast::Type) -> Result<(), String> {
        // if typ is a heap-allocated type, we also need to copy the memory
        if Numtype::from_ast_type(typ)? == Numtype::I64 {
            self.copy_heap_object()?;
        }
        self.write_opcode(Opcode::LocalTee);
        self.bytes_mut().append(&mut unsigned_leb128(idx));

        Ok(())
    }
    pub fn get_variable(&mut self, name: String) -> Result<i32, String> {
        // can't yet deal with upvalues
        // first look in local variables
        let idx = self.current_func().get_local_idx(&name);
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

    // get an entry from an array.
    // last two values on the stack should be the array fatptr and the index to get
    pub fn get_array_entry(&mut self, array_type: &ast::Type) -> Result<(), String> {
        let numtype = Numtype::from_ast_type(array_type)?;
        // calculate the memory offset as index * size_of_array_element
        self.write_opcode(Opcode::I32Const);
        self.write_byte(numtype.size() as u8);
        self.write_opcode(Opcode::I32Mul);
        match numtype {
            Numtype::I32 => self.call_builtin("get_i32_field"),
            Numtype::I64 => self.call_builtin("get_i64_field"),
            Numtype::F32 => self.call_builtin("get_f32_field"),
            _ => unreachable!()
        }?;

        Ok(())
    }

    pub fn write_if(&mut self, typ: &ast::Type) -> Result<(), String> {
        self.write_opcode(Opcode::If);
        self.write_byte(Numtype::from_ast_type(typ)? as u8);
        Ok(())
    }
    pub fn write_else(&mut self) -> Result<(), String> {
        self.write_opcode(Opcode::Else);
        Ok(())
    }

    pub fn write_end(&mut self) -> Result<(), String> {
        self.write_opcode(Opcode::End);
        Ok(())
    }

    // take a struct definition and creates a constructor for that struct
    // adds struct to Wasmizer's list of struct definitions
    // optionally adds struct constructor idx to stack
    pub fn create_struct(&mut self, struct_name: String, struct_def: Struct, write_constructor: bool) -> Result<(), String> {
        let mut fieldnames = Vec::with_capacity(struct_def.fields.len());
        let mut fieldtypes = Vec::with_capacity(struct_def.fields.len());
        for (name, field) in struct_def.fields.iter() {
            fieldnames.push(name.clone());
            fieldtypes.push(field.nt);
        }
        let mut func = BuiltinFunc::new(
            FuncTypeSignature::new(fieldtypes, Some(Numtype::I64)),
            fieldnames,
        );

        // copy memory for all the heap fields
        let copy_idx = unsigned_leb128(*self.builtins.get("copy_heap_obj").unwrap());
        for (name, field) in struct_def.fields.iter() {
            if !field.t.is_heap() || field.nt != Numtype::I64 {
                continue;
            }
            func.write_opcode(Opcode::LocalGet);
            func.write_var(name);
            func.write_opcode(Opcode::Call);
            func.write_slice(&copy_idx);
            func.write_opcode(Opcode::LocalSet);
            func.write_var(name);
        }

        func.add_local("<offset>", Numtype::I32);
        func.add_local("<size>", Numtype::I32);
        // allocate memory for the struct
        let alloc_idx = unsigned_leb128(*self.builtins.get("alloc").unwrap());
        func.write_opcode(Opcode::I32Const);
        func.write_slice(&unsigned_leb128(struct_def.size));
        func.write_opcode(Opcode::LocalTee);
        func.write_var("<size>");
        func.write_opcode(Opcode::Call);
        func.write_slice(&alloc_idx);
        func.write_opcode(Opcode::LocalSet);
        func.write_var("<offset>");
        // TODO: n_allocs needs to be incremented whenever this constructor is called

        // write variables to memory
        for (name, field) in struct_def.fields.iter() {
            // set *(offset + field.offset) = value of name
            func.write_opcode(Opcode::LocalGet);
            func.write_var("<offset>");
            func.write_opcode(Opcode::I32Const);
            func.write_slice(&unsigned_leb128(field.offset));
            func.write_opcode(Opcode::I32Add);
            func.write_opcode(Opcode::LocalGet);
            func.write_var(name);
            let store_op = match field.nt {
                Numtype::I32 => Opcode::I32Store,
                Numtype::F32 => Opcode::F32Store,
                Numtype::I64 => Opcode::I64Store,
                _ => unreachable!("Other numtypes should not be possible here"),
            };
            func.write_opcode(store_op);
            func.write_byte(0x02);
            func.write_byte(0x00);
        }

        // return fatptr
        func.create_fatptr("<offset>", "<size>");

        func.write_opcode(Opcode::End);

        // add to list of builtin functions
        let idx = self.builder.add_builtin(&func)?;
        self.builtins.insert(struct_name.clone(), idx);

        // add to definitions
        self.structs.insert(struct_name, struct_def);
       
        if write_constructor {
            // add constructor to stack
            self.write_last_func_index();
        }
        Ok(())
    }

    // get the field of a struct
    // fatptr to struct should be on top of stack when calling this
    pub fn get_field(&mut self, object_type: ast::Type, field_name: &str) -> Result<(), String> {
        // first, figure out which struct type this is for
        let (struct_name, _) = match object_type {
            ast::Type::Object(name, fields) => (name, fields),
            _ => unreachable!()
        };
        let struct_def = self.structs.get(&struct_name).unwrap();
        let field = struct_def.fields.iter().find(|(name, _)| name == field_name).unwrap().1.clone();

        let field_offset = unsigned_leb128(field.offset);
        self.write_opcode(Opcode::I32Const);
        self.write_slice(&field_offset);
        match field.nt {
            Numtype::I32 => self.call_builtin("get_i32_field"),
            Numtype::F32 => self.call_builtin("get_f32_field"),
            Numtype::I64 => self.call_builtin("get_i64_field"),
            _ => unreachable!(),
        }?;
        Ok(())
    }

    // create a struct type that is used to store ranges
    fn init_range_iter(&mut self) -> Result<(), String> {
        // add the struct definition and constructor
        let struct_def = Struct::new(
            vec![
                ("current".to_string(), ast::Type::Bool),
                ("advance_fn".to_string(), ast::Type::Int),
                ("step".to_string(), ast::Type::Int),
                ("stop".to_string(), ast::Type::Int),
            ],
        );

        self.create_struct("<RangeIter>".to_string(), struct_def, false)?;
        let constructor_idx = *self.builtins.get("<RangeIter>").unwrap();

        // initialize the "advance" function used by this Iter type
        // function will take the offset of the start of a range struct, update the "current" field, and return a bool (1 if the iterator is done)
        let mut func = BuiltinFunc::new(
            FuncTypeSignature::new(vec![Numtype::I32], Some(Numtype::I32)),
            vec!["offset".to_string()],
        );
        func.add_local("current", Numtype::I32);
        func.add_local("step", Numtype::I32);
        func.add_local("stop", Numtype::I32);
        // add "step" to "current"
        // load value at "step"
        func.write_opcode(Opcode::LocalGet);
        func.write_var("offset");  // step = *(offset + 2 * 4)
        func.write_opcode(Opcode::I32Const);
        func.write_byte(2 * 4);
        func.write_opcode(Opcode::I32Add);
        func.write_opcode(Opcode::I32Load);
        func.write_slice(&[0x02, 0x00]);
        func.write_opcode(Opcode::LocalTee);
        func.write_var("step");
        // load value at current
        func.write_opcode(Opcode::LocalGet);
        func.write_var("offset");
        func.write_opcode(Opcode::I32Load);
        func.write_slice(&[0x02, 0x00]);
        // add
        func.write_opcode(Opcode::I32Add);
        // set as new value of current, and store new value
        func.write_opcode(Opcode::LocalSet);
        func.write_var("current");
        func.write_opcode(Opcode::LocalGet);
        func.write_var("offset");
        func.write_opcode(Opcode::LocalGet);
        func.write_var("current");
        func.write_opcode(Opcode::I32Store);
        func.write_slice(&[0x02, 0x00]);

        // return (current > stop && step > 0) || (current < stop && step < 0)
        // (1 if done, 0 otherwise)
        // stop = *(offset + 3 * 4)
        func.write_opcode(Opcode::LocalGet);
        func.write_var("offset");
        func.write_opcode(Opcode::I32Const);
        func.write_byte(3 * 4);
        func.write_opcode(Opcode::I32Add);
        func.write_opcode(Opcode::I32Load);
        func.write_slice(&[0x02, 0x00]);
        func.write_opcode(Opcode::LocalSet);
        func.write_var("stop");
        // current > stop && step > 0
        func.write_opcode(Opcode::LocalGet);
        func.write_var("current");
        func.write_opcode(Opcode::LocalGet);
        func.write_var("stop");
        func.write_opcode(Opcode::I32GtS);
        func.write_opcode(Opcode::LocalGet);
        func.write_var("step");
        func.write_opcode(Opcode::I32Const);
        func.write_byte(0x00);
        func.write_opcode(Opcode::I32GtS);
        func.write_opcode(Opcode::I32And);
        // current < stop && step < 0
        func.write_opcode(Opcode::LocalGet);
        func.write_var("current");
        func.write_opcode(Opcode::LocalGet);
        func.write_var("stop");
        func.write_opcode(Opcode::I32LtS);
        func.write_opcode(Opcode::LocalGet);
        func.write_var("step");
        func.write_opcode(Opcode::I32Const);
        func.write_byte(0x00);
        func.write_opcode(Opcode::I32LtS);
        func.write_opcode(Opcode::I32And);
        // (current > stop && step > 0) || (current < stop && step < 0)
        func.write_opcode(Opcode::I32Or);

        func.write_opcode(Opcode::End);

        let advance_fn_idx = self.builder.add_builtin(&func)?;
        self.builtins.insert("<RangeIterAdvance>".to_string(), advance_fn_idx);

        // create helper function for building range iterators from `<start> to <stop>` syntax
        let mut func = BuiltinFunc::new(
            FuncTypeSignature::new(vec![Numtype::I32, Numtype::I32], Some(Numtype::I64)),
            vec!["start".to_string(), "stop".to_string()],
        );
        func.add_local("step", Numtype::I32);
        // step = 1 if stop > start else -1
        func.write_opcode(Opcode::LocalGet);
        func.write_var("start");
        func.write_opcode(Opcode::LocalGet);
        func.write_var("stop");
        func.write_opcode(Opcode::I32LtS);
        func.write_opcode(Opcode::If);
        func.write_byte(Numtype::I32 as u8);
        func.write_opcode(Opcode::I32Const);
        func.write_byte(1);
        func.write_opcode(Opcode::Else);
        func.write_opcode(Opcode::I32Const);
        func.write_slice(&signed_leb128(-1));
        func.write_opcode(Opcode::End);
        func.write_opcode(Opcode::LocalSet);
        func.write_var("step");

        // pass values to constructor
        // current
        func.write_opcode(Opcode::LocalGet);
        func.write_var("start");
        // advance_fn
        func.write_opcode(Opcode::I32Const);
        func.write_slice(&unsigned_leb128(advance_fn_idx - self.builder.imports.len() as u32));  // need to subtract number of imports, since this will be called indirectly, and imports are not included in the function table
        // step
        func.write_opcode(Opcode::LocalGet);
        func.write_var("step");
        // stop
        func.write_opcode(Opcode::LocalGet);
        func.write_var("stop");
        func.write_opcode(Opcode::Call);
        func.write_slice(&unsigned_leb128(constructor_idx));

        func.write_opcode(Opcode::End);

        let factory_idx = self.builder.add_builtin(&func)?;
        self.builtins.insert("<RangeIterFactory>".to_string(), factory_idx);

        Ok(())
    }

    // create the `collect_i32` builtin (used for the @ operator on Iter(Int) or Iter(Func(..)) types)
    fn init_collect_i32(&mut self) -> Result<(), String> {
        let mut func = BuiltinFunc::new(
            // takes an Iter and returns an Arr
            FuncTypeSignature::new(vec![Numtype::I64], Some(Numtype::I64)),
            vec!["iter_fatptr".to_string()],
        );
        func.add_local("iter_offset", Numtype::I32);
        func.add_local("array_offset", Numtype::I32);
        func.add_local("element_offset", Numtype::I32);
        func.add_local("alloc_size", Numtype::I32);
        func.add_local("current", Numtype::I32);

        // iter_offset = iter_fatpr >> 32
        func.write_opcode(Opcode::LocalGet);
        func.write_var("iter_fatptr");
        func.write_opcode(Opcode::I64Const);
        func.write_byte(0x20);
        func.write_opcode(Opcode::I64ShrU);
        func.write_opcode(Opcode::I32WrapI64);
        func.write_opcode(Opcode::LocalSet);
        func.write_var("iter_offset");

        // start by allocating space for 1 element
        func.write_opcode(Opcode::I32Const);
        func.write_byte(1 * 4);
        func.write_opcode(Opcode::Call);
        let alloc_idx = unsigned_leb128(*self.builtins.get("alloc").unwrap());
        func.write_slice(&alloc_idx);
        func.write_opcode(Opcode::LocalTee);
        func.write_var("array_offset");
        func.write_opcode(Opcode::LocalSet);
        func.write_var("element_offset");

        func.write_opcode(Opcode::I32Const);
        func.write_byte(2 * 4);
        func.write_opcode(Opcode::LocalSet);
        func.write_var("alloc_size");

        // loop:
        // *element_offset = iter_offset->current
        // element_offset += 4
        // if element_offset >= memptr, allocate more space
        // if iterator->advance(), break

        func.write_opcode(Opcode::Loop);
        func.write_byte(Numtype::Void as u8);

        // read current value
        func.write_opcode(Opcode::LocalGet);
        func.write_var("iter_offset");
        func.write_opcode(Opcode::I32Load);
        func.write_slice(&[0x02, 0x00]);
        func.write_opcode(Opcode::LocalSet);
        func.write_var("current");

        // store current at element_offset
        func.write_opcode(Opcode::LocalGet);
        func.write_var("element_offset");
        func.write_opcode(Opcode::LocalGet);
        func.write_var("current");
        func.write_opcode(Opcode::I32Store);
        func.write_slice(&[0x02, 0x00]);

        // add 4 to element_offset
        func.write_opcode(Opcode::LocalGet);
        func.write_var("element_offset");
        func.write_opcode(Opcode::I32Const);
        func.write_byte(4);
        func.write_opcode(Opcode::I32Add);
        func.write_opcode(Opcode::LocalTee);
        func.write_var("element_offset");

        // if element_offset >= memptr, allocate more space
        func.write_opcode(Opcode::GlobalGet);
        func.write_byte(0x00);
        func.write_opcode(Opcode::I32GeU);
        func.write_opcode(Opcode::If);
        func.write_byte(Numtype::Void as u8);

        // allocate
        func.write_opcode(Opcode::LocalGet);
        func.write_var("alloc_size");
        func.write_opcode(Opcode::Call);
        func.write_slice(&alloc_idx);
        func.write_opcode(Opcode::Drop);

        // double alloc size
        func.write_opcode(Opcode::LocalGet);
        func.write_var("alloc_size");
        func.write_opcode(Opcode::I32Const);
        func.write_byte(2);
        func.write_opcode(Opcode::I32Mul);
        func.write_opcode(Opcode::LocalSet);
        func.write_var("alloc_size");

        func.write_opcode(Opcode::End);  // end if

        // call iterator->advance
        func.write_opcode(Opcode::LocalGet);
        func.write_var("iter_offset");
        func.write_opcode(Opcode::LocalGet);
        func.write_var("iter_offset");
        func.write_opcode(Opcode::I32Const);
        func.write_byte(4);
        func.write_opcode(Opcode::I32Add);
        func.write_opcode(Opcode::I32Load);
        func.write_slice(&[0x02, 0x00]);
        let functype_idx = self.builder.get_functype_idx(&FuncTypeSignature::new(vec![Numtype::I32], Some(Numtype::I32)));
        func.write_opcode(Opcode::CallIndirect);
        func.write_slice(&mut unsigned_leb128(functype_idx));  // signature index
        func.write_byte(0x00);  // table index

        // branch if result != 1
        func.write_opcode(Opcode::I32Eqz);
        func.write_opcode(Opcode::BrIf);
        func.write_byte(0x00);

        func.write_opcode(Opcode::End);  // end loop

        // return [array_offset, element_offset - array_offset]
        func.write_opcode(Opcode::LocalGet);
        func.write_var("element_offset");
        func.write_opcode(Opcode::LocalGet);
        func.write_var("array_offset");
        func.write_opcode(Opcode::I32Sub);
        func.write_opcode(Opcode::LocalSet);
        func.write_var("element_offset");  // use element offset again to store size
        func.create_fatptr("array_offset", "element_offset");

        func.write_opcode(Opcode::End);  // end function

        let fn_idx = self.builder.add_builtin(&func)?;
        self.builtins.insert("collect_i32".to_string(), fn_idx);

        Ok(())
    }

    fn to_bytes(&self) -> Vec<u8> {
        self.builder.program()
    }
}

pub fn wasmize(source: &str, global_env: env::Env) -> Result<(Vec<u8>, ast::Type), String> {
    let tokens = scanner::scan(source);
    let ast = parser::parse(tokens, global_env.global_types.clone()).map_err(|_| "Compilation halted due to parsing error.")?;
    #[cfg(feature = "debug")]
    println!("{:?}", ast);
    let mut wasmizer = Wasmizer::new(global_env)?;
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
