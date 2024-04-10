use std::rc::Rc;
use byteorder::{BigEndian, ByteOrder, WriteBytesExt};

use crate::compiler;
use crate::Value;

#[derive(Debug)]
#[repr(u8)]
pub enum OpCode {
    Return,
    
    True,
    False,
    
    // Comparisons
    Equal,
    NotEqual,
    Greater,
    GreaterEqual,
    Less,
    LessEqual,
    
    // Binary operations
    Add,
    Subtract,
    Multiply,
    Divide,
    And,
    Or,
    To,
    
    // Unary operations
    Negate,
    Not,

    EndExpr,
    EndBlock,
    Jump,
    JumpIfFalse,
    Call,
    Array,
    Map,
    
    Constant,
    SetGlobal,
    GetGlobal,
    SetLocal,
    GetLocal,
    Closure,
    GetUpvalue,

    WrapSome,
}

impl From<u8> for OpCode {
    fn from(value: u8) -> Self {
        unsafe { std::mem::transmute(value) }
    }
}

pub struct Chunk {
    bytes: Vec<u8>,
    constants: Vec<Value>,
    newlines: Vec<usize>,
}

impl Chunk {
    pub fn new() -> Self {
        Self {
            bytes: Vec::new(),
            constants: Vec::new(),
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

    pub fn write_closure(&mut self, value: Value, upvalues: Vec<compiler::Upvalue>, line: usize) -> Result<(), &'static str> {
        match &value {
            Value::Closure(func) => func.clone(),
            _ => return Err("Value is not a closure"),
        };
        let idx = self.create_constant(value)?;
        self.write_opcode(OpCode::Closure, line);
        self.bytes.write_u16::<BigEndian>(idx).map_err(|_| "Failed to write index of closure to bytes")?;
        for upvalue in upvalues.iter() {
            self.bytes.write_u8(
                if upvalue.is_local { 1 } else { 0 }
            ).map_err(|_| "Failed to write upvalue locality to bytes")?;
            self.bytes.write_u16::<BigEndian>(upvalue.index).map_err(|_| "Failed to write index of upvalue to bytes")?;
        }
        Ok(())
    }

    pub fn write_set_global(&mut self, idx: u16, line: usize) -> Result<(), &'static str> {
        self.write_opcode(OpCode::SetGlobal, line);
        self.bytes.write_u16::<BigEndian>(idx).map_err(|_| "Failed to write index of global variable to bytes")
    }
    pub fn write_get_global(&mut self, name: String, line: usize) -> Result<(), &'static str> {
        let idx = self.create_constant(Value::String(Rc::new(name)))?;
        self.write_opcode(OpCode::GetGlobal, line);
        self.bytes.write_u16::<BigEndian>(idx).map_err(|_| "Failed to write index of global variable name to bytes")
    }

    pub fn write_get_local(&mut self, idx: u16, line: usize) -> Result<(), &'static str> {
        self.write_opcode(OpCode::GetLocal, line);
        self.bytes.write_u16::<BigEndian>(idx).map_err(|_| "Failed to write index of local variable to bytes")
    }

    pub fn write_get_upvalue(&mut self, idx: u16, line: usize) -> Result<(), &'static str> {
        self.write_opcode(OpCode::GetUpvalue, line);
        self.bytes.write_u16::<BigEndian>(idx).map_err(|_| "Failed to write index of upvalue to bytes")
    }

    pub fn write_endblock(&mut self, n_pops: u16, line: usize) -> Result<(), &'static str> {
        self.write_opcode(OpCode::EndBlock, line);
        self.bytes.write_u16::<BigEndian>(n_pops).map_err(|_| "Failed to write number of pops to bytes")
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
    pub fn read_constant(&self, ip: &mut usize) -> &Value {
        let index = self.read_u16(ip);
        &self.constants[index as usize]
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
    pub fn disassemble_instruction(&self, ip: &mut usize) -> bool {
        if *ip >= self.bytes.len() {
            return true;
        }
        let ip0 = *ip;
        let opcode = OpCode::from(self.read_u8(ip));
        match opcode {
            OpCode::Return => {
                println!("{:04} RETURN", ip0);
            },
            OpCode::True => {
                println!("{:04} TRUE", ip0);
            },
            OpCode::False => {
                println!("{:04} FALSE", ip0);
            },
            OpCode::Equal => {
                println!("{:04} EQUAL", ip0);
            },
            OpCode::NotEqual => {
                println!("{:04} NOTEQUAL", ip0);
            },
            OpCode::Greater => {
                println!("{:04} GREATER", ip0);
            },
            OpCode::GreaterEqual => {
                println!("{:04} GREATEREQUAL", ip0);
            },
            OpCode::Less => {
                println!("{:04} LESS", ip0);
            },
            OpCode::LessEqual => {
                println!("{:04} LESSEQUAL", ip0);
            },
            OpCode::Add => {
                println!("{:04} ADD", ip0);
            },
            OpCode::Subtract => {
                println!("{:04} SUBTRACT", ip0);
            },
            OpCode::Multiply => {
                println!("{:04} MULTIPLY", ip0);
            },
            OpCode::Divide => {
                println!("{:04} DIVIDE", ip0);
            },
            OpCode::And => {
                println!("{:04} AND", ip0);
            },
            OpCode::Or => {
                println!("{:04} OR", ip0);
            },
            OpCode::To => {
                println!("{:04} TO", ip0);
            },
            OpCode::Negate => {
                println!("{:04} NEGATE", ip0);
            },
            OpCode::Not => {
                println!("{:04} NOT", ip0);
            },
            OpCode::EndExpr => {
                println!("{:04} ENDEXPR", ip0);
            },
            OpCode::EndBlock => {
                let n_pops = self.read_u16(ip);
                println!("{:04} ENDBLOCK {}", ip0, n_pops);
            },
            OpCode::Jump => {
                let offset = self.read_u16(ip);
                println!("{:04} JUMP {}", ip0, offset);
            },
            OpCode::JumpIfFalse => {
                let offset = self.read_u16(ip);
                println!("{:04} JUMPIFFALSE {}", ip0, offset);
            },
            OpCode::Call => {
                let arg_count = self.read_u8(ip);
                println!("{:04} CALL {}", ip0, arg_count);
            },
            OpCode::Array => {
                let num_elems = self.read_u16(ip);
                println!("{:04} ARRAY {}", ip0, num_elems);  
            },
            OpCode::Map => {
                println!("{:04} MAP", ip0);
            },
            OpCode::Constant => {
                let constant = self.read_constant(ip);
                println!("{:04} CONSTANT {:?}", ip0, constant);
            },
            OpCode::SetGlobal => {
                let name = self.read_constant(ip);
                println!("{:04} SETGLOBAL {:?}", ip0, name);
            },
            OpCode::GetGlobal => {
                let name = self.read_constant(ip);
                println!("{:04} GETGLOBAL {:?}", ip0, name);
            },
            OpCode::SetLocal => {
                println!("{:04} SETLOCAL", ip0);
            },
            OpCode::GetLocal => {
                let idx = self.read_u16(ip);
                println!("{:04} GETLOCAL {}", ip0, idx);
            },
            OpCode::Closure => {
                println!("{:04} CLOSURE", ip0);
                let closure = match self.read_constant(ip) {
                    Value::Closure(f) => f,
                    _ => unreachable!(),
                };
                for _ in 0..closure.function.num_upvalues {
                    let is_local = self.read_u8(ip) == 1;
                    let index = self.read_u16(ip);
                    println!("{:04} | UPVALUE {} {}", *ip - 2, is_local, index);
                }
            },
            OpCode::GetUpvalue => {
                let idx = self.read_u16(ip);
                println!("{:04} GETUPVALUE {}", ip0, idx);
            },
            OpCode::WrapSome => {
                println!("{:04} SOME", ip0);
            },
        }
        return false
    }

    #[cfg(feature = "debug")]
    pub fn disassemble(&self) {
        let mut ip = 0;
        loop {
            if self.disassemble_instruction(&mut ip) {
                break;
            }
        }
    }
}
