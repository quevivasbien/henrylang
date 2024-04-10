use std::rc::Rc;

use crate::ast::Type;
use crate::chunk::{Chunk, OpCode};
use crate::parser;
use crate::scanner;
use crate::values::{Closure, Function, Value};

struct Local {
    name: String,
    depth: i32,
}

struct LocalData {
    locals: Vec<Local>,
    scope_depth: i32,
}

impl Default for LocalData {
    fn default() -> Self {
        Self {
            locals: {
                let mut locals = Vec::new();
                locals.push(Local { name: "".to_string(), depth: 0 });
                locals
            },
            scope_depth: -1,
        }
    }
}

impl LocalData {
    fn push(&mut self, local: Local) -> Result<(), &'static str> {
        self.locals.push(local);
        if self.locals.len() == u16::MAX as usize {
            return Err("Too many locals declared in current function");
        }
        Ok(())
    }

    fn get_idx(&self, name: &str) -> Option<u16> {
        for (i, local) in self.locals.iter().enumerate().rev() {
            if local.name == name {
                return Some(i as u16);
            }
        }
        None
    }
}


#[derive(PartialEq, Clone, Copy, Debug)]
pub struct Upvalue {
    pub index: u16,
    pub is_local: bool,
}

pub struct Compiler {
    pub function: Function,
    locals: LocalData,
    upvalues: Vec<Upvalue>,
    parent: *mut Compiler,
}

impl Default for Compiler {
    fn default() -> Self {
        Self {
            function: Function::default(),
            locals: LocalData::default(),
            upvalues: Vec::new(),
            parent: std::ptr::null_mut(),
        }
    }
}

impl Compiler {
    pub fn new(parent: *mut Compiler) -> Self {
        Self { parent, ..Default::default() }
    }

    fn chunk(&mut self) -> &mut Chunk {
        &mut self.function.chunk
    }

    pub fn write_opcode(&mut self, opcode: OpCode) {
        self.chunk().write_opcode(opcode, 0);
    }
    pub fn write_constant(&mut self, value: Value) -> Result<(), String> {
        self.chunk().write_constant(value, 0).map_err(|e| e.to_string())
    }
    pub fn write_array(&mut self, len: u16) -> Result<(), String> {
        self.chunk().write_array(len, 0).map_err(|e| e.to_string())
    }
    pub fn write_jump(&mut self, opcode: OpCode) -> Result<usize, String> {
        self.chunk().write_jump(opcode, 0).map_err(|e| e.to_string())
    }
    pub fn patch_jump(&mut self, offset: usize) -> Result<(), String> {
        self.chunk().patch_jump(offset).map_err(|e| e.to_string())
    }
    pub fn write_call(&mut self) -> Result<(), String> {
        // todo: modify write_call to not take arg_count
        self.chunk().write_call(0, 0).map_err(|e| e.to_string())
    }
    pub fn write_function(&mut self, inner_compiler: Compiler) -> Result<(), String> {
        let closure = Value::Closure(Box::new(
            Closure::new(Rc::new(inner_compiler.function))
        ));
        self.chunk().write_closure(closure, inner_compiler.upvalues, 0).map_err(|e| e.to_string())
    }

    pub fn begin_scope(&mut self) {
        self.locals.scope_depth += 1;
    }
    pub fn end_scope(&mut self) -> Result<(), String> {
        self.locals.scope_depth -= 1;
        let mut n_pops = 0;
        while match self.locals.locals.last() {
            Some(local) => local.depth > self.locals.scope_depth,
            None => false,
        } {
            n_pops += 1;
            self.locals.locals.pop();
        }
        self.chunk().write_endblock(n_pops, 0).map_err(|e| e.to_string())
    }

    pub fn create_variable(&mut self, name: String) -> Result<Option<u16>, String> {
        if self.locals.scope_depth == 0 {
            // create a global variable
            return match self.chunk().create_constant(Value::String(
                Rc::new(name)
            )) {
                Ok(idx) => Ok(Some(idx)),
                Err(e) => return Err(e.to_string()),
            };
        }
        // create a local variable
        let local = Local {
            name,
            depth: self.locals.scope_depth,
        };
        self.locals.push(local).map_err(|e| e.to_string())?;
        Ok(None)
    }
    pub fn set_variable(&mut self, idx: Option<u16>) -> Result<(), String> {
        match idx {
            // set global
            Some(idx) => {
                self.chunk().write_set_global(idx, 0).map_err(|e| e.to_string())
            },
            // set local
            None => {
                Ok(self.write_opcode(OpCode::SetLocal))
            }
        }
    }

    fn add_upvalue(&mut self, index: u16, is_local: bool) -> Result<u16, String> {
        let uv = Upvalue { index, is_local };
        // check if this upvalue already exists
        for (i, upvalue) in self.upvalues.iter().enumerate() {
            if *upvalue == uv {
                return Ok(i as u16);
            }
        }
        if self.upvalues.len() == u16::MAX as usize {
            return Err("Too many upvalues in current function".to_string());
        }
        self.upvalues.push(uv);
        self.function.num_upvalues += 1;
        Ok((self.upvalues.len() - 1) as u16)
    }
    fn resolve_upvalue(&mut self, name: &String) -> Result<Option<u16>, String> {
        if self.parent.is_null() {
            return Ok(None);
        }
        let parent = unsafe { &mut *self.parent };
        
        // look for upvalue as local in parent scope
        if let Some(idx) = parent.locals.get_idx(name) {
            return Ok(Some(self.add_upvalue(idx, true)?));
        }

        // look for upvalue recursively going upward in scope
        if let Some(idx) = parent.resolve_upvalue(name)? {
            return Ok(Some(self.add_upvalue(idx, false)?));
        }

        Ok(None)
    }

    pub fn get_variable(&mut self, name: String) -> Result<(), String> {
        let local_idx = self.locals.get_idx(&name);
        let res = if let Some(idx) = local_idx {
            self.chunk().write_get_local(idx, 0)
        }
        else if let Some(idx) = self.resolve_upvalue(&name)? {
            self.chunk().write_get_upvalue(idx, 0)
        }
        else {
            self.chunk().write_get_global(name, 0)
        };
        res.map_err(|e| e.to_string())
    }
}

pub fn compile(source: String) -> Result<Function, String> {
    let tokens = scanner::scan(source);
    let ast = parser::parse(tokens).map_err(|_| "Compilation halted due to parsing error.")?;
    #[cfg(feature = "debug")]
    println!("{:?}", ast);
    let mut compiler = Compiler::default();
    ast.compile(&mut compiler)?;
    compiler.write_opcode(OpCode::Return);
    #[cfg(feature = "debug")]
    compiler.function.chunk.disassemble();

    Ok(compiler.function)
}