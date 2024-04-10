use crate::ast::Expression;
use crate::parser;
use crate::scanner;
use crate::values::Function;

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
            scope_depth: 0,
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
    function: Function,
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
    
}

pub fn compile(source: String) -> Result<Function, String> {
    let tokens = scanner::scan(source);
    let ast = parser::parse(tokens).map_err(|_| "Compilation halted due to parsing error.")?;
    let mut compiler = Compiler::default();
    ast.compile(&mut compiler)?;

    Ok(compiler.function)
}