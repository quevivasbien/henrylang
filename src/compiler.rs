use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use crate::{ast, parser};
use crate::chunk::{Chunk, OpCode};
use crate::scanner;
use crate::values::{Closure, Function, HeapValue, Value};

struct Local {
    name: String,
    is_heap: bool,
    depth: i32,
}

struct LocalData {
    locals: Vec<Local>,
    scope_depth: i32,
}

impl Default for LocalData {
    fn default() -> Self {
        Self {
            locals: Vec::new(),
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

pub type TypeContext = Rc<RefCell<HashMap<String, ast::Type>>>;

pub struct Compiler {
    pub function: Function,
    pub typecontext: TypeContext,
    locals: LocalData,
    upvalues: Vec<Upvalue>,
    // I'm aware that storing pointers rather than references is not ideal, but this drastically simplifies the code, making it so there aren't lifetimes attached to everything
    // In practice, this should be fine: Compilers are only ever created and used by their parents.
    parent: *mut Compiler,
}

impl Compiler {
    pub fn new(typecontext: TypeContext) -> Self {
        Self {
            function: Function::default(),
            typecontext,
            locals: LocalData::default(),
            upvalues: Vec::new(),
            parent: std::ptr::null_mut(),
        }
    }

    pub fn new_from(parent: &mut Compiler) -> Self {
        let typecontext = parent.typecontext.clone();
        Self { parent, ..Self::new(typecontext) }
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
    pub fn write_heap_constant(&mut self, value: HeapValue) -> Result<(), String> {
        self.chunk().write_heap_constant(value, 0).map_err(|e| e.to_string())
    }
    pub fn write_string(&mut self, s: String) -> Result<(), String> {
        let s = HeapValue::String(Rc::new(s));
        self.chunk().write_heap_constant(s, 0).map_err(|e| e.to_string())
    }
    pub fn write_array(&mut self, len: u16) -> Result<(), String> {
        self.chunk().write_array(len, 0).map_err(|e| e.to_string())
    }
    pub fn write_array_array(&mut self, len: u16) -> Result<(), String> {
        self.chunk().write_array_array(len, 0).map_err(|e| e.to_string())
    }
    pub fn write_jump(&mut self, opcode: OpCode) -> Result<usize, String> {
        self.chunk().write_jump(opcode, 0).map_err(|e| e.to_string())
    }
    pub fn patch_jump(&mut self, offset: usize) -> Result<(), String> {
        self.chunk().patch_jump(offset).map_err(|e| e.to_string())
    }
    pub fn write_function(&mut self, inner_compiler: Compiler) -> Result<(), String> {
        unimplemented!("write_function")
        // let closure = Value::Closure(Box::new(
        //     Closure::new(Rc::new(inner_compiler.function))
        // ));
        // self.chunk().write_closure(closure, inner_compiler.upvalues, 0).map_err(|e| e.to_string())
    }

    pub fn begin_scope(&mut self) {
        self.locals.scope_depth += 1;
    }
    pub fn end_scope(&mut self, is_heap: bool) -> Result<(), String> {
        self.locals.scope_depth -= 1;
        let mut n_pops = 0;
        let mut n_heap_pops = 0;
        while let Some(local) = self.locals.locals.last() {
            if local.depth <= self.locals.scope_depth {
                break;
            }
            if local.is_heap {
                n_heap_pops += 1;
            }
            else {
                n_pops += 1;
            }
            self.locals.locals.pop();
        }
        self.chunk().write_endblock(n_pops, n_heap_pops, is_heap, 0).map_err(|e| e.to_string())
    }

    pub fn create_variable(&mut self, name: String, typ: &ast::Type) -> Result<Option<u16>, String> {
        if self.locals.scope_depth == 0 {
            // create a global variable
            let name_hv = HeapValue::String(Rc::new(name.clone()));
            return match self.chunk().create_heap_constant(name_hv) {
                Ok(idx) => {
                    self.typecontext.borrow_mut().insert(name, typ.clone());
                    Ok(Some(idx))
                },
                Err(e) => return Err(e.to_string()),
            };
        }
        // create a local variable
        let local = Local {
            name,
            is_heap: typ.is_heap(),
            depth: self.locals.scope_depth,
        };
        self.locals.push(local).map_err(|e| e.to_string())?;
        Ok(None)
    }
    pub fn set_variable(&mut self, idx: Option<u16>, is_heap: bool) -> Result<(), String> {
        match idx {
            // set global
            Some(idx) => {
                self.chunk().write_set_global(idx, is_heap, 0).map_err(|e| e.to_string())
            },
            // set local
            None => {
                Ok(self.write_opcode(
                    if is_heap { OpCode::SetHeapLocal } else { OpCode::SetLocal }
                ))
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

    pub fn get_variable(&mut self, name: String, is_heap: bool) -> Result<(), String> {
        let local_idx = self.locals.get_idx(&name);
        let res = if let Some(idx) = local_idx {
            self.chunk().write_get_local(idx, is_heap, 0)
        }
        else if let Some(idx) = self.resolve_upvalue(&name)? {
            self.chunk().write_get_upvalue(idx, 0)
        }
        else {
            self.chunk().write_get_global(name, is_heap, 0)
        };
        res.map_err(|e| e.to_string())
    }
}

pub fn compile(source: String, typecontext: TypeContext) -> Result<(Function, ast::Type), String> {
    let tokens = scanner::scan(source);
    let ast = parser::parse(tokens, typecontext.clone()).map_err(|_| "Compilation halted due to parsing error.")?;
    #[cfg(feature = "debug")]
    println!("{:?}", ast);
    let mut compiler = Compiler::new(typecontext);
    ast.compile(&mut compiler)?;
    let return_type = ast.get_type()?;

    Ok((compiler.function, return_type))
}
