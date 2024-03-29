use stdio::Cursor;
use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};

use crate::Value;

#[derive(Debug)]
#[repr(u8)]
pub enum OpCode {
    Return,
    Constant,
    Add,
    Subtract,
    Multiply,
    Divide,
    Negate,
}

impl From<u8> for OpCode {
    fn from(value: u8) -> Self {
        unsafe { std::mem::transmute(value) }
    }
}

pub struct Chunk {
    pub name: String,
    bytes: Vec<u8>,
    constants: Vec<Value>,
    newlines: Vec<usize>,
}

impl Chunk {
    pub fn new(name: String) -> Self {
        Self {
            name,
            bytes: Vec::new(),
            constants: Vec::new(),
            newlines: Vec::new(),
        }
    }

    pub fn write_opcode(&mut self, opcode: OpCode) {
        self.bytes.write_u8(opcode as u8).unwrap();
    }
    pub fn add_constant(&mut self, value: Value) -> u16 {
        self.constants.push(value);
        (self.constants.len() - 1) as u16
    }
    pub fn write_constant(&mut self, idx: u16) {
        self.write_opcode(OpCode::Constant);
        self.bytes.write_u16::<BigEndian>(idx).unwrap()
    }

    pub fn read_constant(&self, cursor: &mut Cursor<&[u8]>) -> Value {
        let index = cursor.read_u16::<BigEndian>().unwrap();
        self.constants[index as usize]
    }

    pub fn cursor(&self) -> Cursor<&[u8]> {
        Cursor::new(&self.bytes.as_slice())
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

    pub fn disassemble_instruction(&self, cursor: &mut Cursor<&[u8]>) -> bool {
        let pos = cursor.position();
            let opcode = match cursor.read_u8() {
                Ok(x) => OpCode::from(x),
                Err(_) => return true,
            };
            match opcode {
                OpCode::Return => {
                    println!("{:04} RETURN", pos);
                },
                OpCode::Constant => {
                    let constant = self.read_constant(cursor);
                    println!("{:04} CONSTANT {}", pos, constant);
                },
                OpCode::Add => {
                    println!("{:04} ADD", pos);
                },
                OpCode::Subtract => {
                    println!("{:04} SUBTRACT", pos);
                },
                OpCode::Multiply => {
                    println!("{:04} MULTIPLY", pos);
                },
                OpCode::Divide => {
                    println!("{:04} DIVIDE", pos);
                },
                OpCode::Negate => {
                    println!("{:04} NEGATE", pos);
                },
            }
            return false
    }

    pub fn disassemble(&self) {
        println!("== {} ==", self.name);
        let mut cursor = self.cursor();
        loop {
            if self.disassemble_instruction(&mut cursor) {
                break;
            }
        }
    }
}
