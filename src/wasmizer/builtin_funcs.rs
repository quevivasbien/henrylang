use lazy_static::lazy_static;
use rustc_hash::FxHashMap;

use super::{structs::Struct, wasmtypes::*};

struct LocalVar {
    name: String,
    numtype: Numtype,
}

pub struct BuiltinFunc {
    signature: FuncTypeSignature,
    params: Vec<LocalVar>,
    locals: Vec<LocalVar>,

    bytes: Vec<u8>,
}

impl BuiltinFunc {
    pub fn new(signature: FuncTypeSignature, param_names: Vec<String>) -> Self {
        assert_eq!(param_names.len(), signature.args.len());
        let params = param_names
            .into_iter()
            .zip(signature.args.iter())
            .map(|(name, &numtype)| LocalVar { name, numtype })
            .collect();
        Self {
            signature,
            params,
            locals: vec![],
            bytes: vec![],
        }
    }

    pub fn get_var_idx(&self, name: &str) -> Option<Vec<u8>> {
        for (i, param) in self.params.iter().enumerate() {
            if param.name == name {
                return Some(unsigned_leb128(i as u32));
            }
        }
        for (i, local) in self.locals.iter().enumerate() {
            if local.name == name {
                return Some(unsigned_leb128((i + self.params.len()) as u32));
            }
        }
        None
    }

    pub fn add_local(&mut self, name: &str, numtype: Numtype) {
        assert_eq!(self.get_var_idx(name), None);
        let name = name.to_string();
        self.locals.push(LocalVar { name, numtype });
    }

    pub fn write_opcode(&mut self, opcode: Opcode) {
        self.bytes.push(opcode as u8);
    }
    pub fn write_byte(&mut self, byte: u8) {
        self.bytes.push(byte);
    }
    pub fn write_slice(&mut self, bytes: &[u8]) {
        self.bytes.extend_from_slice(bytes);
    }
    pub fn write_var(&mut self, name: &str) {
        let idx = self.get_var_idx(name).unwrap();
        self.write_slice(&idx);
    }

    pub fn get_signature(&self) -> &FuncTypeSignature {
        &self.signature
    }
    pub fn get_bytes(&self) -> &[u8] {
        &self.bytes
    }
    pub fn get_local_types(&self) -> Vec<u8> {
        self.locals.iter().map(|x| x.numtype as u8).collect()
    }

    pub fn align_memptr(&mut self) {
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

    // sets offset = fatptr >> 32 and size = fatptr & 0xFFFFFFFF
    pub fn set_offset_and_size(&mut self, fatptr_name: &str, offset_name: &str, size_name: &str) {
        // offset = highest 32 bits of fatptr
        self.write_opcode(Opcode::LocalGet);
        self.write_var(fatptr_name);
        self.write_opcode(Opcode::I64Const);
        self.write_byte(0x20);
        self.write_opcode(Opcode::I64ShrU); // shift right 32 bits
        self.write_opcode(Opcode::I32WrapI64); // discard high 32 bits
        self.write_opcode(Opcode::LocalSet);
        self.write_var(offset_name); // set as offset

        // size = lowest 32 bits of fatptr
        self.write_opcode(Opcode::LocalGet);
        self.write_var(fatptr_name);
        self.write_opcode(Opcode::I32WrapI64); // discard high 32 bits
        self.write_opcode(Opcode::LocalSet);
        self.write_var(size_name); // set as size
    }

    // creates fatptr value of offset << 32 + size
    pub fn create_fatptr(&mut self, offset_name: &str, size_name: &str) {
        self.write_opcode(Opcode::LocalGet);
        self.write_var(offset_name);
        self.write_opcode(Opcode::I64ExtendI32U);
        self.write_opcode(Opcode::I64Const);
        self.write_byte(0x20);
        self.write_opcode(Opcode::I64Shl);
        self.write_opcode(Opcode::LocalGet);
        self.write_var(size_name);
        self.write_opcode(Opcode::I64ExtendI32U);
        self.write_opcode(Opcode::I64Add);
    }

    // copy size bytes from offset to memptr
    // sets offset to value of memptr, then increments memptr by size
    pub fn copy_mem(&mut self, offset_name: &str, size_name: &str) {
        // destination is current value of memptr
        self.write_opcode(Opcode::GlobalGet);
        self.write_byte(0x00);
        // source address is `offset`
        self.write_opcode(Opcode::LocalGet);
        self.write_var(offset_name);
        // copy `size` bytes
        self.write_opcode(Opcode::LocalGet);
        self.write_var(size_name);
        self.write_slice(&MEMCOPY);

        // set offset to memptr (start of new memory block)
        self.write_opcode(Opcode::GlobalGet);
        self.write_byte(0x00);
        self.write_opcode(Opcode::LocalSet);
        self.write_var(offset_name);

        // set memptr to memptr + size
        self.write_opcode(Opcode::GlobalGet);
        self.write_byte(0x00);
        self.write_opcode(Opcode::LocalGet);
        self.write_var(size_name);
        self.write_opcode(Opcode::I32Add);
        self.write_opcode(Opcode::GlobalSet);
        self.write_byte(0x00);
    }

    // Create a new function that will be used to advance an iterator
    pub fn advance_fn_template() -> Self {
        BuiltinFunc::new(
            FuncTypeSignature::new(vec![Numtype::I32], Some(Numtype::I32)),
            vec!["offset".to_string()],
        )
    }

    // Used to call the advance function on an iterator within an iterator
    // Also sets the value of the inner_offset variable
    // Meant to be used as part of the `advance` function for the [outer] iterator
    pub fn iter_call_advance_on_inner(
        &mut self,
        offset_name: &str,
        inner_offset_name: &str,
        inner_offset_delta: u32,
        inner_type: Numtype,
        advance_fn_type_idx: u32,
    ) {
        self.write_opcode(Opcode::LocalGet);
        self.write_var(offset_name);
        self.write_opcode(Opcode::I32Const);
        self.write_slice(&unsigned_leb128(inner_offset_delta)); // index of inner_offset within map iterator
        self.write_opcode(Opcode::I32Add);
        self.write_opcode(Opcode::I32Load);
        self.write_slice(&[0x02, 0x00]);
        self.write_opcode(Opcode::LocalTee);
        self.write_var(inner_offset_name);
        self.write_opcode(Opcode::LocalGet);
        self.write_var(inner_offset_name); // put this on stack again, since we'll also need to pass it to advance fn
        let inner_advance_fn_delta = unsigned_leb128(inner_type.size());
        self.write_opcode(Opcode::I32Const);
        self.write_slice(&inner_advance_fn_delta); // index of advance_fn within inner iterator
        self.write_opcode(Opcode::I32Add);
        self.write_opcode(Opcode::I32Load);
        self.write_slice(&[0x02, 0x00]);
        let advance_fn_signature = unsigned_leb128(advance_fn_type_idx);
        self.write_opcode(Opcode::CallIndirect);
        self.write_slice(&advance_fn_signature);
        self.write_byte(0x00); // table index
    }

    // Used to call the advance function on an iterator
    pub fn iter_call_advance(
        &mut self,
        offset_name: &str,
        advance_fn_delta: u32,
        advance_fn_type_idx: u32,
    ) {
        self.write_opcode(Opcode::LocalGet);
        self.write_var(offset_name);
        self.write_opcode(Opcode::LocalGet); // iter offset
        self.write_var(offset_name);
        self.write_opcode(Opcode::I32Const);
        self.write_slice(&unsigned_leb128(advance_fn_delta));
        self.write_opcode(Opcode::I32Add);
        self.write_opcode(Opcode::I32Load);
        self.write_slice(&[0x02, 0x00]); // advance fn
        self.write_opcode(Opcode::CallIndirect);
        self.write_slice(&unsigned_leb128(advance_fn_type_idx)); // signature index
        self.write_byte(0x00); // table index
    }

    // Used to call map (or filter or whatever) function within an iterator
    // Meant to be used as part of the `advance` function for the [outer] iterator
    pub fn iter_call_map_fn(&mut self, offset_name: &str, map_fn_delta: u32, map_fn_type_idx: u32) {
        self.write_opcode(Opcode::LocalGet);
        self.write_var(offset_name);
        self.write_opcode(Opcode::I32Const);
        self.write_slice(&unsigned_leb128(map_fn_delta)); // index of map_fn within map iterator
        self.write_opcode(Opcode::I32Add);
        self.write_opcode(Opcode::I32Load);
        self.write_slice(&[0x02, 0x00]);
        // call
        self.write_opcode(Opcode::CallIndirect);
        self.write_slice(&unsigned_leb128(map_fn_type_idx));
        self.write_byte(0x00); // table index
    }
}

lazy_static! {
    pub static ref BUILTINS: FxHashMap<String, BuiltinFunc> = {
        let mut map = FxHashMap::default();

        let alloc = {
            let mut func = BuiltinFunc::new(
                FuncTypeSignature::new(vec![Numtype::I32], Some(Numtype::I32)),
                vec!["size".to_string()]
            );

            func.add_local("offset", Numtype::I32);
            func.add_local("current_capacity", Numtype::I32);  // stores the current memory capacity in bytes

            // get memptr (start of next memory chunk)
            func.write_opcode(Opcode::GlobalGet);
            func.write_byte(0x00);
            func.write_opcode(Opcode::LocalSet);
            func.write_var("offset");  // save_location as value to return

            // set memptr to end of new memory chunk (memptr = memptr + size)
            func.write_opcode(Opcode::GlobalGet);
            func.write_byte(0x00);
            func.write_opcode(Opcode::LocalGet);
            func.write_var("size");
            func.write_opcode(Opcode::I32Add);
            func.write_opcode(Opcode::GlobalSet);
            func.write_byte(0x00);

            // get memory capacity in bytes
            func.write_opcode(Opcode::MemorySize);
            func.write_byte(0x00);
            func.write_opcode(Opcode::I32Const);
            func.write_slice(&unsigned_leb128(65536));
            func.write_opcode(Opcode::I32Mul);
            func.write_opcode(Opcode::LocalTee);
            func.write_var("current_capacity");

            // if memptr > memory capacity, grow memory
            func.write_opcode(Opcode::GlobalGet);
            func.write_byte(0x00);
            func.write_opcode(Opcode::I32LeU);
            func.write_opcode(Opcode::If);
            func.write_byte(Numtype::Void as u8);
            // amount to grow is (memptr - memory capacity + 65536) / 65536
            func.write_opcode(Opcode::GlobalGet);
            func.write_byte(0x00);
            func.write_opcode(Opcode::LocalGet);
            func.write_var("current_capacity");
            func.write_opcode(Opcode::I32Sub);
            func.write_opcode(Opcode::I32Const);
            func.write_slice(&unsigned_leb128(65536));
            func.write_opcode(Opcode::I32Add);
            func.write_opcode(Opcode::I32Const);
            func.write_slice(&unsigned_leb128(65536));
            func.write_opcode(Opcode::I32DivU);
            func.write_opcode(Opcode::MemoryGrow);
            func.write_byte(0x00);
            func.write_opcode(Opcode::Drop);  // TODO: Handle when grow fails (= -1)
            func.write_opcode(Opcode::End);  // end if

            // return start of the new memory chunk
            func.write_opcode(Opcode::LocalGet);
            func.write_var("offset");

            func.write_opcode(Opcode::End);

            func
        };
        map.insert("alloc".to_string(), alloc);

        // // Copies a heap object and returns a new fatptr for the object
        // let copy_heap_obj = {
        //     let mut func = BuiltinFunc::new(
        //         FuncTypeSignature::new(vec![Numtype::I64], Some(Numtype::I64)),
        //         vec!["fatptr".to_string()]
        //     );

        //     func.add_local("offset", Numtype::I32);
        //     func.add_local("size", Numtype::I32);

        //     func.set_offset_and_size("fatptr", "offset", "size");
        //     func.copy_mem("offset", "size");

        //     func.align_memptr();

        //     // return [offset, size]
        //     func.create_fatptr("offset", "size");

        //     func.write_opcode(Opcode::End);

        //     func
        // };
        // map.insert("copy_heap_obj".to_string(), copy_heap_obj);

        let concat_heap_objs = {
            let mut func = BuiltinFunc::new(
                FuncTypeSignature::new(vec![Numtype::I64, Numtype::I64], Some(Numtype::I64)),
                vec!["fatptr1".to_string(), "fatptr2".to_string()]
            );

            func.add_local("offset1", Numtype::I32);
            func.add_local("size1", Numtype::I32);
            func.add_local("offset2", Numtype::I32);
            func.add_local("size2", Numtype::I32);

            func.set_offset_and_size("fatptr1", "offset1", "size1");
            func.set_offset_and_size("fatptr2", "offset2", "size2");

            func.copy_mem("offset1", "size1");
            func.copy_mem("offset2", "size2");

            func.align_memptr();

            // return [offset1, size1 + size2]
            func.write_opcode(Opcode::LocalGet);
            func.write_var("offset1");
            func.write_opcode(Opcode::I64ExtendI32U);
            func.write_opcode(Opcode::I64Const);
            func.write_byte(0x20);
            func.write_opcode(Opcode::I64Shl);
            func.write_opcode(Opcode::LocalGet);
            func.write_var("size1");
            func.write_opcode(Opcode::LocalGet);
            func.write_var("size2");
            func.write_opcode(Opcode::I32Add);
            func.write_opcode(Opcode::I64ExtendI32U);
            func.write_opcode(Opcode::I64Add);

            func.write_opcode(Opcode::End);

            func
        };
        map.insert("concat_heap_objs".to_string(), concat_heap_objs);

        let heap_objs_equal = {
            let mut func = BuiltinFunc::new(
                FuncTypeSignature::new(vec![Numtype::I64, Numtype::I64], Some(Numtype::I32)),
                vec!["fatptr1".to_string(), "fatptr2".to_string()]
            );

            func.add_local("offset1", Numtype::I32);
            func.add_local("size1", Numtype::I32);
            func.add_local("offset2", Numtype::I32);
            func.add_local("size2", Numtype::I32);

            func.set_offset_and_size("fatptr1", "offset1", "size1");
            func.set_offset_and_size("fatptr2", "offset2", "size2");

            // check if sizes are equal
            func.write_opcode(Opcode::LocalGet);
            func.write_var("size1");
            func.write_opcode(Opcode::LocalGet);
            func.write_var("size2");
            func.write_opcode(Opcode::I32Eq);
            func.write_opcode(Opcode::If);
            func.write_byte(Numtype::I32 as u8);  // will be 1 if equal, 0 if not

            // case if sizes are equal
            // loop through all values and check if they are equal
            // <inner_offset> is used to store the index of the current value within the loop
            func.add_local("inner_offset", Numtype::I32);
            func.write_opcode(Opcode::I32Const);
            func.write_byte(0x00);
            func.write_opcode(Opcode::LocalSet);
            func.write_var("inner_offset");
            // <equal> is used to store whether the values are equal, initialized to 1
            func.add_local("equal", Numtype::I32);
            func.write_opcode(Opcode::I32Const);
            func.write_byte(0x01);
            func.write_opcode(Opcode::LocalSet);
            func.write_var("equal");
            func.write_opcode(Opcode::Loop);
            func.write_byte(Numtype::Void as u8);
            // read value from memory at offset1 + inner_offset
            func.write_opcode(Opcode::LocalGet);
            func.write_var("inner_offset");
            func.write_opcode(Opcode::LocalGet);
            func.write_var("offset1");
            func.write_opcode(Opcode::I32Add);
            func.write_opcode(Opcode::I32Load);
            func.write_byte(0x02);  // alignment
            func.write_byte(0x00);  // load offset
            // read value from memory at offset2 + inner_offset
            func.write_opcode(Opcode::LocalGet);
            func.write_var("inner_offset");
            func.write_opcode(Opcode::LocalGet);
            func.write_var("offset2");
            func.write_opcode(Opcode::I32Add);
            func.write_opcode(Opcode::I32Load);
            func.write_byte(0x02);  // alignment
            func.write_byte(0x00);  // load offset
            // compare values, update equal, keep that value on stack
            func.write_opcode(Opcode::I32Eq);
            func.write_opcode(Opcode::LocalTee);
            func.write_var("equal");
            // add 4 to inner_offset
            func.write_opcode(Opcode::LocalGet);
            func.write_var("inner_offset");
            func.write_opcode(Opcode::I32Const);
            func.write_byte(0x04);
            func.write_opcode(Opcode::I32Add);
            func.write_opcode(Opcode::LocalTee);
            func.write_var("inner_offset");
            // check if inner_offset < size1
            func.write_opcode(Opcode::LocalGet);
            func.write_var("size1");
            func.write_opcode(Opcode::I32LtU);
            // continue if inner_offset < size1 AND equal == 1
            func.write_opcode(Opcode::I32And);
            func.write_opcode(Opcode::BrIf);
            func.write_byte(0x00);  // break depth
            func.write_opcode(Opcode::End); // end loop
            // return <equal>
            func.write_opcode(Opcode::LocalGet);
            func.write_var("equal");


            func.write_opcode(Opcode::Else);
            // case if sizes are not equal
            func.write_opcode(Opcode::I32Const);
            func.write_byte(0);


            func.write_opcode(Opcode::End); // end if

            func.write_opcode(Opcode::End); // end function

            func
        };
        map.insert("heap_objs_equal".to_string(), heap_objs_equal);

        let get_i32_field = {
            let mut func = BuiltinFunc::new(
                FuncTypeSignature::new(vec![Numtype::I64, Numtype::I32], Some(Numtype::I32)),
                vec!["obj".to_string(), "field_offset".to_string()]
            );
            func.add_local("obj_offset", Numtype::I32);

            // obj_offset = obj >> 32
            func.write_opcode(Opcode::LocalGet);
            func.write_var("obj");
            func.write_opcode(Opcode::I64Const);
            func.write_byte(0x20);
            func.write_opcode(Opcode::I64ShrU);
            func.write_opcode(Opcode::I32WrapI64);
            func.write_opcode(Opcode::LocalTee);
            func.write_var("obj_offset");
            // read i32 at obj_offset + field_offset
            func.write_opcode(Opcode::LocalGet);
            func.write_var("field_offset");
            func.write_opcode(Opcode::I32Add);
            func.write_opcode(Opcode::I32Load);
            func.write_byte(0x02);  // alignment
            func.write_byte(0x00);  // load offset

            func.write_opcode(Opcode::End);

            func
        };
        map.insert("get_i32_field".to_string(), get_i32_field);

        let get_f32_field = {
            let mut func = BuiltinFunc::new(
                FuncTypeSignature::new(vec![Numtype::I64, Numtype::I32], Some(Numtype::F32)),
                vec!["obj".to_string(), "field_offset".to_string()]
            );
            func.add_local("obj_offset", Numtype::I32);

            // obj_offset = obj >> 32
            func.write_opcode(Opcode::LocalGet);
            func.write_var("obj");
            func.write_opcode(Opcode::I64Const);
            func.write_byte(0x20);
            func.write_opcode(Opcode::I64ShrU);
            func.write_opcode(Opcode::I32WrapI64);
            func.write_opcode(Opcode::LocalTee);
            func.write_var("obj_offset");
            // read i32 at obj_offset + field_offset
            func.write_opcode(Opcode::LocalGet);
            func.write_var("field_offset");
            func.write_opcode(Opcode::I32Add);
            func.write_opcode(Opcode::F32Load);
            func.write_byte(0x02);  // alignment
            func.write_byte(0x00);  // load offset

            func.write_opcode(Opcode::End);

            func
        };
        map.insert("get_f32_field".to_string(), get_f32_field);

        let get_i64_field = {
            let mut func = BuiltinFunc::new(
                FuncTypeSignature::new(vec![Numtype::I64, Numtype::I32], Some(Numtype::I64)),
                vec!["obj".to_string(), "field_offset".to_string()]
            );
            func.add_local("obj_offset", Numtype::I32);

            // obj_offset = obj >> 32
            func.write_opcode(Opcode::LocalGet);
            func.write_var("obj");
            func.write_opcode(Opcode::I64Const);
            func.write_byte(0x20);
            func.write_opcode(Opcode::I64ShrU);
            func.write_opcode(Opcode::I32WrapI64);
            func.write_opcode(Opcode::LocalTee);
            func.write_var("obj_offset");
            // read i64 at obj_offset + field_offset
            func.write_opcode(Opcode::LocalGet);
            func.write_var("field_offset");
            func.write_opcode(Opcode::I32Add);
            func.write_opcode(Opcode::I64Load);
            func.write_byte(0x02);  // alignment
            func.write_byte(0x00);  // load offset

            func.write_opcode(Opcode::End);

            func
        };
        map.insert("get_i64_field".to_string(), get_i64_field);

        map
    };
}

pub fn define_builtin_abs_int() -> BuiltinFunc {
    let mut func = BuiltinFunc::new(
        FuncTypeSignature::new(vec![Numtype::I32], Some(Numtype::I32)),
        vec!["x".to_string()],
    );
    func.add_local("mask", Numtype::I32);
    func.write_opcode(Opcode::LocalGet);
    func.write_var("x");
    // there is no I32Abs opcode. Instead we'll calculate mask = x >> 31, then (x ^ mask) - mask
    func.write_opcode(Opcode::I32Const);
    func.write_byte(31);
    func.write_opcode(Opcode::I32ShrS);
    func.write_opcode(Opcode::LocalTee);
    func.write_var("mask");
    func.write_opcode(Opcode::LocalGet);
    func.write_var("x");
    func.write_opcode(Opcode::I32Xor);
    func.write_opcode(Opcode::LocalGet);
    func.write_var("mask");
    func.write_opcode(Opcode::I32Sub);
    func.write_opcode(Opcode::End);

    func
}

pub fn define_builtin_abs_float() -> BuiltinFunc {
    let mut func = BuiltinFunc::new(
        FuncTypeSignature::new(vec![Numtype::F32], Some(Numtype::F32)),
        vec!["x".to_string()],
    );
    func.write_opcode(Opcode::LocalGet);
    func.write_var("x");
    func.write_opcode(Opcode::F32Abs);
    func.write_opcode(Opcode::End);

    func
}

pub fn define_builtin_itof() -> BuiltinFunc {
    let mut func = BuiltinFunc::new(
        FuncTypeSignature::new(vec![Numtype::I32], Some(Numtype::F32)),
        vec!["x".to_string()],
    );
    func.write_opcode(Opcode::LocalGet);
    func.write_var("x");
    func.write_opcode(Opcode::F32ConvertI32S);
    func.write_opcode(Opcode::End);

    func
}

pub fn define_builtin_ftoi() -> BuiltinFunc {
    let mut func = BuiltinFunc::new(
        FuncTypeSignature::new(vec![Numtype::F32], Some(Numtype::I32)),
        vec!["x".to_string()],
    );
    func.write_opcode(Opcode::LocalGet);
    func.write_var("x");
    func.write_opcode(Opcode::I32TruncF32S);
    func.write_opcode(Opcode::End);

    func
}

pub fn define_builtin_sqrt_float() -> BuiltinFunc {
    let mut func = BuiltinFunc::new(
        FuncTypeSignature::new(vec![Numtype::F32], Some(Numtype::F32)),
        vec!["x".to_string()],
    );
    func.write_opcode(Opcode::LocalGet);
    func.write_var("x");
    func.write_opcode(Opcode::F32Sqrt);
    func.write_opcode(Opcode::End);

    func
}

pub fn define_builtin_mod() -> BuiltinFunc {
    let mut func = BuiltinFunc::new(
        FuncTypeSignature::new(vec![Numtype::I32, Numtype::I32], Some(Numtype::I32)),
        vec!["x".to_string(), "y".to_string()],
    );
    // Do a few extra steps, since we want the answer to always be positive
    // mod(x, y) = ((x % y) + y) % y
    func.write_opcode(Opcode::LocalGet);
    func.write_var("x");
    func.write_opcode(Opcode::LocalGet);
    func.write_var("y");
    func.write_opcode(Opcode::I32RemS);
    func.write_opcode(Opcode::LocalGet);
    func.write_var("y");
    func.write_opcode(Opcode::I32Add);
    func.write_opcode(Opcode::LocalGet);
    func.write_var("y");
    func.write_opcode(Opcode::I32RemS);
    func.write_opcode(Opcode::End);

    func
}

pub fn define_builtin_iter_len(advance_fn_delta: u32, advance_fn_type_idx: u32) -> BuiltinFunc {
    let mut func = BuiltinFunc::new(
        FuncTypeSignature::new(vec![Numtype::I64], Some(Numtype::I32)),
        vec!["iter_fatptr".to_string()],
    );
    func.add_local("iter_offset", Numtype::I32);
    func.add_local("count", Numtype::I32);

    // iter_offset = iter_fatpr >> 32
    func.write_opcode(Opcode::LocalGet);
    func.write_var("iter_fatptr");
    func.write_opcode(Opcode::I64Const);
    func.write_byte(0x20);
    func.write_opcode(Opcode::I64ShrU);
    func.write_opcode(Opcode::I32WrapI64);
    func.write_opcode(Opcode::LocalSet);
    func.write_var("iter_offset");

    func.write_opcode(Opcode::Loop);
    func.write_byte(Numtype::Void as u8);

    // call iterator->advance
    func.iter_call_advance("iter_offset", advance_fn_delta, advance_fn_type_idx);

    // if !iterator->done
    func.write_opcode(Opcode::I32Eqz);
    func.write_opcode(Opcode::If);
    func.write_byte(Numtype::Void as u8);
    // count += 1
    func.write_opcode(Opcode::LocalGet);
    func.write_var("count");
    func.write_opcode(Opcode::I32Const);
    func.write_byte(1);
    func.write_opcode(Opcode::I32Add);
    func.write_opcode(Opcode::LocalSet);
    func.write_var("count");
    // branch to loop
    func.write_opcode(Opcode::Br);
    func.write_byte(1); // 1 since we need to leave the if statement

    func.write_opcode(Opcode::End); // end if

    func.write_opcode(Opcode::End); // end loop

    // return count
    func.write_opcode(Opcode::LocalGet);
    func.write_var("count");

    func.write_opcode(Opcode::End); // end function

    func
}

pub fn define_builtin_str_len() -> BuiltinFunc {
    let mut func = BuiltinFunc::new(
        FuncTypeSignature::new(vec![Numtype::I64], Some(Numtype::I32)),
        vec!["str_fatptr".to_string()],
    );
    func.add_local("offset", Numtype::I32);
    func.add_local("size", Numtype::I32);
    func.add_local("count", Numtype::I32);
    func.add_local("leading_byte", Numtype::I32);

    func.set_offset_and_size("str_fatptr", "offset", "size");

    // loop:
    // if offset == size: return count
    // count += 1
    // char_size := if first bit == 0 {
    //     1
    // }
    // else {
    //     if third bit == 0 {
    //         2
    //     }
    //     else {
    //         if fourth bit == 0 {
    //             3
    //         }
    //         else {
    //             4
    //         }
    //     }
    // }
    // offset += char_size
    // branch to loop

    func.write_opcode(Opcode::Loop);
    func.write_byte(Numtype::I32 as u8);

    // if offset == size: return count
    func.write_opcode(Opcode::LocalGet);
    func.write_var("offset");
    func.write_opcode(Opcode::LocalGet);
    func.write_var("size");
    func.write_opcode(Opcode::I32Eq);
    func.write_opcode(Opcode::If);
    func.write_byte(Numtype::Void as u8);
    func.write_opcode(Opcode::LocalGet);
    func.write_var("count");
    func.write_opcode(Opcode::Return);
    func.write_opcode(Opcode::End); // end if

    // count += 1
    func.write_opcode(Opcode::LocalGet);
    func.write_var("count");
    func.write_opcode(Opcode::I32Const);
    func.write_byte(1);
    func.write_opcode(Opcode::I32Add);
    func.write_opcode(Opcode::LocalSet);
    func.write_var("count");

    // now figure out size of current char
    // read current byte
    func.write_opcode(Opcode::LocalGet);
    func.write_var("offset");
    func.write_opcode(Opcode::I32Load8U);
    func.write_slice(&[0x00, 0x00]);
    func.write_opcode(Opcode::LocalTee);
    func.write_var("leading_byte");

    // first bit of leading byte == 1
    func.write_opcode(Opcode::I32Const);
    func.write_byte(7);
    func.write_opcode(Opcode::I32ShrU);
    func.write_opcode(Opcode::If); // if #0
    func.write_byte(Numtype::I32 as u8);

    // third bit of leading byte == 1
    func.write_opcode(Opcode::LocalGet);
    func.write_var("leading_byte");
    func.write_opcode(Opcode::I32Const);
    func.write_byte(5);
    func.write_opcode(Opcode::I32ShrU);
    func.write_opcode(Opcode::I32Const);
    func.write_byte(1);
    func.write_opcode(Opcode::I32And);

    func.write_opcode(Opcode::If); // if #1
    func.write_byte(Numtype::I32 as u8);

    // fourth bit of leading byte == 1
    func.write_opcode(Opcode::LocalGet);
    func.write_var("leading_byte");
    func.write_opcode(Opcode::I32Const);
    func.write_byte(4);
    func.write_opcode(Opcode::I32ShrU);
    func.write_opcode(Opcode::I32Const);
    func.write_byte(1);
    func.write_opcode(Opcode::I32And);

    func.write_opcode(Opcode::If); // if #2
    func.write_byte(Numtype::I32 as u8);

    func.write_opcode(Opcode::I32Const);
    func.write_byte(4);

    // third bit of leading byte == 0
    func.write_opcode(Opcode::Else); // else #2

    func.write_opcode(Opcode::I32Const);
    func.write_byte(3);

    func.write_opcode(Opcode::End); // end if#2

    // second bit of leading byte == 0
    func.write_opcode(Opcode::Else); // else #1

    func.write_opcode(Opcode::I32Const);
    func.write_byte(2);

    func.write_opcode(Opcode::End); // end if#1

    // first bit of leading byte == 0
    func.write_opcode(Opcode::Else); // else #0

    func.write_opcode(Opcode::I32Const);
    func.write_byte(1);

    func.write_opcode(Opcode::End); // end if#0

    // current char number of bytes should now be on stack
    // add that to offset
    func.write_opcode(Opcode::LocalGet);
    func.write_var("offset");
    func.write_opcode(Opcode::I32Add);
    func.write_opcode(Opcode::LocalSet);
    func.write_var("offset");

    // branch to loop
    func.write_opcode(Opcode::Br);
    func.write_byte(0x00);

    func.write_opcode(Opcode::End); // end loop

    func.write_opcode(Opcode::End); // end function

    func
}

pub fn define_builtin_unwrap(numtype: Numtype) -> BuiltinFunc {
    let mut func = BuiltinFunc::new(
        FuncTypeSignature::new(vec![Numtype::I64, numtype], Some(numtype)),
        vec!["maybe_value".to_string(), "default".to_string()],
    );
    func.add_local("maybe_value_offset", Numtype::I32);

    // get the offset from the maybe_value fatptr
    func.write_opcode(Opcode::LocalGet);
    func.write_var("maybe_value");
    func.write_opcode(Opcode::I64Const);
    func.write_byte(0x20);
    func.write_opcode(Opcode::I64ShrU);
    func.write_opcode(Opcode::I32WrapI64);
    func.write_opcode(Opcode::LocalTee);
    func.write_var("maybe_value_offset");

    // get the is_some (second) field from the maybe_value
    func.write_opcode(Opcode::I32Const);
    func.write_slice(&unsigned_leb128(numtype.size())); // offset of is_some field
    func.write_opcode(Opcode::I32Add);
    func.write_opcode(Opcode::I32Load);
    func.write_slice(&[0x02, 0x00]);

    // if is_some is 1, return the value, otherwise return default
    func.write_opcode(Opcode::If);
    func.write_byte(numtype as u8);

    // get the value
    func.write_opcode(Opcode::LocalGet);
    func.write_var("maybe_value_offset");
    func.write_opcode(numtype.load_op());
    func.write_slice(&[0x02, 0x00]);

    func.write_opcode(Opcode::Else);

    func.write_opcode(Opcode::LocalGet);
    func.write_var("default");

    func.write_opcode(Opcode::End); // end if

    func.write_opcode(Opcode::End); // end function

    func
}

pub fn define_builtin_is_some(numtype: Numtype) -> BuiltinFunc {
    let mut func = BuiltinFunc::new(
        FuncTypeSignature::new(vec![Numtype::I64], Some(Numtype::I32)),
        vec!["maybe_value".to_string()],
    );
    func.add_local("maybe_value_offset", Numtype::I32);

    // get the offset from the maybe_value fatptr
    func.write_opcode(Opcode::LocalGet);
    func.write_var("maybe_value");
    func.write_opcode(Opcode::I64Const);
    func.write_byte(0x20);
    func.write_opcode(Opcode::I64ShrU);
    func.write_opcode(Opcode::I32WrapI64);
    func.write_opcode(Opcode::LocalTee);
    func.write_var("maybe_value_offset");

    // get the is_some (second) field from the maybe_value
    func.write_opcode(Opcode::I32Const);
    func.write_slice(&unsigned_leb128(numtype.size())); // offset of is_some field
    func.write_opcode(Opcode::I32Add);
    func.write_opcode(Opcode::I32Load);
    func.write_slice(&[0x02, 0x00]);

    // if is_some is 1, return 1 (true), otherwise return 0 (false)
    func.write_opcode(Opcode::If);
    func.write_byte(Numtype::I32 as u8);

    func.write_opcode(Opcode::I32Const);
    func.write_byte(1);

    func.write_opcode(Opcode::Else);

    func.write_opcode(Opcode::I32Const);
    func.write_byte(0);

    func.write_opcode(Opcode::End); // end if

    func.write_opcode(Opcode::End); // end function

    func
}

pub fn define_builtin_struct_constructor(struct_def: &Struct, alloc_fn_idx: u32) -> BuiltinFunc {
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

    // // copy memory for all the heap fields
    // let copy_idx = unsigned_leb128(*self.builtins.get("copy_heap_obj").unwrap());
    // for (name, field) in struct_def.fields.iter() {
    //     if field.nt != Numtype::I64 {
    //         continue;
    //     }
    //     func.write_opcode(Opcode::LocalGet);
    //     func.write_var(name);
    //     func.write_opcode(Opcode::Call);
    //     func.write_slice(&copy_idx);
    //     func.write_opcode(Opcode::LocalSet);
    //     func.write_var(name);
    // }

    func.add_local("<offset>", Numtype::I32);
    func.add_local("<size>", Numtype::I32);
    // allocate memory for the struct
    let alloc_idx = unsigned_leb128(alloc_fn_idx);
    func.write_opcode(Opcode::I32Const);
    func.write_slice(&unsigned_leb128(struct_def.size));
    func.write_opcode(Opcode::LocalTee);
    func.write_var("<size>");
    func.write_opcode(Opcode::Call);
    func.write_slice(&alloc_idx);
    func.write_opcode(Opcode::LocalSet);
    func.write_var("<offset>");

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

    func
}

pub fn define_builtin_range_iter_advance() -> BuiltinFunc {
    // function will take the offset of the start of a range struct, update the "current" field, and return a bool (1 if the iterator is done)
    let mut func = BuiltinFunc::advance_fn_template();
    func.add_local("current", Numtype::I32);
    func.add_local("step", Numtype::I32);
    // add "step" to "current"
    // load value at "step"
    func.write_opcode(Opcode::LocalGet);
    func.write_var("offset"); // step = *(offset + 2 * 4)
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

    // return current == stop + step
    // (1 if done, 0 otherwise)
    // stop = *(offset + 3 * 4)
    func.write_opcode(Opcode::LocalGet);
    func.write_var("offset");
    func.write_opcode(Opcode::I32Const);
    func.write_byte(3 * 4);
    func.write_opcode(Opcode::I32Add);
    func.write_opcode(Opcode::I32Load);
    func.write_slice(&[0x02, 0x00]);
    func.write_opcode(Opcode::LocalGet);
    func.write_var("step");
    func.write_opcode(Opcode::I32Add);
    func.write_opcode(Opcode::LocalGet);
    func.write_var("current");
    func.write_opcode(Opcode::I32Eq);

    func.write_opcode(Opcode::End);

    func
}

pub fn define_builtin_range_iter_factory(
    constructor_idx: u32,
    advance_fn_table_idx: u32,
) -> BuiltinFunc {
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
    // current is initialized to start - step, since the iterator should only be at its first valid state after calling advance for the first time
    func.write_opcode(Opcode::LocalGet);
    func.write_var("start");
    func.write_opcode(Opcode::LocalGet);
    func.write_var("step");
    func.write_opcode(Opcode::I32Sub);
    // advance_fn
    func.write_opcode(Opcode::I32Const);
    func.write_slice(&unsigned_leb128(advance_fn_table_idx));
    // step
    func.write_opcode(Opcode::LocalGet);
    func.write_var("step");
    // stop
    func.write_opcode(Opcode::LocalGet);
    func.write_var("stop");
    func.write_opcode(Opcode::Call);
    func.write_slice(&unsigned_leb128(constructor_idx));

    func.write_opcode(Opcode::End);

    func
}

pub fn define_builtin_map_iter_advance(
    in_type: Numtype,
    out_type: Numtype,
    inner_offset_delta: u32,
    map_fn_delta: u32,
    advance_fn_type_idx: u32,
    map_fn_type_idx: u32,
) -> BuiltinFunc {
    // function will take the offset of the start of a range struct, update the "current" field, and return a bool (1 if the iterator is done)
    let mut func = BuiltinFunc::advance_fn_template();
    func.add_local("inner_offset", Numtype::I32); // will store the offset of the inner iterator in memory
    func.add_local("current", out_type); // will store the current value of the map iterator

    // call `advance` on the inner iterator
    func.iter_call_advance_on_inner(
        "offset",
        "inner_offset",
        inner_offset_delta,
        in_type,
        advance_fn_type_idx,
    );

    // if done, we can just return that here
    func.write_opcode(Opcode::If);
    func.write_byte(Numtype::I32 as u8);

    func.write_opcode(Opcode::I32Const);
    func.write_byte(1); // done = 1

    // otherwise, we need to get the current value from the inner iterator
    // and map through function
    func.write_opcode(Opcode::Else);

    let load_op = in_type.load_op();
    // now get inner_current
    func.write_opcode(Opcode::LocalGet);
    func.write_var("inner_offset");
    func.write_opcode(load_op);
    func.write_slice(&[0x02, 0x00]);

    // pass inner_current to mapping fn
    func.iter_call_map_fn("offset", map_fn_delta, map_fn_type_idx);

    // // When the current value is an I64 (heap object), we need to also copy the memory for that value
    // if in_type == Numtype::I64 {
    //     func.write_opcode(Opcode::Call);
    //     let copy_heap_obj_idx = self.builtins.get("copy_heap_obj").unwrap();
    //     func.write_slice(&unsigned_leb128(*copy_heap_obj_idx));
    // }

    // set as new current value
    func.write_opcode(Opcode::LocalSet);
    func.write_var("current");

    func.write_opcode(Opcode::LocalGet);
    func.write_var("offset");
    func.write_opcode(Opcode::LocalGet);
    func.write_var("current");
    func.write_opcode(out_type.store_op());
    func.write_slice(&[0x02, 0x00]);

    // return 0 (for not done)
    func.write_opcode(Opcode::I32Const);
    func.write_byte(0);

    func.write_opcode(Opcode::End); // end if

    func.write_opcode(Opcode::End); // end function

    func
}

pub fn define_builtin_map_iter_factory(
    out_type: Numtype,
    constructor_idx: u32,
    advance_fn_table_idx: u32,
) -> BuiltinFunc {
    let mut func = BuiltinFunc::new(
        FuncTypeSignature::new(vec![Numtype::I32, Numtype::I64], Some(Numtype::I64)),
        vec!["map_fn".to_string(), "iter_over".to_string()],
    );

    // pass values to constructor
    // current can just be set to an arbitrary value, since its initial state doesn't matter
    func.write_opcode(out_type.const_op());
    match out_type {
        Numtype::F32 => func.write_slice(&[0x00, 0x00, 0x00, 0x00]),
        _ => func.write_byte(0x00),
    };
    // advance_fn
    func.write_opcode(Opcode::I32Const);
    func.write_slice(&unsigned_leb128(advance_fn_table_idx));
    // map_fn
    func.write_opcode(Opcode::LocalGet);
    func.write_var("map_fn");
    // iter_offset = iter_over >> 32
    func.write_opcode(Opcode::LocalGet);
    func.write_var("iter_over");
    func.write_opcode(Opcode::I64Const);
    func.write_byte(0x20);
    func.write_opcode(Opcode::I64ShrU);
    func.write_opcode(Opcode::I32WrapI64);

    func.write_opcode(Opcode::Call);
    func.write_slice(&unsigned_leb128(constructor_idx));

    func.write_opcode(Opcode::End);

    func
}

pub fn define_builtin_scan_iter_advance(
    acc_type: Numtype,
    x_type: Numtype,
    inner_offset_delta: u32,
    reduce_fn_delta: u32,
    advance_fn_type_idx: u32,
    reduce_fn_type_idx: u32,
) -> BuiltinFunc {
    let mut func = BuiltinFunc::advance_fn_template();
    func.add_local("inner_offset", Numtype::I32);
    func.add_local("current", acc_type);

    // start by calling `advance` on the inner iterator
    func.iter_call_advance_on_inner(
        "offset",
        "inner_offset",
        inner_offset_delta,
        x_type,
        advance_fn_type_idx,
    );

    // if done, return 1 here
    func.write_opcode(Opcode::If);
    func.write_byte(Numtype::I32 as u8);

    func.write_opcode(Opcode::I32Const);
    func.write_byte(0x01);

    // otherwise, get current value from inner iterator and use to update accumulator
    func.write_opcode(Opcode::Else);

    // get values of acc (current) and x (inner current) variables
    let acc_load_op = acc_type.load_op();
    let x_load_op = x_type.load_op();
    // get acc (current)
    func.write_opcode(Opcode::LocalGet);
    func.write_var("offset");
    func.write_opcode(acc_load_op);
    func.write_slice(&[0x02, 0x00]);
    // get x (inner current)
    func.write_opcode(Opcode::LocalGet);
    func.write_var("inner_offset");
    func.write_opcode(x_load_op);
    func.write_slice(&[0x02, 0x00]);

    // pass acc and x to reduce_fn
    func.iter_call_map_fn("offset", reduce_fn_delta, reduce_fn_type_idx);

    // set as new acc (current) value
    func.write_opcode(Opcode::LocalSet);
    func.write_var("current");

    let store_op = acc_type.store_op();
    func.write_opcode(Opcode::LocalGet);
    func.write_var("offset");
    func.write_opcode(Opcode::LocalGet);
    func.write_var("current");
    func.write_opcode(store_op);
    func.write_slice(&[0x02, 0x00]);

    // return 0
    func.write_opcode(Opcode::I32Const);
    func.write_byte(0x00);

    func.write_opcode(Opcode::End); // end if

    func.write_opcode(Opcode::End); // end function

    func
}

pub fn define_builtin_scan_iter_factory(
    acc_type: Numtype,
    advance_fn_type_idx: u32,
    constructor_idx: u32,
) -> BuiltinFunc {
    let mut func = BuiltinFunc::new(
        FuncTypeSignature::new(
            vec![Numtype::I32, acc_type, Numtype::I64],
            Some(Numtype::I64),
        ),
        vec![
            "reduce_fn".to_string(),
            "init".to_string(),
            "iter_over".to_string(),
        ],
    );
    // current = init
    func.write_opcode(Opcode::LocalGet);
    func.write_var("init");
    // advance_fn
    func.write_opcode(Opcode::I32Const);
    func.write_slice(&unsigned_leb128(advance_fn_type_idx));
    // reduce_fn
    func.write_opcode(Opcode::LocalGet);
    func.write_var("reduce_fn");
    // inner_offset = iter_over >> 32
    func.write_opcode(Opcode::LocalGet);
    func.write_var("iter_over");
    func.write_opcode(Opcode::I64Const);
    func.write_byte(0x20);
    func.write_opcode(Opcode::I64ShrU);
    func.write_opcode(Opcode::I32WrapI64);

    func.write_opcode(Opcode::Call);
    func.write_slice(&unsigned_leb128(constructor_idx));

    func.write_opcode(Opcode::End);

    func
}

pub fn define_builtin_filter_iter_advance(
    inner_type: Numtype,
    inner_offset_delta: u32,
    filter_fn_delta: u32,
    advance_fn_type_idx: u32,
    filter_fn_type_idx: u32,
) -> BuiltinFunc {
    let mut func = BuiltinFunc::advance_fn_template();
    func.add_local("inner_offset", Numtype::I32);
    func.add_local("current", inner_type);
    func.add_local("inner_current", inner_type);

    // This will all be in a loop, since we want to keep going until we run out of values or get a value that passes the filter
    func.write_opcode(Opcode::Loop);
    func.write_byte(Numtype::I32 as u8);

    // call advance on the inner iterator
    func.iter_call_advance_on_inner(
        "offset",
        "inner_offset",
        inner_offset_delta,
        inner_type,
        advance_fn_type_idx,
    );

    // if done, return 1 here
    func.write_opcode(Opcode::If);
    func.write_byte(Numtype::I32 as u8);

    func.write_opcode(Opcode::I32Const);
    func.write_byte(0x01);

    // otherwise, get the inner current value, and call the filter function on it; repeat until getting a value that isn't filtered out
    func.write_opcode(Opcode::Else);

    // get inner current
    func.write_opcode(Opcode::LocalGet);
    func.write_var("inner_offset");
    func.write_opcode(inner_type.load_op());
    func.write_slice(&[0x02, 0x00]);
    func.write_opcode(Opcode::LocalTee);
    func.write_var("inner_current");

    // pass to filter fn
    func.iter_call_map_fn("offset", filter_fn_delta, filter_fn_type_idx);

    // if result is false (0), branch to start of loop
    func.write_opcode(Opcode::I32Eqz);
    func.write_opcode(Opcode::BrIf);
    func.write_byte(0x01); // break depth is 1 since we need to get out of the if statement

    // otherwise, this value is not filtered out, so set it as the new current value
    func.write_opcode(Opcode::LocalGet);
    func.write_var("inner_current");
    func.write_opcode(Opcode::LocalSet);
    func.write_var("current");

    func.write_opcode(Opcode::LocalGet);
    func.write_var("offset");
    func.write_opcode(Opcode::LocalGet);
    func.write_var("current");
    func.write_opcode(inner_type.store_op());
    func.write_slice(&[0x02, 0x00]);

    // return 0 (not done)
    func.write_opcode(Opcode::I32Const);
    func.write_byte(0x00);

    func.write_opcode(Opcode::End); // end if

    func.write_opcode(Opcode::End); // end loop

    func.write_opcode(Opcode::End); // end function

    func
}

pub fn define_builtin_filter_iter_factory(
    inner_type: Numtype,
    advance_fn_table_idx: u32,
    constructor_idx: u32,
) -> BuiltinFunc {
    let mut func = BuiltinFunc::new(
        FuncTypeSignature::new(vec![Numtype::I32, Numtype::I64], Some(Numtype::I64)),
        vec!["filter_fn".to_string(), "iter_over".to_string()],
    );
    // initial value of current is arbitrary, just set to 0
    func.write_opcode(inner_type.const_op());
    match inner_type {
        Numtype::F32 => func.write_slice(&[0x00, 0x00, 0x00, 0x00]),
        _ => func.write_byte(0x00),
    };
    // advance_fn
    func.write_opcode(Opcode::I32Const);
    func.write_slice(&unsigned_leb128(advance_fn_table_idx));
    // filter_fn
    func.write_opcode(Opcode::LocalGet);
    func.write_var("filter_fn");
    // inner_offset = iter_over >> 32
    func.write_opcode(Opcode::LocalGet);
    func.write_var("iter_over");
    func.write_opcode(Opcode::I64Const);
    func.write_byte(0x20);
    func.write_opcode(Opcode::I64ShrU);
    func.write_opcode(Opcode::I32WrapI64);

    func.write_opcode(Opcode::Call);
    func.write_slice(&unsigned_leb128(constructor_idx));

    func.write_opcode(Opcode::End);

    func
}

pub fn define_builtin_zipmap_iter_advance(
    iter_over_types: &[Numtype],
    out_type: Numtype,
    inner_offset_deltas: &[u32],
    map_fn_delta: u32,
    advance_fn_type_idx: u32,
    map_fn_type_idx: u32,
) -> BuiltinFunc {
    let mut func = BuiltinFunc::advance_fn_template();
    for i in 0..iter_over_types.len() {
        func.add_local(&format!("inner_{}_offset", i), Numtype::I32);
    }
    func.add_local("current", out_type);
    func.add_local("done", Numtype::I32);

    // call advance on *each* of the inner iterators
    for (i, (inner_offset_delta, inner_type)) in inner_offset_deltas
        .iter()
        .zip(iter_over_types.iter())
        .enumerate()
    {
        func.iter_call_advance_on_inner(
            "offset",
            &format!("inner_{}_offset", i),
            *inner_offset_delta,
            *inner_type,
            advance_fn_type_idx,
        );
        // accumulate "done" value
        func.write_opcode(Opcode::LocalGet);
        func.write_var("done");
        func.write_opcode(Opcode::I32Or);
        func.write_opcode(Opcode::LocalSet);
        func.write_var("done");
    }

    // if "done" is true, we are done
    func.write_opcode(Opcode::LocalGet);
    func.write_var("done");
    func.write_opcode(Opcode::If);
    func.write_byte(Numtype::I32 as u8);

    func.write_opcode(Opcode::I32Const);
    func.write_byte(1); // done = true

    // else, update current value
    func.write_opcode(Opcode::Else);

    // put all the inner current values on the stack
    for (i, numtype) in iter_over_types.iter().enumerate() {
        func.write_opcode(Opcode::LocalGet);
        func.write_var(&format!("inner_{}_offset", i));
        func.write_opcode(numtype.load_op());
        func.write_slice(&[0x02, 0x00]);
    }

    // call map fn
    func.iter_call_map_fn("offset", map_fn_delta, map_fn_type_idx);

    // set as new current value
    func.write_opcode(Opcode::LocalSet);
    func.write_var("current");

    func.write_opcode(Opcode::LocalGet);
    func.write_var("offset");
    func.write_opcode(Opcode::LocalGet);
    func.write_var("current");
    func.write_opcode(out_type.store_op());
    func.write_slice(&[0x02, 0x00]);

    // return 0 (for not done)
    func.write_opcode(Opcode::I32Const);
    func.write_byte(0);

    func.write_opcode(Opcode::End); // end if

    func.write_opcode(Opcode::End); // end function

    func
}

pub fn define_builtin_zipmap_iter_factory(
    iter_over_types: &[Numtype],
    out_type: Numtype,
    advance_fn_table_idx: u32,
    constructor_idx: u32,
) -> BuiltinFunc {
    let argtypes = [
        vec![Numtype::I32],
        vec![Numtype::I64; iter_over_types.len()],
    ]
    .concat();
    let param_names = [
        vec!["map_fn".to_string()],
        (0..iter_over_types.len())
            .map(|i| format!("iter_over_{}", i))
            .collect::<Vec<_>>(),
    ]
    .concat();
    let mut func = BuiltinFunc::new(
        FuncTypeSignature::new(argtypes, Some(Numtype::I64)),
        param_names,
    );

    // initialize current to 0
    func.write_opcode(out_type.const_op());
    match out_type {
        Numtype::F32 => func.write_slice(&[0x00, 0x00, 0x00, 0x00]),
        _ => func.write_byte(0x00),
    };
    // advance fn
    func.write_opcode(Opcode::I32Const);
    func.write_slice(&unsigned_leb128(advance_fn_table_idx));
    // map fn
    func.write_opcode(Opcode::LocalGet);
    func.write_var("map_fn");
    // each of the iterators
    for i in 0..iter_over_types.len() {
        // iter_offset = iter_over >> 32
        func.write_opcode(Opcode::LocalGet);
        func.write_var(&format!("iter_over_{}", i));
        func.write_opcode(Opcode::I64Const);
        func.write_byte(0x20);
        func.write_opcode(Opcode::I64ShrU);
        func.write_opcode(Opcode::I32WrapI64);
    }

    func.write_opcode(Opcode::Call);
    func.write_slice(&unsigned_leb128(constructor_idx));

    func.write_opcode(Opcode::End);

    func
}

pub fn define_builtin_array_iter_advance(
    numtype: Numtype,
    inner_offset_delta: u32,
    max_inner_offset_delta: u32,
) -> BuiltinFunc {
    let mut func = BuiltinFunc::advance_fn_template();
    func.add_local("current", numtype);
    func.add_local("inner_offset", Numtype::I32);

    let load_op = numtype.load_op();
    let store_op = numtype.store_op();

    // get the inner offset
    func.write_opcode(Opcode::LocalGet);
    func.write_var("offset");
    func.write_opcode(Opcode::I32Const);
    func.write_slice(&unsigned_leb128(inner_offset_delta));
    func.write_opcode(Opcode::I32Add);
    func.write_opcode(Opcode::I32Load);
    func.write_slice(&[0x02, 0x00]);
    func.write_opcode(Opcode::LocalTee);
    func.write_var("inner_offset");
    // read the memory at that location
    func.write_opcode(load_op);
    func.write_slice(&[0x02, 0x00]);

    // // If the current value is a heap type, we need to copy it
    // if numtype == Numtype::I64 {
    //     func.write_opcode(Opcode::Call);
    //     let copy_heap_obj_idx = self.builtins.get("copy_heap_obj").unwrap();
    //     func.write_slice(&unsigned_leb128(*copy_heap_obj_idx));
    // }

    // set this as the new current value
    func.write_opcode(Opcode::LocalSet);
    func.write_var("current");
    // also store it in the iterator struct
    func.write_opcode(Opcode::LocalGet);
    func.write_var("offset");
    func.write_opcode(Opcode::LocalGet);
    func.write_var("current");
    func.write_opcode(store_op);
    func.write_slice(&[0x02, 0x00]);

    // increment inner offset
    func.write_opcode(Opcode::LocalGet);
    func.write_var("inner_offset");
    func.write_opcode(Opcode::I32Const);
    func.write_slice(&unsigned_leb128(numtype.size()));
    func.write_opcode(Opcode::I32Add);
    func.write_opcode(Opcode::LocalSet);
    func.write_var("inner_offset");
    // store the new value in the iterator struct
    func.write_opcode(Opcode::LocalGet);
    func.write_var("offset");
    func.write_opcode(Opcode::I32Const);
    func.write_slice(&unsigned_leb128(inner_offset_delta));
    func.write_opcode(Opcode::I32Add);
    func.write_opcode(Opcode::LocalGet);
    func.write_var("inner_offset");
    func.write_opcode(Opcode::I32Store);
    func.write_slice(&[0x02, 0x00]);

    // return inner_offset > max_inner_offset
    func.write_opcode(Opcode::LocalGet);
    func.write_var("inner_offset");
    func.write_opcode(Opcode::LocalGet);
    func.write_var("offset");
    func.write_opcode(Opcode::I32Const);
    func.write_slice(&unsigned_leb128(max_inner_offset_delta));
    func.write_opcode(Opcode::I32Add);
    func.write_opcode(Opcode::I32Load);
    func.write_slice(&[0x02, 0x00]);
    func.write_opcode(Opcode::I32GtU);

    func.write_opcode(Opcode::End);

    func
}

pub fn define_builtin_array_iter_factory(
    numtype: Numtype,
    advance_fn_table_idx: u32,
    constructor_idx: u32,
) -> BuiltinFunc {
    let mut func = BuiltinFunc::new(
        FuncTypeSignature::new(vec![Numtype::I64], Some(Numtype::I64)),
        vec!["iter_fatptr".to_string()],
    );
    func.add_local("iter_offset", Numtype::I32);
    func.add_local("iter_size", Numtype::I32);

    func.set_offset_and_size("iter_fatptr", "iter_offset", "iter_size");

    // pass values to constructor
    // current is initialized to 0 (this is arbitrary, since this value is never read)
    func.write_opcode(numtype.const_op());
    match numtype {
        Numtype::F32 => func.write_slice(&[0x00, 0x00, 0x00, 0x00]),
        _ => func.write_byte(0x00),
    };
    // advance_fn
    func.write_opcode(Opcode::I32Const);
    func.write_slice(&unsigned_leb128(advance_fn_table_idx));
    // inner_offset
    func.write_opcode(Opcode::LocalGet);
    func.write_var("iter_offset");
    // max_inner_offset
    func.write_opcode(Opcode::LocalGet);
    func.write_var("iter_offset");
    func.write_opcode(Opcode::LocalGet);
    func.write_var("iter_size");
    func.write_opcode(Opcode::I32Add);

    func.write_opcode(Opcode::Call);
    func.write_slice(&unsigned_leb128(constructor_idx));

    func.write_opcode(Opcode::End);

    func
}

pub fn define_builtin_iter_collect(numtype: Numtype, alloc_idx: u32, advance_fn_type_idx: u32) -> BuiltinFunc {
    let mut func = BuiltinFunc::new(
        // takes an Iter and returns an Arr
        FuncTypeSignature::new(vec![Numtype::I64], Some(Numtype::I64)),
        vec!["iter_fatptr".to_string()],
    );
    func.add_local("iter_offset", Numtype::I32);
    func.add_local("array_offset", Numtype::I32);
    func.add_local("element_offset", Numtype::I32);
    func.add_local("alloc_size", Numtype::I32);
    func.add_local("current", numtype);
    func.add_local("array_size", Numtype::I32);
    func.add_local("new_array_offset", Numtype::I32);

    let memsize = numtype.size();
    let load_op = numtype.load_op();
    let store_op = numtype.store_op();

    // iter_offset = iter_fatpr >> 32
    func.write_opcode(Opcode::LocalGet);
    func.write_var("iter_fatptr");
    func.write_opcode(Opcode::I64Const);
    func.write_byte(0x20);
    func.write_opcode(Opcode::I64ShrU);
    func.write_opcode(Opcode::I32WrapI64);
    func.write_opcode(Opcode::LocalSet);
    func.write_var("iter_offset");

    // start by allocating space for 2 elements
    func.write_opcode(Opcode::I32Const);
    func.write_slice(&unsigned_leb128(2 * memsize));
    func.write_opcode(Opcode::LocalTee);
    func.write_var("alloc_size");
    func.write_opcode(Opcode::Call);
    func.write_slice(&unsigned_leb128(alloc_idx));
    func.write_opcode(Opcode::LocalTee);
    func.write_var("array_offset");
    func.write_opcode(Opcode::LocalSet);
    func.write_var("element_offset");

    // loop:
    // if iterator->advance():
    //   *element_offset = iter_offset->current
    //   element_offset += memsize
    //   if element_offset >= memptr, allocate more space
    //   branch to loop

    func.write_opcode(Opcode::Loop);
    func.write_byte(Numtype::Void as u8);

    // call iterator->advance
    func.iter_call_advance("iter_offset", memsize, advance_fn_type_idx);

    // branch if result != 1
    func.write_opcode(Opcode::If);
    func.write_byte(Numtype::I32 as u8);

    // case if done == 1
    func.write_opcode(Opcode::I32Const);
    func.write_byte(0x00); // don't branch to loop (break out of loop)

    func.write_opcode(Opcode::Else);

    // case if done == 0
    // read current value
    func.write_opcode(Opcode::LocalGet);
    func.write_var("iter_offset");
    func.write_opcode(load_op);
    func.write_slice(&[0x02, 0x00]);
    func.write_opcode(Opcode::LocalSet);
    func.write_var("current");

    // store current at element_offset
    func.write_opcode(Opcode::LocalGet);
    func.write_var("element_offset");
    func.write_opcode(Opcode::LocalGet);
    func.write_var("current");
    func.write_opcode(store_op);
    func.write_slice(&[0x02, 0x00]);

    // add memsize to element_offset
    func.write_opcode(Opcode::LocalGet);
    func.write_var("element_offset");
    func.write_opcode(Opcode::I32Const);
    func.write_slice(&unsigned_leb128(memsize));
    func.write_opcode(Opcode::I32Add);
    func.write_opcode(Opcode::LocalTee);
    func.write_var("element_offset");

    // if element_offset >= array_offset + alloc_size, allocate more space
    func.write_opcode(Opcode::LocalGet);
    func.write_var("array_offset");
    func.write_opcode(Opcode::LocalGet);
    func.write_var("alloc_size");
    func.write_opcode(Opcode::I32Add);
    func.write_opcode(Opcode::I32GeU);
    func.write_opcode(Opcode::If);
    func.write_byte(Numtype::Void as u8);

    // double alloc size and allocate new space
    func.write_opcode(Opcode::LocalGet);
    func.write_var("alloc_size");
    func.write_opcode(Opcode::I32Const);
    func.write_byte(2);
    func.write_opcode(Opcode::I32Mul);
    func.write_opcode(Opcode::LocalTee);
    func.write_var("alloc_size");
    func.write_opcode(Opcode::Call);
    func.write_slice(&unsigned_leb128(alloc_idx));
    func.write_opcode(Opcode::LocalTee);
    func.write_var("new_array_offset");

    // copy old array to new spot
    // destination is new_array_offset
    // source address is `array_offset` (old array offset)
    func.write_opcode(Opcode::LocalGet);
    func.write_var("array_offset");
    // amount to copy is array size (equal to element_offset - array_offset)
    func.write_opcode(Opcode::LocalGet);
    func.write_var("element_offset");
    func.write_opcode(Opcode::LocalGet);
    func.write_var("array_offset");
    func.write_opcode(Opcode::I32Sub);
    func.write_slice(&MEMCOPY);

    // set element_offset to new_array_offset + (element_offset - array_offset)
    func.write_opcode(Opcode::LocalGet);
    func.write_var("new_array_offset");
    func.write_opcode(Opcode::LocalGet);
    func.write_var("element_offset");
    func.write_opcode(Opcode::LocalGet);
    func.write_var("array_offset");
    func.write_opcode(Opcode::I32Sub);
    func.write_opcode(Opcode::I32Add);
    func.write_opcode(Opcode::LocalSet);
    func.write_var("element_offset");

    // set array_offset to new_array_offset
    func.write_opcode(Opcode::LocalGet);
    func.write_var("new_array_offset");
    func.write_opcode(Opcode::LocalSet);
    func.write_var("array_offset");

    func.write_opcode(Opcode::End); // end if

    func.write_opcode(Opcode::I32Const);
    func.write_byte(0x01); // branch to loop

    func.write_opcode(Opcode::End); // end if

    func.write_opcode(Opcode::BrIf);
    func.write_byte(0x00);

    func.write_opcode(Opcode::End); // end loop

    // return [array_offset, element_offset - array_offset]
    func.write_opcode(Opcode::LocalGet);
    func.write_var("element_offset");
    func.write_opcode(Opcode::LocalGet);
    func.write_var("array_offset");
    func.write_opcode(Opcode::I32Sub);
    func.write_opcode(Opcode::LocalSet);
    func.write_var("element_offset"); // use element offset again to store size
    func.create_fatptr("array_offset", "element_offset");

    func.write_opcode(Opcode::End); // end function

    func
}

pub fn define_builtin_iter_last(
    iter_type: Numtype,
    advance_fn_type_idx: u32,
) -> BuiltinFunc {
    let mut func = BuiltinFunc::new(
        // takes an Iter([type]) and returns a value of type [type]
        FuncTypeSignature::new(vec![Numtype::I64], Some(iter_type)),
        vec!["iter_fatptr".to_string()],
    );
    func.add_local("iter_offset", Numtype::I32);

    // iter_offset = iter_fatptr >> 32
    func.write_opcode(Opcode::LocalGet);
    func.write_var("iter_fatptr");
    func.write_opcode(Opcode::I64Const);
    func.write_byte(0x20);
    func.write_opcode(Opcode::I64ShrU);
    func.write_opcode(Opcode::I32WrapI64);
    func.write_opcode(Opcode::LocalSet);
    func.write_var("iter_offset");

    // loop:
    // if iterator->advance() == 1:
    //   return iterator->current
    // else:
    //   goto loop

    func.write_opcode(Opcode::Loop);
    func.write_byte(iter_type as u8);

    // call iterator->advance
    func.write_opcode(Opcode::LocalGet);
    func.write_var("iter_offset");
    func.write_opcode(Opcode::LocalGet); // iter offset
    func.write_var("iter_offset");
    func.write_opcode(Opcode::I32Const);
    func.write_slice(&unsigned_leb128(iter_type.size()));
    func.write_opcode(Opcode::I32Add);
    func.write_opcode(Opcode::I32Load);
    func.write_slice(&[0x02, 0x00]); // advance fn
    func.write_opcode(Opcode::CallIndirect);
    func.write_slice(&unsigned_leb128(advance_fn_type_idx)); // signature index
    func.write_byte(0x00); // table index

    // branch (continue to next loop iteration) if advance() == 0
    func.write_opcode(Opcode::I32Eqz);
    func.write_opcode(Opcode::BrIf);
    func.write_byte(0x00);

    // otherwise, get iterator->current (which will be returned)
    func.write_opcode(Opcode::LocalGet);
    func.write_var("iter_offset");
    func.write_opcode(iter_type.load_op());
    func.write_slice(&[0x02, 0x00]);

    func.write_opcode(Opcode::End); // end loop

    func.write_opcode(Opcode::End); // end function

    func
}
