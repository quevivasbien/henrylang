use std::rc::Rc;

use byteorder::{BigEndian, ByteOrder, WriteBytesExt};

use crate::compiler;
use crate::values::{Closure, HeapValue, Value};

#[derive(Debug, PartialEq)]
#[repr(u8)]
pub enum OpCode {
    Return,
    ReturnHeap,
    
    // Constants
    True,
    False,
    Constant,
    HeapConstant,
    
    // Comparisons
    IntEqual,
    IntNotEqual,
    IntGreater,
    IntGreaterEqual,
    IntLess,
    IntLessEqual,

    FloatEqual,
    FloatNotEqual,
    FloatGreater,
    FloatGreaterEqual,
    FloatLess,
    FloatLessEqual,
    
    BoolEqual,
    BoolNotEqual,

    HeapEqual,
    HeapNotEqual,
    
    // Binary operations
    IntAdd,
    IntSubtract,
    IntMultiply,
    IntDivide,

    FloatAdd,
    FloatSubtract,
    FloatMultiply,
    FloatDivide,

    Concat,
    
    And,
    Or,
    
    To,
    
    // Unary operations
    IntNegate,
    FloatNegate,
    Not,

    EndExpr,
    EndHeapExpr,
    EndBlock,
    EndHeapBlock,
    Jump,
    JumpIfFalse,
    Call,
    Array,
    ArrayHeap,

    SetGlobal,
    SetHeapGlobal,
    GetGlobal,
    GetHeapGlobal,
    SetLocal,
    SetHeapLocal,
    GetLocal,
    GetHeapLocal,
    
    Closure,
    GetUpvalue,
    GetHeapUpvalue,

    WrapSome,
    WrapHeapSome,

    Map,
    Reduce,
    HeapReduce,
}

impl From<u8> for OpCode {
    fn from(value: u8) -> Self {
        unsafe { std::mem::transmute(value) }
    }
}

pub struct Chunk {
    bytes: Vec<u8>,
    // for storing 64-bit values
    constants: Vec<Value>,
    // for storing larger values (strings, arrays, etc.)
    heap_constants: Vec<HeapValue>,
    newlines: Vec<usize>,
}

impl Chunk {
    pub fn new() -> Self {
        Self {
            bytes: Vec::new(),
            constants: Vec::new(),
            heap_constants: Vec::new(),
            newlines: Vec::new(),
        }
    }

    pub fn len(&self) -> usize {
        self.bytes.len()
    }

    fn sync_line(&mut self, line: usize) {
        let current_line = self.newlines.len() + 1;
        if line > current_line {
            for _ in current_line..line {
                self.newlines.push(self.bytes.len());
            }
        }
    }

    pub fn write_opcode(&mut self, opcode: OpCode, line: usize) {
        self.bytes.write_u8(opcode as u8).unwrap();
        self.sync_line(line);
    }


    pub fn create_constant(&mut self, value: Value) -> Result<u16, &'static str> {
        self.constants.push(value);
        let idx = self.constants.len() - 1;
        if idx > u16::MAX as usize {
            return Err("Too many constants in one chunk");
        }
        return Ok(idx as u16);
    }
    pub fn write_constant(&mut self, value: Value, line: usize) -> Result<(), &'static str> {
        let idx = self.create_constant(value)?;
        self.write_opcode(OpCode::Constant, line);
        self.bytes.write_u16::<BigEndian>(idx as u16).map_err(|_| "Failed to write index of constant to bytes")
    }

    pub fn create_heap_constant(&mut self, value: HeapValue) -> Result<u16, &'static str> {
        self.heap_constants.push(value);
        let idx = self.heap_constants.len() - 1;
        if idx > u16::MAX as usize {
            return Err("Too many constants in one chunk");
        }
        return Ok(idx as u16);
    }
    pub fn write_heap_constant(&mut self, value: HeapValue, line: usize) -> Result<(), &'static str> {
        let idx = self.create_heap_constant(value)?;
        self.write_opcode(OpCode::HeapConstant, line);
        self.bytes.write_u16::<BigEndian>(idx).map_err(|_| "Failed to write index of constant to bytes")
    }

    pub fn write_closure(
        &mut self,
        closure: Closure,
        upvalues: Vec<compiler::Upvalue>,
        heap_upvalues: Vec<compiler::Upvalue>,
        line: usize
    ) -> Result<(), &'static str> {
        let closure = HeapValue::Closure(Box::new(closure));
        let idx = self.create_heap_constant(closure)?;
        self.write_opcode(OpCode::Closure, line);
        self.bytes.write_u16::<BigEndian>(idx).map_err(|_| "Failed to write index of closure to bytes")?;
        for upvalue in upvalues.iter() {
            let locality = if upvalue.is_local { 1 } else { 0 };
            self.bytes.write_u8(locality ).map_err(|_| "Failed to write locality and heapness of upvalue to bytes")?;
            self.bytes.write_u16::<BigEndian>(upvalue.index).map_err(|_| "Failed to write index of upvalue to bytes")?;
        }
        for upvalue in heap_upvalues.iter() {
            let locality = if upvalue.is_local { 1 } else { 0 };
            self.bytes.write_u8(locality ).map_err(|_| "Failed to write locality and heapness of upvalue to bytes")?;
            self.bytes.write_u16::<BigEndian>(upvalue.index).map_err(|_| "Failed to write index of upvalue to bytes")?;
        }
        Ok(())
    }

    pub fn write_set_global(&mut self, idx: u16, is_heap: bool, line: usize) -> Result<(), &'static str> {
        self.write_opcode(
            if is_heap { OpCode::SetHeapGlobal } else { OpCode::SetGlobal },
            line
        );
        self.bytes.write_u16::<BigEndian>(idx).map_err(|_| "Failed to write index of global variable to bytes")
    }
    pub fn write_get_global(&mut self, name: String, is_heap: bool, line: usize) -> Result<(), &'static str> {
        let name = HeapValue::String(Rc::new(name));
        let idx = self.create_heap_constant(name)?;
        self.write_opcode(
            if is_heap { OpCode::GetHeapGlobal } else { OpCode::GetGlobal },
            line
        );
        self.bytes.write_u16::<BigEndian>(idx).map_err(|_| "Failed to write index of global variable name to bytes")
    }

    pub fn write_get_local(&mut self, idx: u16, is_heap: bool, line: usize) -> Result<(), &'static str> {
        self.write_opcode(
            if is_heap { OpCode::GetHeapLocal } else { OpCode::GetLocal },
            line
        );
        self.bytes.write_u16::<BigEndian>(idx).map_err(|_| "Failed to write index of local variable to bytes")
    }

    pub fn write_get_upvalue(&mut self, idx: u16, is_heap: bool, line: usize) -> Result<(), &'static str> {
        self.write_opcode(
            if is_heap { OpCode::GetHeapUpvalue } else { OpCode::GetUpvalue },
            line
        );
        self.bytes.write_u16::<BigEndian>(idx).map_err(|_| "Failed to write index of upvalue to bytes")
    }

    pub fn write_endblock(&mut self, n_pops: u16, n_heap_pops: u16, is_heap: bool, line: usize) -> Result<(), &'static str> {
        self.write_opcode(
            if is_heap { OpCode::EndHeapBlock } else { OpCode::EndBlock },
            line
        );
        self.bytes.write_u16::<BigEndian>(n_pops).map_err(|_| "Failed to write number of pops to bytes")?;
        self.bytes.write_u16::<BigEndian>(n_heap_pops).map_err(|_| "Failed to write number of heap pops to bytes")
    }
    
    pub fn write_jump(&mut self, opcode: OpCode, line: usize) -> Result<usize, &'static str> {
        match opcode {
            OpCode::Jump => (),
            OpCode::JumpIfFalse => (),
            _ => return Err("Invalid opcode for write_jump"),
        };
        self.write_opcode(opcode, line);
        self.bytes.write_u16::<BigEndian>(0).map_err(|_| "Failed to write jump offset placeholder to bytes")?;
        Ok(self.bytes.len()-2)
    }
    pub fn patch_jump(&mut self, idx: usize) -> Result<(), &'static str> {
        let jump = self.bytes.len() - idx - 2;
        if jump > u16::MAX as usize {
            return Err("Jump offset overflow");
        }
        // manually write the jump size at the offset index
        self.bytes[idx] = (jump >> 8) as u8;
        self.bytes[idx + 1] = jump as u8;
        Ok(())
    }

    pub fn write_array(&mut self, num_elems: u16, line: usize) -> Result<(), &'static str> {
        self.write_opcode(OpCode::Array, line);
        self.bytes.write_u16::<BigEndian>(num_elems).map_err(|_| "Failed to write number of elements to bytes")
    }
    pub fn write_array_array(&mut self, num_elems: u16, line: usize) -> Result<(), &'static str> {
        self.write_opcode(OpCode::ArrayHeap, line);
        self.bytes.write_u16::<BigEndian>(num_elems).map_err(|_| "Failed to write number of elements to bytes")
    }

    pub fn read_u8(&self, ip: &mut usize) -> u8 {
        let out = self.bytes[*ip];
        *ip += 1;
        out
    }
    pub fn read_u16(&self, ip: &mut usize) -> u16 {
        let out = BigEndian::read_u16(&self.bytes[*ip..*ip + 2]);
        *ip += 2;
        out
    }
    pub fn read_constant(&self, ip: &mut usize) -> Value {
        let index = self.read_u16(ip);
        self.constants[index as usize]
    }
    pub fn read_heap_constant(&self, ip: &mut usize) -> &HeapValue {
        let index = self.read_u16(ip);
        &self.heap_constants[index as usize]
    }

    // figures out line number for a given byte index
    pub fn line_num(&self, index: usize) -> usize {
        let mut line_count = 1;
        for &i in self.newlines.iter() {
            if i > index {
                return line_count;
            }
            line_count += 1;
        }
        line_count
    }

    #[cfg(feature = "debug")]
    pub fn disassemble_instruction(&self, ip: &mut usize) {
        let ip0 = *ip;
        let opcode = OpCode::from(self.read_u8(ip));
        match opcode {
            OpCode::Constant => {
                let constant = self.read_constant(ip);
                println!("{:04} Constant {:?}", ip0, constant);
            },
            OpCode::HeapConstant => {
                let constant = self.read_heap_constant(ip);
                println!("{:04} HeapConstant {:?}", ip0, constant);
            },

            OpCode::EndBlock => {
                let n_pops = self.read_u16(ip);
                let n_heap_pops = self.read_u16(ip);
                println!("{:04} EndBlock {:?} {:?}", ip0, n_pops, n_heap_pops);
            },
            OpCode::EndHeapBlock => {
                let n_pops = self.read_u16(ip);
                let n_heap_pops = self.read_u16(ip);
                println!("{:04} EndHeapBlock {:?} {:?}", ip0, n_pops, n_heap_pops);
            },
            OpCode::Jump => {
                let offset = self.read_u16(ip);
                println!("{:04} Jump {:?}", ip0, offset);
            },
            OpCode::JumpIfFalse => {
                let offset = self.read_u16(ip);
                println!("{:04} JumpIfFalse {:?}", ip0, offset);
            },

            OpCode::Array => {
                let num_elems = self.read_u16(ip);
                println!("{:04} Array {}", ip0, num_elems);
            },
            OpCode::ArrayHeap => {
                let num_elems = self.read_u16(ip);
                println!("{:04} ArrayHeap {}", ip0, num_elems);
            },

            OpCode::SetGlobal => {
                let name = match self.read_heap_constant(ip) {
                    HeapValue::String(s) => s.clone(),
                    _ => unreachable!(),
                };
                println!("{:04} SetGlobal {}", ip0, name);
            },
            OpCode::SetHeapGlobal => {
                let name = match self.read_heap_constant(ip) {
                    HeapValue::String(s) => s.clone(),
                    _ => unreachable!(),
                };
                println!("{:04} SetHeapGlobal {}", ip0, name);
            },
            OpCode::GetGlobal => {
                let name = match self.read_heap_constant(ip) {
                    HeapValue::String(s) => s.clone(),
                    _ => unreachable!(),
                };
                println!("{:04} GetGlobal {}", ip0, name);
            },
            OpCode::GetHeapGlobal => {
                let name = match self.read_heap_constant(ip) {
                    HeapValue::String(s) => s.clone(),
                    _ => unreachable!(),
                };
                println!("{:04} GetHeapGlobal {}", ip0, name);
            },
            OpCode::SetLocal => {
                let idx = self.read_u16(ip);
                println!("{:04} SetLocal {}", ip0, idx);
            },
            OpCode::SetHeapLocal => {
                let idx = self.read_u16(ip);
                println!("{:04} SetHeapLocal {}", ip0, idx);
            },
            OpCode::GetLocal => {
                let idx = self.read_u16(ip);
                println!("{:04} GetLocal {}", ip0, idx);
            },
            OpCode::GetHeapLocal => {
                let idx = self.read_u16(ip);
                println!("{:04} GetHeapLocal {}", ip0, idx);
            },
            
            OpCode::Closure => {
                let closure = match self.read_heap_constant(ip) {
                    HeapValue::Closure(c) => c,
                    _ => unreachable!(),
                };
                let n_upvalues = closure.function.num_upvalues;
                let n_heap_upvalues = closure.function.num_heap_upvalues;
                println!("{:04} Closure {} {}", ip0, n_upvalues, n_heap_upvalues);
                for _ in 0..(n_upvalues+n_heap_upvalues) {
                    let is_local = self.read_u8(ip) == 1;
                    let index = self.read_u16(ip);
                    println!(" | local: {}, idx: {}", is_local, index);
                }
            },

            x => println!("{:04} {:?}", ip0, x),
        }
    }

    #[cfg(feature = "debug")]
    pub fn disassemble(&self) {
        let mut ip = 0;
        while ip < self.bytes.len() {
            self.disassemble_instruction(&mut ip);
        }
    }
}
