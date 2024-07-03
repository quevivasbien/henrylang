use lazy_static::lazy_static;
use rustc_hash::FxHashMap;

use super::wasmtypes::*;

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
    pub fn call_advance_on_inner(
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
