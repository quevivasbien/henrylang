use rustc_hash::FxHashMap;

use crate::{ast, compiler::TypeContext, parser, scanner};
use crate::env;
use super::module_builder::ModuleBuilder;
use super::{builtin_funcs, wasmtypes::*};

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
    builtins: FxHashMap<&'static str, u32>,  // name -> func index
}

impl Wasmizer {
    fn new(global_env: env::Env) -> Result<Self, String> {
        let mut builder: ModuleBuilder = Default::default();

        let mut builtins = FxHashMap::default();
        for import in global_env.imports.iter() {
            let idx = builder.add_import(import);
            builtins.insert(import.field, idx);
        }

        // add hard-coded builtin functions
        for (&name, func) in builtin_funcs::BUILTINS.iter() {
            let idx = builder.add_function(
                func.get_signature(),
                func.get_local_types(),
                func.get_bytes().to_vec(),
                None
            )?;
            builtins.insert(name, idx);
        }

        Ok(Self {
            typecontext: global_env.global_types,
            global_vars: global_env.global_vars,
            builder,
            frames: vec![],
            builtins
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

        let counter_idx = unsigned_leb128(self.locals_mut().add_local(format!("<counter>"), Numtype::I32 as u8));
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

        // continue if counter <= memptr
        self.write_opcode(Opcode::GlobalGet);
        self.write_byte(0x00);
        self.write_opcode(Opcode::I32LeU);
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
        Ok(())
    }
    pub fn finish_func(&mut self) -> Result<u32, String> {
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

    fn repeat(&mut self, bytes: &[u8], count: u32) -> Result<(), String> {
        // creates a for loop that repeats instructions in bytes count times
        let i = unsigned_leb128(self.locals_mut().add_local(format!("<i>"), Numtype::I32 as u8));
        // set i = count
        self.write_opcode(Opcode::I32Const);
        self.bytes_mut().append(&mut unsigned_leb128(count));
        self.write_opcode(Opcode::LocalSet);
        self.write_slice(&i);

        // begin loop
        self.write_opcode(Opcode::Loop);
        self.write_byte(Numtype::Void as u8);  // loop shouldn't add anything to stack
        
        // insert bytes
        self.write_slice(bytes);

        // subtract 1 from i
        self.write_opcode(Opcode::LocalGet);
        self.write_slice(&i);
        self.write_opcode(Opcode::I32Const);
        self.write_byte(0x01);
        self.write_opcode(Opcode::I32Sub);
        self.write_opcode(Opcode::LocalTee);
        self.write_slice(&i);

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
        self.write_slice(offset_idx);  // set as offset
        // size = lowest 32 bits of fatptr
        self.write_opcode(Opcode::LocalGet);
        self.write_slice(fatptr_idx);
        self.write_opcode(Opcode::I32WrapI64);  // discard high 32 bits
        self.write_opcode(Opcode::LocalSet);
        self.write_slice(size_idx);  // set as size
    }

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
        let fatptr1_idx = unsigned_leb128(self.locals_mut().add_local(format!("<fatptr1>"), Numtype::I64 as u8));
        let offset1_idx = unsigned_leb128(self.locals_mut().add_local(format!("<offset1>"), Numtype::I32 as u8));
        let size1_idx = unsigned_leb128(self.locals_mut().add_local(format!("<size1>"), Numtype::I32 as u8));

        let fatptr2_idx = unsigned_leb128(self.locals_mut().add_local(format!("<fatptr2>"), Numtype::I64 as u8));
        let offset2_idx = unsigned_leb128(self.locals_mut().add_local(format!("<offset2>"), Numtype::I32 as u8));
        let size2_idx = unsigned_leb128(self.locals_mut().add_local(format!("<size2>"), Numtype::I32 as u8));

        // set offsets and sizes from fatptrs
        self.write_opcode(Opcode::LocalTee);
        self.write_slice(&fatptr2_idx);
        self.set_offset_and_size(&fatptr2_idx, &offset2_idx, &size2_idx);
        
        self.write_opcode(Opcode::LocalTee);
        self.write_slice(&fatptr1_idx);
        self.set_offset_and_size(&fatptr1_idx, &offset1_idx, &size1_idx);

        // check if sizes are equal
        self.write_opcode(Opcode::LocalGet);
        self.write_slice(&size1_idx);
        self.write_opcode(Opcode::LocalGet);
        self.write_slice(&size2_idx);
        self.write_opcode(Opcode::I32Eq);
        self.write_opcode(Opcode::If);
        self.write_byte(Numtype::I32 as u8); // will return 1 if equal, 0 if not

        // case if sizes are equal
        // loop through all values and check if they are equal
        // <inner_offset> is used to store the index of the current value within the loop
        let inner_offset_idx = unsigned_leb128(self.locals_mut().add_local(format!("<loop>"), Numtype::I32 as u8));
        self.write_opcode(Opcode::I32Const);
        self.write_byte(0x00);
        self.write_opcode(Opcode::LocalSet);
        self.write_slice(&inner_offset_idx);
        // <equal> is used to store whether the values are equal, initialized to 1
        let equal_idx = unsigned_leb128(self.locals_mut().add_local(format!("<equal>"), Numtype::I32 as u8));
        self.write_opcode(Opcode::I32Const);
        self.write_byte(0x01);
        self.write_opcode(Opcode::LocalSet);
        self.write_slice(&equal_idx);
        self.write_opcode(Opcode::Loop);
        self.write_byte(Numtype::Void as u8);
        // read value from memory at offset1 + inner_offset
        self.write_opcode(Opcode::LocalGet);
        self.write_slice(&inner_offset_idx);
        self.write_opcode(Opcode::LocalGet);
        self.write_slice(&offset1_idx);
        self.write_opcode(Opcode::I32Add);
        self.write_opcode(Opcode::I32Load);
        self.write_byte(0x02);  // alignment
        self.write_byte(0x00);  // load offset
        // read value from memory at offset2 + inner_offset
        self.write_opcode(Opcode::LocalGet);
        self.write_slice(&inner_offset_idx);
        self.write_opcode(Opcode::LocalGet);
        self.write_slice(&offset2_idx);
        self.write_opcode(Opcode::I32Add);
        self.write_opcode(Opcode::I32Load);
        self.write_byte(0x02);  // alignment
        self.write_byte(0x00);  // load offset
        // compare values, update equal, keep that value on stack
        self.write_opcode(Opcode::I32Eq);
        self.write_opcode(Opcode::LocalTee);
        self.write_slice(&equal_idx);
        // add 4 to inner_offset
        self.write_opcode(Opcode::LocalGet);
        self.write_slice(&inner_offset_idx);
        self.write_opcode(Opcode::I32Const);
        self.write_byte(0x04);
        self.write_opcode(Opcode::I32Add);
        self.write_opcode(Opcode::LocalTee);
        self.write_slice(&inner_offset_idx);
        // check if inner_offset < size1
        self.write_opcode(Opcode::LocalGet);
        self.write_slice(&size1_idx);
        self.write_opcode(Opcode::I32LtU);
        // continue if inner_offset < size1 AND equal == 1
        self.write_opcode(Opcode::I32And);
        self.write_opcode(Opcode::BrIf);
        self.write_byte(0x00);  // break depth
        self.write_opcode(Opcode::End); // end loop
        // return <equal>
        self.write_opcode(Opcode::LocalGet);
        self.write_slice(&equal_idx);


        self.write_opcode(Opcode::Else);
        // case if sizes are not equal
        self.write_opcode(Opcode::I32Const);
        self.write_byte(0);


        self.write_opcode(Opcode::End); // end if

        Ok(())
    }

    pub fn write_array(&mut self, len: u16, typ: &ast::Type) -> Result<(), String> {
        // create locals to store info about array as it is constructed
        let startptr_idx = unsigned_leb128(self.locals_mut().add_local(format!("<startptr>"), Numtype::I32 as u8));
        let memptr_idx = unsigned_leb128(self.locals_mut().add_local(format!("<memptr>"), Numtype::I32 as u8));
        let value_idx = unsigned_leb128(self.locals_mut().add_local(format!("<value>"), Numtype::from_ast_type(typ)? as u8));
        
        // determine the bytes per value needed
        let memsize = match typ {
            ast::Type::Int | ast::Type::Bool | ast::Type::Float => 4,
            _ => return Err(format!("cannot allocate memory for array of type {:?}", typ))
        } as u8;
        // call alloc
        self.write_opcode(Opcode::I32Const);
        self.bytes_mut().append(&mut unsigned_leb128(len as u32 * memsize as u32));
        self.call_builtin("alloc")?;
        self.increment_n_allocs();
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

    fn write_string(&mut self, value: &str) -> Result<(), String> {
        let data = value[1..value.len() - 1].as_bytes().to_vec();
        let size = unsigned_leb128(data.len() as u32);
        let segment_idx = self.builder.add_data(data)?;
        let offset_idx = unsigned_leb128(self.locals_mut().add_local(format!("<offset>"), Numtype::I32 as u8));
        // destination is memptr + 4
        self.write_opcode(Opcode::GlobalGet);
        self.write_byte(0x00);
        self.write_opcode(Opcode::I32Const);
        self.write_byte(0x04);
        self.write_opcode(Opcode::I32Add);
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
        self.write_opcode(Opcode::I32Const);
        self.write_byte(0x04);
        self.write_opcode(Opcode::I32Add);
        self.write_opcode(Opcode::I32Add);
        self.write_opcode(Opcode::GlobalSet);
        self.write_byte(0x00);

        self.align_memptr();

        // set alloc size at memptr
        self.write_opcode(Opcode::GlobalGet);
        self.write_byte(0x00);
        // alloc size is memptr - offset
        self.write_opcode(Opcode::GlobalGet);
        self.write_byte(0x00);
        self.write_opcode(Opcode::LocalGet);
        self.write_slice(&offset_idx);
        self.write_opcode(Opcode::I32Sub);
        self.write_opcode(Opcode::I32Store);
        self.write_byte(0x02);  // alignment
        self.write_byte(0x00);  // offset

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

    fn free_multiple(&mut self, n_frees: u32) -> Result<(), String> {
        #[cfg(feature = "debug")]
        {
            self.print_mem();
        }

        // call free
        let mut bytes = Vec::with_capacity(2);
        let free_idx = match self.builtins.get("free") {
            Some(idx) => unsigned_leb128(*idx),
            None => return Err("Could not find 'free' builtin function".to_string()),
        };
        bytes.push(Opcode::Call as u8);
        bytes.extend_from_slice(&free_idx);
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
            self.copy_heap_object()?;
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
