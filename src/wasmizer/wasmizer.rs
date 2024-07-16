use rustc_hash::FxHashMap;

use super::module_builder::{Global, ModuleBuilder};
use super::{builtin_funcs, structs::Struct, wasmtypes::*};
use crate::env;
use crate::{ast, compiler::TypeContext, parser, scanner};

#[derive(Debug)]
struct Local {
    // The local variable's name
    name: String,
    // The scope depth at which the local variable was declared
    depth: i32,
    // The index within the function's local data
    index: u32,
    // Whether or not a global variable is set to copy the value of this variable (for use as an upvalue);
    // if so, the index of the global variable that shadows it
    global_shadow: Option<u32>,
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
        self.locals.push(Local {
            name,
            depth: self.scope_depth,
            index,
            global_shadow: None,
        });
        self.types.push(typ);
        index
    }
    fn get_idx(&self, name: &str) -> Option<u32> {
        self.locals.iter().rev().find_map(|local| {
            if local.name == name {
                Some(local.index)
            } else {
                None
            }
        })
    }
}

struct Param {
    name: String,
    global_shadow: Option<u32>,
}

impl Param {
    fn new(name: String) -> Self {
        Self {
            name,
            global_shadow: None,
        }
    }
}

struct WasmFunc {
    name: String,
    signature: FuncTypeSignature,
    params: Vec<Param>,
    locals: LocalData,
    bytes: Vec<u8>,
    export: bool,
}

impl WasmFunc {
    fn new(name: String, signature: FuncTypeSignature, export: bool) -> Self {
        Self {
            name,
            signature,
            params: vec![],
            locals: Default::default(),
            bytes: vec![],
            export,
        }
    }
    fn n_params(&self) -> u32 {
        self.signature.args.len() as u32
    }
    fn get_param_idx(&self, name: &str) -> Option<u32> {
        self.params
            .iter()
            .position(|x| x.name == name)
            .map(|x| x as u32)
    }
    fn get_local_idx(&self, name: &str) -> Option<u32> {
        self.locals.get_idx(name).map(|x| x + self.n_params())
    }

    fn get_idx_and_global_shadow(&self, name: &str) -> Option<(u32, Option<u32>)> {
        if let Some(local_idx) = self.get_local_idx(name) {
            return Some((
                local_idx,
                self.locals.locals[local_idx as usize].global_shadow,
            ));
        }
        if let Some(param_idx) = self.get_param_idx(name) {
            return Some((param_idx, self.params[param_idx as usize].global_shadow));
        }
        None
    }
    fn add_global_shadow(&mut self, name: &str, global_idx: u32) {
        for local in self.locals.locals.iter_mut().rev() {
            if local.name == name {
                local.global_shadow = Some(global_idx);
                return;
            }
        }
        for param in self.params.iter_mut().rev() {
            if param.name == name {
                param.global_shadow = Some(global_idx);
                return;
            }
        }
    }
}

pub struct Wasmizer {
    pub typecontext: TypeContext,
    pub global_vars: env::GlobalVars,
    builder: ModuleBuilder,
    frames: Vec<WasmFunc>,
    builtins: FxHashMap<String, u32>, // name -> func index
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
    fn get_frame_mut(&mut self, depth: usize) -> Option<&mut WasmFunc> {
        if depth >= self.frames.len() {
            return None;
        }
        let idx = self.frames.len() - depth - 1;
        self.frames.get_mut(idx)
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
            None => return Err(format!("Unknown builtin function: {}", name)),
        };
        self.write_opcode(Opcode::Call);
        self.bytes_mut().append(&mut unsigned_leb128(idx));
        Ok(())
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
        self.write_byte(0x02); // alignment
        self.write_byte(0x00); // load offset

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
        self.write_byte(0x00); // break depth
        self.write_opcode(Opcode::End);

        // print -8888 twice to show that we're done
        self.write_opcode(Opcode::I32Const);
        self.write_slice(&signed_leb128(-8888));
        self.call_builtin("print[Int]").unwrap();
        self.call_builtin("print[Int]").unwrap();
        self.write_opcode(Opcode::Drop);
    }

    pub fn init_func(
        &mut self,
        name: String,
        argtypes: &[ast::Type],
        rettype: &ast::Type,
        export: bool,
    ) -> Result<(), String> {
        let args = argtypes
            .iter()
            .map(|t| Numtype::from_ast_type(t))
            .collect::<Result<Vec<_>, _>>()?;
        let ret = Numtype::from_ast_type(rettype)?;
        let func = WasmFunc::new(name, FuncTypeSignature::new(args, Some(ret)), export);
        self.frames.push(func);

        Ok(())
    }
    pub fn finish_func(&mut self) -> Result<u32, String> {
        #[cfg(feature = "debug")]
        {
            // Print contents of memory
            self.print_mem();
            // Print memptr
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
        } else {
            None
        };

        // pop from frames and add to ModuleBuilder
        let func = match self.frames.pop() {
            Some(func) => func,
            None => return Err("Tried to pop function when frames is empty".to_string()),
        };
        self.builder
            .add_function(&func.signature, func.locals.types, func.bytes, export_name)
    }
    pub fn write_last_func_index(&mut self) {
        let idx = self.builder.funcs.len() as i32 - 1;
        self.write_opcode(Opcode::I32Const);
        self.bytes_mut().append(&mut signed_leb128(idx));
    }

    pub fn write_const(&mut self, value: &str, typ: &ast::Type) -> Result<(), String> {
        match typ {
            ast::Type::Int => {
                let value = value.parse::<i32>().map_err(|e| e.to_string())?;
                self.write_opcode(Opcode::I32Const);
                self.bytes_mut().append(&mut signed_leb128(value));
            }
            ast::Type::Float => {
                let value = value.parse::<f32>().map_err(|e| e.to_string())?;
                self.write_opcode(Opcode::F32Const);
                self.write_slice(&value.to_le_bytes());
            }
            ast::Type::Bool => {
                let value = value.parse::<bool>().map_err(|e| e.to_string())?;
                self.write_opcode(Opcode::I32Const);
                self.write_byte(if value { 1 } else { 0 });
            }
            ast::Type::Str => self.write_string(value)?,
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
                self.write_byte(0x7f); // -1
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

        let collect_fn_idx = unsigned_leb128(self.init_collect(numtype)?);
        self.write_opcode(Opcode::Call);
        self.write_slice(&collect_fn_idx);

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
            }
            ast::Type::Float => {
                self.write_opcode(Opcode::F32Add);
            }
            ast::Type::Str | ast::Type::Arr(_) => {
                self.concat_heap_objects()?;
            }
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
            }
            ast::Type::Float => {
                self.write_opcode(Opcode::F32Sub);
            }
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
            }
            ast::Type::Float => {
                self.write_opcode(Opcode::F32Mul);
            }
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
            }
            ast::Type::Float => {
                self.write_opcode(Opcode::F32Div);
            }
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
            }
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
            }
            _ => {
                return Err(format!("Cannot OR values of type {:?}", typ));
            }
        }
        Ok(())
    }

    pub fn write_range(&mut self, typ: &ast::Type) -> Result<(), String> {
        if typ != &ast::Type::Int {
            return Err(format!(
                "Cannot create range of values of type {:?}. Ranges must be between integers.",
                typ
            ));
        }

        let factory = unsigned_leb128(self.get_range_iter_factory()?);
        self.write_opcode(Opcode::Call);
        self.write_slice(&factory);

        Ok(())
    }

    pub fn write_len(&mut self, typ: &ast::Type) -> Result<(), String> {
        match typ {
            ast::Type::Arr(inner_type) => {
                let inner_type = Numtype::from_ast_type(inner_type)?;
                // extract size from fatptr
                self.write_opcode(Opcode::I32WrapI64); // discard high 32 bits
                                                       // divide by size of inner type
                self.write_opcode(Opcode::I32Const);
                self.write_slice(&unsigned_leb128(inner_type.size()));
                self.write_opcode(Opcode::I32DivU);
                Ok(())
            }
            ast::Type::Iter(inner_type) => {
                let inner_type = Numtype::from_ast_type(inner_type)?;
                let iter_len_fn_idx = self.init_iter_len(inner_type)?;
                self.write_opcode(Opcode::Call);
                self.write_slice(&unsigned_leb128(iter_len_fn_idx));
                Ok(())
            }
            ast::Type::Str => {
                let str_len_fn_idx = self.init_str_len()?;
                self.write_opcode(Opcode::Call);
                self.write_slice(&unsigned_leb128(str_len_fn_idx));
                Ok(())
            }
            _ => Err(format!("Cannot get length of type {:?}", typ)),
        }
    }

    fn init_iter_len(&mut self, numtype: Numtype) -> Result<u32, String> {
        let memsize = numtype.size();
        let function_name = format!("iter_len_{}", memsize);
        if let Some(idx) = self.builtins.get(&function_name) {
            return Ok(*idx);
        }

        let advance_fn_type_idx = self.get_advance_fn_type_idx();

        let func = builtin_funcs::define_builtin_iter_len(memsize, advance_fn_type_idx);

        let fn_idx = self.builder.add_builtin(&func)?;
        self.builtins.insert(function_name, fn_idx);

        Ok(fn_idx)
    }

    fn init_str_len(&mut self) -> Result<u32, String> {
        if let Some(idx) = self.builtins.get("str_len") {
            return Ok(*idx);
        }

        let func = builtin_funcs::define_builtin_str_len();

        let func_idx = self.builder.add_builtin(&func)?;
        self.builtins.insert("str_len".to_string(), func_idx);

        Ok(func_idx)
    }

    pub fn write_some(&mut self, typ: &ast::Type) -> Result<(), String> {
        let numtype = Numtype::from_ast_type(typ)?;
        let constructor_idx = self.init_maybe(numtype)?;

        // Constructor requires 2 arguments: value and is_some. Value is already on stack.
        self.write_opcode(Opcode::I32Const);
        self.write_byte(1); // yes, this is some

        // Call the constructor
        self.write_opcode(Opcode::Call);
        self.write_slice(&unsigned_leb128(constructor_idx));

        Ok(())
    }

    pub fn write_null(&mut self, typ: &ast::Type) -> Result<(), String> {
        let numtype = Numtype::from_ast_type(typ)?;
        let constructor_idx = self.init_maybe(numtype)?;

        // Constructor requies 2 arguments: value and is_some. We can just pass 0 for the value.
        self.write_opcode(numtype.const_op());
        match numtype {
            Numtype::F32 => self.write_slice(&[0, 0, 0, 0]),
            _ => self.write_byte(0),
        };
        self.write_opcode(Opcode::I32Const);
        self.write_byte(0); // no, this is null

        // Call the constructor
        self.write_opcode(Opcode::Call);
        self.write_slice(&unsigned_leb128(constructor_idx));

        Ok(())
    }

    fn init_maybe(&mut self, numtype: Numtype) -> Result<u32, String> {
        let struct_name = format!("<Maybe[{}]>", numtype);

        // check if the struct has already been defined
        if let Some(idx) = self.builtins.get(&struct_name) {
            return Ok(*idx);
        }

        // define the struct
        let struct_def = Struct::new(vec![
            ("value".to_string(), numtype),
            ("is_some".to_string(), Numtype::I32),
        ]);

        self.create_struct(struct_name, struct_def, false)
    }

    pub fn write_unwrap(&mut self, typ: &ast::Type) -> Result<(), String> {
        let numtype = Numtype::from_ast_type(typ)?;
        let unwrap_fn_idx = self.init_unwrap(numtype)?;

        self.write_opcode(Opcode::Call);
        self.write_slice(&unsigned_leb128(unwrap_fn_idx));

        Ok(())
    }

    pub fn write_is_some(&mut self, typ: &ast::Type) -> Result<(), String> {
        let numtype = Numtype::from_ast_type(typ)?;
        let is_some_fn_idx = self.init_is_some(numtype)?;

        self.write_opcode(Opcode::Call);
        self.write_slice(&unsigned_leb128(is_some_fn_idx));

        Ok(())
    }

    fn init_unwrap(&mut self, numtype: Numtype) -> Result<u32, String> {
        let fn_name = format!("unwrap_{}", numtype);

        // check if the function has already been defined
        if let Some(idx) = self.builtins.get(&fn_name) {
            return Ok(*idx);
        }

        // define the function
        let func = builtin_funcs::define_builtin_unwrap(numtype);

        let fn_idx = self.builder.add_builtin(&func)?;
        self.builtins.insert(fn_name, fn_idx);

        Ok(fn_idx)
    }

    fn init_is_some(&mut self, numtype: Numtype) -> Result<u32, String> {
        let fn_name = format!("is_some_{}", numtype);

        // check if the function has already been defined
        if let Some(idx) = self.builtins.get(&fn_name) {
            return Ok(*idx);
        }

        // define the function
        let func = builtin_funcs::define_builtin_is_some(numtype);

        let fn_idx = self.builder.add_builtin(&func)?;
        self.builtins.insert(fn_name, fn_idx);

        Ok(fn_idx)
    }

    // convert an array on the stack into an array iterator
    pub fn make_array_iter(&mut self, inner_type: &ast::Type) -> Result<(), String> {
        let numtype = Numtype::from_ast_type(inner_type)?;
        let factory = unsigned_leb128(self.get_array_iter_factory(numtype)?);
        self.write_opcode(Opcode::Call);
        self.write_slice(&factory);

        Ok(())
    }

    pub fn write_map(
        &mut self,
        result_inner_type: &ast::Type,
        input_inner_type: &ast::Type,
        input_is_array: bool,
    ) -> Result<(), String> {
        if input_is_array {
            self.make_array_iter(input_inner_type)?;
        }

        let result_inner_type = Numtype::from_ast_type(result_inner_type)?;
        let input_inner_type = Numtype::from_ast_type(input_inner_type)?;

        let factory =
            unsigned_leb128(self.get_map_iter_factory(input_inner_type, result_inner_type)?);
        self.write_opcode(Opcode::Call);
        self.write_slice(&factory);

        Ok(())
    }

    pub fn write_reduce(
        &mut self,
        acc_type: &ast::Type,
        x_type: &ast::Type,
        use_array_iter: bool,
    ) -> Result<(), String> {
        if use_array_iter {
            // convert the Array into an ArrayIter first
            self.make_array_iter(x_type)?;
        }

        let acc_type = Numtype::from_ast_type(acc_type)?;
        let x_type = Numtype::from_ast_type(x_type)?;

        // we implement this by creating a scan iterator, then getting the last element of that iterator
        let factory = unsigned_leb128(self.get_scan_iter_factory(acc_type, x_type)?);
        self.write_opcode(Opcode::Call);
        self.write_slice(&factory);

        let last_fn_idx = unsigned_leb128(self.init_last(acc_type)?);
        self.write_opcode(Opcode::Call);
        self.write_slice(&last_fn_idx);

        Ok(())
    }

    pub fn write_filter(&mut self, typ: &ast::Type, use_array_iter: bool) -> Result<(), String> {
        if use_array_iter {
            // convert the Array into an ArrayIter first
            self.make_array_iter(typ)?;
        }

        let numtype = Numtype::from_ast_type(typ)?;
        let factory = unsigned_leb128(self.get_filter_iter_factory(numtype)?);
        self.write_opcode(Opcode::Call);
        self.write_slice(&factory);

        Ok(())
    }

    pub fn write_zipmap(
        &mut self,
        func_ret_type: &ast::Type,
        iter_over_types: &[ast::Type],
    ) -> Result<(), String> {
        let func_ret_type = Numtype::from_ast_type(func_ret_type)?;
        let iter_over_types = iter_over_types
            .iter()
            .map(Numtype::from_ast_type)
            .collect::<Result<Vec<_>, _>>()?;
        let factory =
            unsigned_leb128(self.get_zipmap_iter_factory(func_ret_type, &iter_over_types)?);
        self.write_opcode(Opcode::Call);
        self.write_slice(&factory);
        Ok(())
    }

    fn add_local(&mut self, name: &str, numtype: Numtype) -> Vec<u8> {
        let i = self.locals_mut().add_local(name.to_string(), numtype as u8)
            + self.current_func().n_params();
        unsigned_leb128(i)
    }

    // concatenate two heap objects (e.g. arrays, strings)
    // the fatptrs indices to the objects should be the two last things on the stack when calling this
    fn concat_heap_objects(&mut self) -> Result<(), String> {
        self.call_builtin("concat_heap_objs")?;
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
        self.bytes_mut()
            .append(&mut unsigned_leb128(len as u32 * memsize));
        self.call_builtin("alloc")?;
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
        for _ in 0..len {
            // // If values are heap types, they need to be copied here
            // if numtype == Numtype::I64 {
            //     self.call_builtin("copy_heap_obj")?;
            // }
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
        self.bytes_mut()
            .append(&mut unsigned_leb128(len as u32 * memsize));
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
        self.write_slice(&offset_idx); // we'll use this later
                                       // offset in source is 0
        self.write_opcode(Opcode::I32Const);
        self.write_byte(0x00);
        // size of memory region is data.len()
        self.write_opcode(Opcode::I32Const);
        self.write_slice(&size);
        // write memory.init
        self.write_slice(&MEMINIT);
        self.write_slice(&unsigned_leb128(segment_idx));
        self.write_byte(0x00); // idx of memory (always zero)

        // update memptr
        self.write_opcode(Opcode::GlobalGet);
        self.write_byte(0x00);
        self.write_opcode(Opcode::I32Const);
        self.write_slice(&size);
        self.write_opcode(Opcode::I32Add);
        self.write_opcode(Opcode::GlobalSet);
        self.write_byte(0x00);

        self.align_memptr();

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

    fn write_to_memory(
        &mut self,
        memptr_idx: &[u8],
        value_idx: &[u8],
        store_op: Opcode,
        memsize: u8,
    ) -> Result<(), String> {
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
        self.write_byte(0x02); // alignment
        self.write_byte(0x00); // offset

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

    pub fn write_drop(&mut self) {
        self.write_opcode(Opcode::Drop);
    }

    pub fn begin_scope(&mut self, typ: &ast::Type) -> Result<(), String> {
        self.locals_mut().scope_depth += 1;
        if self.locals().scope_depth > 0 {
            self.write_opcode(Opcode::Block);
            self.write_byte(Numtype::from_ast_type(typ)? as u8)
        }
        Ok(())
    }
    pub fn end_scope(&mut self) -> Result<(), String> {
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
        self.current_func_mut().params.push(Param::new(name));
    }
    pub fn create_variable(&mut self, name: String, typ: &ast::Type) -> Result<u32, String> {
        // only handle local variables for now
        // create a local variable
        let typ = Numtype::from_ast_type(typ)? as u8;
        Ok(self.locals_mut().add_local(name, typ) + self.current_func().n_params())
    }
    pub fn set_variable(&mut self, idx: u32, _typ: &ast::Type) -> Result<(), String> {
        // // if typ is a heap-allocated type, we also need to copy the memory
        // if Numtype::from_ast_type(typ)? == Numtype::I64 {
        //     self.call_builtin("copy_heap_obj")?;
        // }
        self.write_opcode(Opcode::LocalTee);
        self.bytes_mut().append(&mut unsigned_leb128(idx));

        Ok(())
    }
    pub fn get_variable(&mut self, name: String, typ: &ast::Type) -> Result<i32, String> {
        // TODO: can't yet deal with recursive functions
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
        // next, look in upvalues
        if let Some(idx) = self.resolve_upvalue(&name, Numtype::from_ast_type(typ)?, 1)? {
            self.write_opcode(Opcode::GlobalGet);
            self.write_slice(&unsigned_leb128(idx));
            return Ok(0);
        }
        // look in global scope
        let maybe_value = {
            let globals = self.global_vars.borrow();
            globals.get(&name).cloned()
        };
        if let Some(value) = maybe_value {
            self.write_opcode(Opcode::I32Const);
            self.write_slice(&unsigned_leb128(value as u32));
            return Ok(1); // 1 denotes found variable is global
        }
        // finally, see if this is one of the dynamically defined special functions
        let idx = self.get_callable_builtin(&name)?;
        self.write_opcode(Opcode::I32Const);
        self.write_slice(&unsigned_leb128(idx));
        Ok(1)
    }

    fn resolve_upvalue(
        &mut self,
        name: &str,
        numtype: Numtype,
        depth: usize,
    ) -> Result<Option<u32>, String> {
        let local_info = {
            let parent = self.get_frame_mut(depth);
            let parent = match parent {
                Some(parent) => parent,
                None => return Ok(None),
            };
            parent.get_idx_and_global_shadow(name)
        };
        if let Some((idx, global_shadow)) = local_info {
            let global_idx = match global_shadow {
                // Use pre-existing global variable
                Some(global_shadow) => global_shadow,
                // Create a new global variable to store the upvalue
                None => {
                    let global_idx = self.builder.add_global(Global::new(numtype, true, 0));
                    // Within the function where the value is defined, set the global variable to the appropriate value
                    let parent = self.get_frame_mut(depth).unwrap();
                    parent.bytes.push(Opcode::LocalGet as u8);
                    parent.bytes.append(&mut unsigned_leb128(idx));
                    parent.bytes.push(Opcode::GlobalSet as u8);
                    parent.bytes.append(&mut unsigned_leb128(global_idx));
                    // Record the index of the shadowing global variable
                    parent.add_global_shadow(name, global_idx);

                    global_idx
                }
            };
            return Ok(Some(global_idx));
        }
        // try looking for the value one more frame up
        self.resolve_upvalue(name, numtype, depth + 1)
    }

    fn get_callable_builtin(&mut self, name: &str) -> Result<u32, String> {
        // check if function is already defined as a builtin
        let special_name = format!("<callable>{}", name);
        if let Some(idx) = self.builtins.get(&special_name) {
            return Ok(*idx);
        }
        // see if there is a definition available
        let func = match name {
            "abs[Int]" => builtin_funcs::define_builtin_abs_int(),
            "abs[Float]" => builtin_funcs::define_builtin_abs_float(),
            "float[Int]" => builtin_funcs::define_builtin_itof(),
            "int[Float]" => builtin_funcs::define_builtin_ftoi(),
            "sqrt[Float]" => builtin_funcs::define_builtin_sqrt_float(),
            "mod[Int, Int]" => builtin_funcs::define_builtin_mod(),
            "sum[Iter(Int)]" => builtin_funcs::define_builtin_reduce_iter(
                Numtype::I32,
                "sum",
                self.get_advance_fn_type_idx(),
            ),
            "sum[Iter(Float)]" => builtin_funcs::define_builtin_reduce_iter(
                Numtype::F32,
                "sum",
                self.get_advance_fn_type_idx(),
            ),
            "prod[Iter(Int)]" => builtin_funcs::define_builtin_reduce_iter(
                Numtype::I32,
                "prod",
                self.get_advance_fn_type_idx(),
            ),
            "prod[Iter(Float)]" => builtin_funcs::define_builtin_reduce_iter(
                Numtype::F32,
                "prod",
                self.get_advance_fn_type_idx(),
            ),
            "all[Iter(Bool)]" => builtin_funcs::define_builtin_reduce_iter(
                Numtype::I32,
                "all",
                self.get_advance_fn_type_idx(),
            ),
            "any[Iter(Bool)]" => builtin_funcs::define_builtin_reduce_iter(
                Numtype::I32,
                "any",
                self.get_advance_fn_type_idx(),
            ),
            _ => return Err(format!("variable {} not found", name)),
        };
        let fn_idx = self.builder.add_builtin(&func)?;
        self.builtins.insert(special_name, fn_idx);

        Ok(fn_idx)
    }

    pub fn call_indirect(&mut self, typ: &ast::Type) -> Result<(), String> {
        let idx = self
            .builder
            .get_functype_idx(&FuncTypeSignature::from_ast_type(typ)?);
        self.write_opcode(Opcode::CallIndirect);
        self.write_slice(&unsigned_leb128(idx)); // signature index
        self.write_byte(0x00); // table index
        Ok(())
    }
    pub fn call(&mut self) -> Result<(), String> {
        let fn_idx = self.bytes_mut().pop().expect(
            "No values left in bytes when attempting to pop Value for direct function call",
        );
        self.bytes_mut()
            .pop()
            .expect("No values left in bytes when attempting to pop I32Const Opcode");
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
        // TODO: Prevent out-of-bounds access
        match numtype {
            Numtype::I32 => self.call_builtin("get_i32_field"),
            Numtype::I64 => self.call_builtin("get_i64_field"),
            Numtype::F32 => self.call_builtin("get_f32_field"),
            _ => unreachable!(),
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
    // returns the index of the struct's constructor function
    pub fn create_struct(
        &mut self,
        struct_name: String,
        struct_def: Struct,
        write_constructor: bool,
    ) -> Result<u32, String> {
        let func = builtin_funcs::define_builtin_struct_constructor(
            &struct_def,
            *self.builtins.get("alloc").unwrap(),
        );

        // add to list of builtin functions
        let idx = self.builder.add_builtin(&func)?;
        self.builtins.insert(struct_name.clone(), idx);

        // add to definitions
        self.structs.insert(struct_name, struct_def);

        if write_constructor {
            // add constructor to stack
            self.write_last_func_index();
        }
        Ok(idx)
    }

    // get the field of a struct
    // fatptr to struct should be on top of stack when calling this
    pub fn get_field(&mut self, object_type: ast::Type, field_name: &str) -> Result<(), String> {
        // first, figure out which struct type this is for
        let (struct_name, _) = match object_type {
            ast::Type::Object(name, fields) => (name, fields),
            _ => unreachable!(),
        };
        let struct_def = self.structs.get(&struct_name).unwrap();
        let field = struct_def
            .fields
            .iter()
            .find(|(name, _)| name == field_name)
            .unwrap()
            .1
            .clone();

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

    fn get_advance_fn_type_idx(&mut self) -> u32 {
        self.builder.get_functype_idx(&FuncTypeSignature::new(
            vec![Numtype::I32],
            Some(Numtype::I32),
        ))
    }

    // create a struct type that is used to store ranges
    fn get_range_iter_factory(&mut self) -> Result<u32, String> {
        if let Some(idx) = self.builtins.get("<RangeIterFactory>") {
            return Ok(*idx);
        }

        // add the struct definition and constructor
        let struct_def = Struct::new(vec![
            ("current".to_string(), Numtype::I32),
            ("advance_fn".to_string(), Numtype::I32),
            ("step".to_string(), Numtype::I32),
            ("stop".to_string(), Numtype::I32),
        ]);

        let constructor_idx = self.create_struct("<RangeIter>".to_string(), struct_def, false)?;

        // initialize the "advance" function used by this Iter type
        let func = builtin_funcs::define_builtin_range_iter_advance();

        let advance_fn_idx = self.builder.add_builtin(&func)?;
        self.builtins
            .insert("<RangeIterAdvance>".to_string(), advance_fn_idx);

        // create helper function for building range iterators from `<start> to <stop>` syntax

        let advance_fn_table_idx = advance_fn_idx - self.builder.imports.len() as u32;
        let func =
            builtin_funcs::define_builtin_range_iter_factory(constructor_idx, advance_fn_table_idx);

        let factory_idx = self.builder.add_builtin(&func)?;
        self.builtins
            .insert("<RangeIterFactory>".to_string(), factory_idx);

        Ok(factory_idx)
    }

    // create a struct type that is used to store map iterators
    fn get_map_iter_factory(&mut self, in_type: Numtype, out_type: Numtype) -> Result<u32, String> {
        let factory_name = format!("<MapIter[{}->{}]Factory>", in_type, out_type);
        if let Some(idx) = self.builtins.get(&factory_name) {
            // no need to do anything else if there's already a factory for this type mapping
            return Ok(*idx);
        }

        // add the struct definition and constructor
        let struct_def = Struct::new(vec![
            ("current".to_string(), out_type),
            ("advance_fn".to_string(), Numtype::I32),
            ("map_fn".to_string(), Numtype::I32), // the table index of the map function
            ("inner_offset".to_string(), Numtype::I32), // the memory offset of the iterator being mapped from
        ]);

        // let advance_delta = unsigned_leb128(struct_def.get_field("advance_fn").unwrap().offset);  // not needed
        let map_fn_delta = struct_def.get_field("map_fn").unwrap().offset;
        let inner_offset_delta = struct_def.get_field("inner_offset").unwrap().offset;

        let struct_name = format!("<MapIter[{}->{}]>", in_type, out_type);
        let constructor_idx = self.create_struct(struct_name.clone(), struct_def, false)?;

        // initialize the "advance" function used by this Iter type
        let advance_fn_type_idx = self.get_advance_fn_type_idx();
        let map_fn_type_idx = self
            .builder
            .get_functype_idx(&FuncTypeSignature::new(vec![in_type], Some(out_type)));
        let func = builtin_funcs::define_builtin_map_iter_advance(
            in_type,
            out_type,
            inner_offset_delta,
            map_fn_delta,
            advance_fn_type_idx,
            map_fn_type_idx,
        );

        let advance_fn_idx = self.builder.add_builtin(&func)?;
        self.builtins.insert(
            format!("<MapIter[{}->{}]Advance>", in_type, out_type),
            advance_fn_idx,
        );

        // create helper function for building map iterators from `<fn> -> <iter>` syntax
        let func = builtin_funcs::define_builtin_map_iter_factory(
            out_type,
            constructor_idx,
            advance_fn_idx - self.builder.imports.len() as u32,
        );

        let factory_idx = self.builder.add_builtin(&func)?;
        self.builtins.insert(factory_name, factory_idx);

        Ok(factory_idx)
    }

    fn get_scan_iter_factory(&mut self, acc_type: Numtype, x_type: Numtype) -> Result<u32, String> {
        let factory_name = format!("<ScanIter[{},{}]Factory>", acc_type, x_type);
        if let Some(idx) = self.builtins.get(&factory_name) {
            return Ok(*idx);
        }

        let struct_def = Struct::new(vec![
            ("current".to_string(), acc_type),
            ("advance_fn".to_string(), Numtype::I32),
            ("reduce_fn".to_string(), Numtype::I32), // the table index of the reduce function
            ("inner_offset".to_string(), Numtype::I32), // the memory offset of the iterator being reduced
        ]);

        let reduce_fn_delta = struct_def.get_field("reduce_fn").unwrap().offset;
        let inner_offset_delta = struct_def.get_field("inner_offset").unwrap().offset;

        let struct_name = format!("<ScanIter[{},{}]>", acc_type, x_type);
        let constructor_idx = self.create_struct(struct_name.clone(), struct_def, false)?;

        // initialize advance fn
        let advance_fn_type_idx = self.get_advance_fn_type_idx();
        let reduce_fn_type_idx = self.builder.get_functype_idx(&FuncTypeSignature::new(
            vec![acc_type, x_type],
            Some(acc_type),
        ));
        let func = builtin_funcs::define_builtin_scan_iter_advance(
            acc_type,
            x_type,
            inner_offset_delta,
            reduce_fn_delta,
            advance_fn_type_idx,
            reduce_fn_type_idx,
        );

        let advance_fn_idx = self.builder.add_builtin(&func)?;
        self.builtins.insert(
            format!("<ScanIter[{},{}]Advance>", acc_type, x_type),
            advance_fn_idx,
        );

        // create factory function (for creating from function, init, iter_over syntax)
        let func = builtin_funcs::define_builtin_scan_iter_factory(
            acc_type,
            advance_fn_idx - self.builder.imports.len() as u32,
            constructor_idx,
        );

        let factory_idx = self.builder.add_builtin(&func)?;
        self.builtins.insert(factory_name, factory_idx);

        Ok(factory_idx)
    }

    fn get_filter_iter_factory(&mut self, inner_type: Numtype) -> Result<u32, String> {
        let factory_name = format!("<FilterIter[{}]Factory>", inner_type);
        if let Some(idx) = self.builtins.get(&factory_name) {
            return Ok(*idx);
        }

        let struct_def = Struct::new(vec![
            ("current".to_string(), inner_type),
            ("advance_fn".to_string(), Numtype::I32),
            ("filter_fn".to_string(), Numtype::I32), // the table index of the filter function
            ("inner_offset".to_string(), Numtype::I32), // the memory offset of the iterator being filtered
        ]);

        let filter_fn_delta = struct_def.get_field("filter_fn").unwrap().offset;
        let inner_offset_delta = struct_def.get_field("inner_offset").unwrap().offset;

        let struct_name = format!("<FilterIter[{}]>", inner_type);
        let constructor_idx = self.create_struct(struct_name.clone(), struct_def, false)?;

        // initialize advance fn
        let advance_fn_type_idx = self.get_advance_fn_type_idx();
        let filter_fn_type_idx = self.builder.get_functype_idx(&FuncTypeSignature::new(
            vec![inner_type],
            Some(Numtype::I32),
        ));
        let func = builtin_funcs::define_builtin_filter_iter_advance(
            inner_type,
            inner_offset_delta,
            filter_fn_delta,
            advance_fn_type_idx,
            filter_fn_type_idx,
        );

        let advance_fn_idx = self.builder.add_builtin(&func)?;
        self.builtins.insert(
            format!("<FilterIter[{}]Advance>", inner_type),
            advance_fn_idx,
        );

        // create factory function (for creating from function, iter_over syntax)
        let func = builtin_funcs::define_builtin_filter_iter_factory(
            inner_type,
            advance_fn_idx - self.builder.imports.len() as u32,
            constructor_idx,
        );

        let factory_idx = self.builder.add_builtin(&func)?;
        self.builtins.insert(factory_name, factory_idx);

        Ok(factory_idx)
    }

    fn get_zipmap_iter_factory(
        &mut self,
        out_type: Numtype,
        iter_over_types: &[Numtype],
    ) -> Result<u32, String> {
        let factory_name = format!("<ZipMapIter[{:?}->{}]Factory>", iter_over_types, out_type);
        if let Some(idx) = self.builtins.get(&factory_name) {
            return Ok(*idx);
        }

        let mut struct_fields = vec![
            ("current".to_string(), out_type),
            ("advance_fn".to_string(), Numtype::I32),
            ("map_fn".to_string(), Numtype::I32), // the table index of the map function
        ];
        // add a field for each inner iterator
        for i in 0..iter_over_types.len() {
            struct_fields.push((format!("inner_{}_offset", i), Numtype::I32));
        }
        let struct_def = Struct::new(struct_fields);

        let inner_offset_deltas = (0..iter_over_types.len())
            .map(|i| {
                struct_def
                    .get_field(&format!("inner_{}_offset", i))
                    .unwrap()
                    .offset
            })
            .collect::<Vec<_>>();
        let map_fn_delta = struct_def.get_field("map_fn").unwrap().offset;

        let struct_name = format!("<ZipMapIter[{:?}->{}]>", iter_over_types, out_type);
        let constructor_idx = self.create_struct(struct_name.clone(), struct_def, false)?;

        // initialize advance fn
        let advance_fn_type_idx = self.get_advance_fn_type_idx();
        let map_fn_type_idx = self.builder.get_functype_idx(&FuncTypeSignature::new(
            iter_over_types.to_vec(),
            Some(out_type),
        ));
        let func = builtin_funcs::define_builtin_zipmap_iter_advance(
            iter_over_types,
            out_type,
            &inner_offset_deltas,
            map_fn_delta,
            advance_fn_type_idx,
            map_fn_type_idx,
        );

        let advance_fn_idx = self.builder.add_builtin(&func)?;
        self.builtins.insert(
            format!("<ZipMapIter[{:?}->{}]Advance>", iter_over_types, out_type),
            advance_fn_idx,
        );

        // create helper function for building map iterators from `<fn>, [<iter0>, <iter1>, ...]` syntax
        let func = builtin_funcs::define_builtin_zipmap_iter_factory(
            &iter_over_types,
            out_type,
            advance_fn_idx - self.builder.imports.len() as u32,
            constructor_idx,
        );

        let factory_idx = self.builder.add_builtin(&func)?;
        self.builtins.insert(factory_name, factory_idx);

        Ok(factory_idx)
    }

    fn get_array_iter_factory(&mut self, numtype: Numtype) -> Result<u32, String> {
        let factory_name = format!("<ArrIter[{}]Factory>", numtype);
        if let Some(idx) = self.builtins.get(&factory_name) {
            // no need to add it if we already have it
            return Ok(*idx);
        }

        let struct_def = Struct::new(vec![
            ("current".to_string(), numtype),
            ("advance_fn".to_string(), Numtype::I32),
            ("inner_offset".to_string(), Numtype::I32), // current offset in inner array
            ("max_inner_offset".to_string(), Numtype::I32), // max offset in inner array -- when this is reached, we are done
        ]);

        // save these for later
        let inner_offset_delta = struct_def.get_field("inner_offset").unwrap().offset;
        let max_inner_offset_delta = struct_def.get_field("max_inner_offset").unwrap().offset;

        let struct_name = format!("ArrIter[{}]", numtype);
        let constructor_idx = self.create_struct(struct_name.clone(), struct_def, false)?;

        // initialize "advance" function
        let func = builtin_funcs::define_builtin_array_iter_advance(
            numtype,
            inner_offset_delta,
            max_inner_offset_delta,
        );

        let advance_fn_idx = self.builder.add_builtin(&func)?;
        self.builtins
            .insert(format!("<ArrIter[{}]Advance>", numtype), advance_fn_idx);

        // create helper function for creating iter from fatptr to array
        let func = builtin_funcs::define_builtin_array_iter_factory(
            numtype,
            advance_fn_idx - self.builder.imports.len() as u32,
            constructor_idx,
        );

        let factory_idx = self.builder.add_builtin(&func)?;
        self.builtins.insert(factory_name, factory_idx);

        Ok(factory_idx)
    }

    // create the `collect_[type]` builtin (used for the @ operator on Iter([type]))
    fn init_collect(&mut self, numtype: Numtype) -> Result<u32, String> {
        // check if already initialized, return index if so
        let fn_name = format!("collect_{}", numtype);
        if let Some(idx) = self.builtins.get(&fn_name) {
            return Ok(*idx);
        }

        let alloc_idx = *self.builtins.get("alloc").unwrap();
        let advance_fn_type_idx = self.get_advance_fn_type_idx();
        let func =
            builtin_funcs::define_builtin_iter_collect(numtype, alloc_idx, advance_fn_type_idx);

        let fn_idx = self.builder.add_builtin(&func)?;
        self.builtins.insert(fn_name, fn_idx);

        Ok(fn_idx)
    }

    // initialize the last_[type] builtin, used to get the last element in an iterator
    fn init_last(&mut self, iter_type: Numtype) -> Result<u32, String> {
        // check if already initialized, return index if so
        let fn_name = format!("last_{}", iter_type);
        if let Some(idx) = self.builtins.get(&fn_name) {
            return Ok(*idx);
        }

        let advance_fn_type_idx = self.get_advance_fn_type_idx();
        let func = builtin_funcs::define_builtin_iter_last(iter_type, advance_fn_type_idx);

        let fn_idx = self.builder.add_builtin(&func)?;
        self.builtins.insert(fn_name, fn_idx);

        Ok(fn_idx)
    }

    fn to_bytes(&self) -> Vec<u8> {
        self.builder.program()
    }
}

pub fn wasmize(source: &str, global_env: env::Env) -> Result<(Vec<u8>, ast::Type), String> {
    let tokens = scanner::scan(source);
    let ast = parser::parse(tokens, global_env.global_types.clone())
        .map_err(|_| "Compilation halted due to parsing error.")?;
    #[cfg(feature = "debug")]
    println!("{:?}", ast);
    let mut wasmizer = Wasmizer::new(global_env)?;
    ast.wasmize(&mut wasmizer)?;
    let return_type = ast.get_type()?;

    let bytes = wasmizer.to_bytes();
    #[cfg(feature = "debug")]
    {
        // print out bytes in form similar to x                                                                                                                          xd -g 1
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
