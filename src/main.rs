use stdio::Cursor;
use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};

mod values;

#[derive(Debug)]
#[repr(u8)]
enum OpCode {
    Return,
    Constant,
}

impl From<u8> for OpCode {
    fn from(value: u8) -> Self {
        unsafe { std::mem::transmute(value) }
    }
}

#[derive(Debug)]
enum InterpreterError {
    CompileError,
    RuntimeError,
}

struct Chunk {
    name: String,
    bytes: Vec<u8>,
    constants: Vec<values::Value>,
}

impl Chunk {
    fn new(name: String) -> Self {
        Self {
            name,
            bytes: Vec::new(),
            constants: Vec::new(),
        }
    }

    fn write_opcode(&mut self, opcode: OpCode) {
        self.bytes.write_u8(opcode as u8).unwrap();
    }
    fn add_constant(&mut self, value: values::Value) -> u16 {
        self.constants.push(value);
        (self.constants.len() - 1) as u16
    }
    fn write_constant(&mut self, idx: u16) {
        self.write_opcode(OpCode::Constant);
        self.bytes.write_u16::<BigEndian>(idx).unwrap()
    }

    fn read_constant(&self, cursor: &mut Cursor<&[u8]>) -> values::Value {
        let index = cursor.read_u16::<BigEndian>().unwrap();
        self.constants[index as usize]
    }

    fn disassemble(&self) {
        println!("== {} ==", self.name);
        let mut cursor = Cursor::new(self.bytes.as_slice());
        loop {
            let pos = cursor.position();
            let opcode = match cursor.read_u8() {
                Ok(x) => OpCode::from(x),
                Err(_) => break,
            };
            match opcode {
                OpCode::Return => {
                    println!("{:04} RETURN", pos);
                },
                OpCode::Constant => {
                    let constant = self.read_constant(&mut cursor);
                    println!("{:04} CONSTANT {}", pos, constant);
                }
            }
        }
    }

    fn interpret(&self) -> Result<(), InterpreterError> {
        let mut cursor = Cursor::new(&self.bytes);
        loop {
            let opcode = match cursor.read_u8() {
                Ok(x) => OpCode::from(x),
                Err(_) => return Err(InterpreterError::CompileError),
            };
            match opcode {
                OpCode::Return => {
                    return Ok(());
                },
                _ => {},
            }
        }
    }
}

fn main() {
    let mut chunk = Chunk::new("Test Chunk".to_string());
    let idx = chunk.add_constant(1.0);
    chunk.write_constant(idx);
    let idx = chunk.add_constant(3.0);
    chunk.write_constant(idx);
    chunk.write_opcode(OpCode::Return);

    chunk.disassemble();

    chunk.interpret().unwrap();
}
