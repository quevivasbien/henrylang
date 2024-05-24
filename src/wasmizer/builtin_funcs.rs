use lazy_static::lazy_static;
use rustc_hash::FxHashMap;

use super::wasmtypes::*;

struct LocalVar {
    name: &'static str,
    numtype: Numtype,
}

pub struct BuiltinFunc {
    signature: FuncTypeSignature,
    params: Vec<LocalVar>,
    locals: Vec<LocalVar>,

    bytes: Vec<u8>,
}

impl BuiltinFunc {
    fn new(signature: FuncTypeSignature, param_names: Vec<&'static str>) -> Self {
        assert_eq!(param_names.len(), signature.args.len());
        let params = param_names.into_iter().zip(signature.args.iter()).map(
            |(name, &numtype)| LocalVar { name, numtype }
        ).collect();
        Self {
            signature,
            params,
            locals: vec![],
            bytes: vec![],
        }
    }

    fn get_var_idx(&self, name: &str) -> Option<Vec<u8>> {
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

    fn add_local(&mut self, name: &'static str, numtype: Numtype) {
        assert_eq!(self.get_var_idx(&name), None);
        self.locals.push(LocalVar { name, numtype });
    }

    fn write_opcode(&mut self, opcode: Opcode) {
        self.bytes.push(opcode as u8);
    }
    fn write_byte(&mut self, byte: u8) {
        self.bytes.push(byte);
    }
    fn write_slice(&mut self, bytes: &[u8]) {
        self.bytes.extend_from_slice(bytes);
    }
    fn write_var(&mut self, name: &str) {
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

    // sets offset = fatptr >> 32 and size = fatptr & 0xFFFFFFFF
    fn set_offset_and_size(&mut self, fatptr_name: &str, offset_name: &str, size_name: &str) {
        self.write_opcode(Opcode::LocalGet);
        self.write_var(fatptr_name);
        self.write_opcode(Opcode::I64Const);
        self.write_byte(0x20);
        self.write_opcode(Opcode::I64ShrU);  // shift right 32 bits
        self.write_opcode(Opcode::I32WrapI64);  // discard high 32 bits
        self.write_opcode(Opcode::LocalSet);
        self.write_var(offset_name);  // set as offset
        // size = lowest 32 bits of fatptr
        self.write_opcode(Opcode::LocalGet);
        self.write_var(fatptr_name);
        self.write_opcode(Opcode::I32WrapI64);  // discard high 32 bits
        self.write_opcode(Opcode::LocalSet);
        self.write_var(size_name);  // set as size
    }

    // copy size bytes from offset to memptr
    // sets offset to value of memptr, then increments memptr by size
    // if write_size is true, write the size of the allocation at the end of the block
    fn copy_mem(&mut self, offset_name: &str, size_name: &str, write_size: bool) {
        // destination is current value of memptr + 4 (+ 4 so we don't overwrite last chunk's size)
        self.write_opcode(Opcode::GlobalGet);
        self.write_byte(0x00);
        self.write_opcode(Opcode::I32Const);
        self.write_byte(0x04);
        self.write_opcode(Opcode::I32Add);
        // source address is `offset`
        self.write_opcode(Opcode::LocalGet);
        self.write_var(offset_name);
        // copy `size` bytes
        self.write_opcode(Opcode::LocalGet);
        self.write_var(size_name);
        self.write_slice(&MEMCOPY);

        // set offset to memptr + 4 (start of new memory block)
        self.write_opcode(Opcode::GlobalGet);
        self.write_byte(0x00);
        self.write_opcode(Opcode::I32Const);
        self.write_byte(0x04);
        self.write_opcode(Opcode::I32Add);
        self.write_opcode(Opcode::LocalSet);
        self.write_var(offset_name);

        // set memptr to memptr + size (+4 if also writing size)
        self.write_opcode(Opcode::GlobalGet);
        self.write_byte(0x00);
        self.write_opcode(Opcode::LocalGet);
        self.write_var(size_name);
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
            // make sure memptr is aligned
            self.align_memptr();

            // destination is memptr
            self.write_opcode(Opcode::GlobalGet);
            self.write_byte(0x00);
            // size is memptr - offset
            self.write_opcode(Opcode::GlobalGet);
            self.write_byte(0x00);
            self.write_opcode(Opcode::LocalGet);
            self.write_var(offset_name);
            self.write_opcode(Opcode::I32Sub);

            self.write_opcode(Opcode::I32Store);
            self.write_byte(0x02);  // alignment
            self.write_byte(0x00);  // store offset
        }
    }
}

lazy_static! {
    pub static ref BUILTINS: FxHashMap<&'static str, BuiltinFunc> = {
        let mut map = FxHashMap::default();

        // TODO: allow this to grow to more pages and raise error if out of memory
        let alloc = {
            let mut func = BuiltinFunc::new(
                FuncTypeSignature::new(vec![Numtype::I32], Some(Numtype::I32)),
                vec!["size"]
            );
    
            func.add_local("offset", Numtype::I32);
    
            // get memptr + 4 (start of next memory chunk)
            func.write_opcode(Opcode::GlobalGet);
            func.write_byte(0x00);
            func.write_opcode(Opcode::I32Const);
            func.write_byte(0x04);
            func.write_opcode(Opcode::I32Add);
            func.write_opcode(Opcode::LocalSet);
            func.write_var("offset");  // save_location as value to return
    
            // set memptr to end of new memory chunk (memptr = memptr + size + 4)
            func.write_opcode(Opcode::GlobalGet);
            func.write_byte(0x00);
            func.write_opcode(Opcode::LocalGet);
            func.write_var("size");
            func.write_opcode(Opcode::I32Const);
            func.write_byte(0x04);
            func.write_opcode(Opcode::I32Add);
            func.write_opcode(Opcode::I32Add);
            func.write_opcode(Opcode::GlobalSet);
            func.write_byte(0x00);
    
            // write size of allocation at end of block
            func.write_opcode(Opcode::GlobalGet);
            func.write_byte(0x00);
            func.write_opcode(Opcode::LocalGet);
            func.write_var("size");
            func.write_opcode(Opcode::I32Store);
            func.write_byte(0x02);  // alignment
            func.write_byte(0x00);  // store offset
    
            // return start of the new memory chunk
            func.write_opcode(Opcode::LocalGet);
            func.write_var("offset");
    
            func.write_opcode(Opcode::End);
    
            func
        };
        map.insert("alloc", alloc);
    
        let free = {
            let mut func = BuiltinFunc::new(
                FuncTypeSignature::default(),
                vec![]
            );
    
            // memptr = memptr - (*memptr + 4)
            // get memptr
            func.write_opcode(Opcode::GlobalGet);
            func.write_byte(0x00);
            // load *memptr
            func.write_opcode(Opcode::GlobalGet);
            func.write_byte(0x00);
            func.write_opcode(Opcode::I32Load);
            func.write_byte(0x02);  // alignment
            func.write_byte(0x00);  // load offset
            // + 4
            func.write_opcode(Opcode::I32Const);
            func.write_byte(0x04);
            func.write_opcode(Opcode::I32Add);
            // - (*memptr + 4)
            func.write_opcode(Opcode::I32Sub);
            // set memptr
            func.write_opcode(Opcode::GlobalSet);
            func.write_byte(0x00);
    
            func.write_opcode(Opcode::End);
    
            func
        };
        map.insert("free", free);

        let copy_heap_obj = {
            let mut func = BuiltinFunc::new(
                FuncTypeSignature::new(vec![Numtype::I64], Some(Numtype::I64)),
                vec!["fatptr"]
            );

            func.add_local("offset", Numtype::I32);
            func.add_local("size", Numtype::I32);

            func.set_offset_and_size("fatptr", "offset", "size");
            func.copy_mem("offset", "size", true);

            // return [offset, size]
            func.write_opcode(Opcode::LocalGet);
            func.write_var("offset");
            func.write_opcode(Opcode::I64ExtendI32U);
            func.write_opcode(Opcode::I64Const);
            func.write_byte(0x20);
            func.write_opcode(Opcode::I64Shl);
            func.write_opcode(Opcode::LocalGet);
            func.write_var("size");
            func.write_opcode(Opcode::I64ExtendI32U);
            func.write_opcode(Opcode::I64Add);

            func.write_opcode(Opcode::End);

            func
        };
        map.insert("copy_heap_obj", copy_heap_obj);

        let concat_heap_objs = {
            let mut func = BuiltinFunc::new(
                FuncTypeSignature::new(vec![Numtype::I64, Numtype::I64], Some(Numtype::I64)),
                vec!["fatptr1", "fatptr2"]
            );

            func.add_local("offset1", Numtype::I32);
            func.add_local("size1", Numtype::I32);
            func.add_local("offset2", Numtype::I32);
            func.add_local("size2", Numtype::I32);

            func.set_offset_and_size("fatptr1", "offset1", "size1");
            func.set_offset_and_size("fatptr2", "offset2", "size2");

            func.copy_mem("offset1", "size1", false);
            func.copy_mem("offset2", "size2", false);

            func.align_memptr();

            // write combined size at end
            // first, increment memptr by 4
            func.write_opcode(Opcode::GlobalGet);
            func.write_byte(0x00);
            func.write_opcode(Opcode::I32Const);
            func.write_byte(0x04);
            func.write_opcode(Opcode::I32Add);
            func.write_opcode(Opcode::GlobalSet);
            func.write_byte(0x00);
            // then write combined size
            func.write_opcode(Opcode::GlobalGet); // destination is memptr
            func.write_byte(0x00);
            func.write_opcode(Opcode::GlobalGet); // size is memptr - offset
            func.write_byte(0x00);
            func.write_opcode(Opcode::LocalGet);
            func.write_var("offset1");
            func.write_opcode(Opcode::I32Sub);
            func.write_opcode(Opcode::I32Store);
            func.write_byte(0x02);  // alignment
            func.write_byte(0x00);  // store offset

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
        map.insert("concat_heap_objs", concat_heap_objs);

        map
    };
}