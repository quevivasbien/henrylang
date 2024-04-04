use stdio::Cursor;
use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};

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
    
    // Unary operations
    Negate,
    Not,

    EndExpr,
    EndBlock,
    Jump,
    JumpIfFalse,
    
    Constant,
    SetGlobal,
    GetGlobal,
    SetLocal,
    GetLocal,
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

    pub fn write_set_global(&mut self, idx: u16, line: usize) -> Result<(), &'static str> {
        self.write_opcode(OpCode::SetGlobal, line);
        self.bytes.write_u16::<BigEndian>(idx).map_err(|_| "Failed to write index of global variable to bytes")
    }
    pub fn write_get_global(&mut self, name: String, line: usize) -> Result<(), &'static str> {
        let idx = self.create_constant(Value::String(name))?;
        self.write_opcode(OpCode::GetGlobal, line);
        self.bytes.write_u16::<BigEndian>(idx).map_err(|_| "Failed to write index of global variable name to bytes")
    }

    pub fn write_get_local(&mut self, idx: u16, line: usize) -> Result<(), &'static str> {
        self.write_opcode(OpCode::GetLocal, line);
        self.bytes.write_u16::<BigEndian>(idx).map_err(|_| "Failed to write index of local variable to bytes")
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

    pub fn read_u16(cursor: &mut Cursor<&[u8]>) -> u16 {
        cursor.read_u16::<BigEndian>().unwrap()
    }
    pub fn read_constant(&self, cursor: &mut Cursor<&[u8]>) -> &Value {
        let index = Self::read_u16(cursor);
        &self.constants[index as usize]
    }

    pub fn cursor(&self) -> Cursor<&[u8]> {
        Cursor::new(&self.bytes.as_slice())
    }

    // figures out line number for a given byte index
    pub fn _line_num(&self, index: usize) -> usize {
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
                OpCode::True => {
                    println!("{:04} TRUE", pos);
                },
                OpCode::False => {
                    println!("{:04} FALSE", pos);
                },
                OpCode::Equal => {
                    println!("{:04} EQUAL", pos);
                },
                OpCode::NotEqual => {
                    println!("{:04} NOTEQUAL", pos);
                },
                OpCode::Greater => {
                    println!("{:04} GREATER", pos);
                },
                OpCode::GreaterEqual => {
                    println!("{:04} GREATEREQUAL", pos);
                },
                OpCode::Less => {
                    println!("{:04} LESS", pos);
                },
                OpCode::LessEqual => {
                    println!("{:04} LESSEQUAL", pos);
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
                OpCode::And => {
                    println!("{:04} AND", pos);
                },
                OpCode::Or => {
                    println!("{:04} OR", pos);
                },
                OpCode::Negate => {
                    println!("{:04} NEGATE", pos);
                },
                OpCode::Not => {
                    println!("{:04} NOT", pos);
                },
                OpCode::EndExpr => {
                    println!("{:04} ENDEXPR", pos);
                },
                OpCode::EndBlock => {
                    let n_pops = Self::read_u16(cursor);
                    println!("{:04} ENDBLOCK {}", pos, n_pops);
                },
                OpCode::Jump => {
                    let offset = Self::read_u16(cursor);
                    println!("{:04} JUMP {}", pos, offset);
                }
                OpCode::JumpIfFalse => {
                    let offset = Self::read_u16(cursor);
                    println!("{:04} JUMPIFFALSE {}", pos, offset);
                }
                OpCode::Constant => {
                    let constant = self.read_constant(cursor);
                    println!("{:04} CONSTANT {:?}", pos, constant);
                },
                OpCode::SetGlobal => {
                    let name = self.read_constant(cursor);
                    println!("{:04} SETGLOBAL {:?}", pos, name);
                },
                OpCode::GetGlobal => {
                    let name = self.read_constant(cursor);
                    println!("{:04} GETGLOBAL {:?}", pos, name);
                },
                OpCode::SetLocal => {
                    println!("{:04} SETLOCAL", pos);
                },
                OpCode::GetLocal => {
                    let idx = Self::read_u16(cursor);
                    println!("{:04} GETLOCAL {}", pos, idx);
                },
            }
            return false
    }

    #[cfg(feature = "debug")]
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
