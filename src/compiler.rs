use std::collections::HashMap;
use std::rc::Rc;

use lazy_static::lazy_static;

use crate::{scan, Chunk, Function, OpCode, Token, TokenType, Value};

#[derive(PartialEq, PartialOrd, Copy, Clone)]
enum Precedence {
    None,
    Assignment,
    Or,
    And,
    Equality,
    Comparison,
    Term,
    Factor,
    Unary,
    Call,
    // Primary,
}

impl From<u8> for Precedence {
    fn from(value: u8) -> Self {
        unsafe { std::mem::transmute(value) }
    }
}

impl Precedence {
    fn next(self) -> Precedence {
        (self as u8 + 1).into()
    }
}

struct ParseRule {
    prefix: Option<fn(&mut Compiler)>,
    infix: Option<fn(&mut Compiler)>,
    precedence: Precedence,
}

impl Default for ParseRule {
    fn default() -> Self {
        Self {
            prefix: None,
            infix: None,
            precedence: Precedence::None,
        }
    }
}

impl ParseRule {
    fn new(prefix: Option<fn(&mut Compiler)>, infix: Option<fn(&mut Compiler)>, precedence: Precedence) -> Self {
        Self {
            prefix,
            infix,
            precedence,
        }
    }
}

lazy_static! {
    static ref RULES: HashMap<TokenType, ParseRule> = {
        let mut map = HashMap::new();
        map.insert(
            TokenType::LParen,
            ParseRule::new(Some(Compiler::grouping), Some(Compiler::call), Precedence::Call),
        );
        map.insert(
            TokenType::LBrace,
            ParseRule::new(Some(Compiler::block), None, Precedence::None),
        );
        map.insert(
            TokenType::Pipe,
            ParseRule::new(Some(Compiler::fn_decl), None, Precedence::None),
        );
        map.insert(
            TokenType::Eq,
            ParseRule::new(None, Some(Compiler::binary), Precedence::Equality),
        );
        map.insert(
            TokenType::NEq,
            ParseRule::new(None, Some(Compiler::binary), Precedence::Equality),
        );
        map.insert(
            TokenType::GT,  
            ParseRule::new(None, Some(Compiler::binary), Precedence::Comparison),
        );
        map.insert(
            TokenType::GEq,
            ParseRule::new(None, Some(Compiler::binary), Precedence::Comparison),
        );
        map.insert(
            TokenType::LT,
            ParseRule::new(None, Some(Compiler::binary), Precedence::Comparison),
        );
        map.insert(
            TokenType::LEq,
            ParseRule::new(None, Some(Compiler::binary), Precedence::Comparison),
        );
        map.insert(
            TokenType::Minus,
            ParseRule::new(Some(Compiler::unary), Some(Compiler::binary), Precedence::Term),
        );
        map.insert(
            TokenType::Plus,
            ParseRule::new(None, Some(Compiler::binary), Precedence::Term),
        );
        map.insert(
            TokenType::Slash,
            ParseRule::new(None, Some(Compiler::binary), Precedence::Factor),
        );
        map.insert(
            TokenType::Star,
            ParseRule::new(None, Some(Compiler::binary), Precedence::Factor),
        );
        map.insert(
            TokenType::Bang,
            ParseRule::new(Some(Compiler::unary), None, Precedence::None),
        );
        map.insert(
            TokenType::And,
            ParseRule::new(None, Some(Compiler::binary), Precedence::And),
        );
        map.insert(
            TokenType::Or,
            ParseRule::new(None, Some(Compiler::binary), Precedence::Or),
        );
        map.insert(
            TokenType::Int,
            ParseRule::new(Some(Compiler::number), None, Precedence::None),
        );
        map.insert(
            TokenType::Float,
            ParseRule::new(Some(Compiler::number), None, Precedence::None),
        );
        map.insert(
            TokenType::Str,
            ParseRule::new(Some(Compiler::string), None, Precedence::None),
        );
        map.insert(
            TokenType::True,
            ParseRule::new(Some(Compiler::literal), None, Precedence::None),
        );
        map.insert(
            TokenType::False,
            ParseRule::new(Some(Compiler::literal), None, Precedence::None),
        );
        map.insert(
            TokenType::Ident,
            ParseRule::new(Some(Compiler::variable), None, Precedence::None),
        );
        map.insert(
            TokenType::If,
            ParseRule::new(Some(Compiler::if_expr), None, Precedence::None),
        );

        // define default rules
        for ttype in enum_iterator::all::<TokenType>() {
            if map.get(&ttype).is_none() {
                map.insert(ttype, ParseRule::default());
            }
        }

        map
    };
}

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

struct Compiler {
    tokens: Rc<Vec<Token>>,
    function: Function,
    locals: LocalData,
    current: usize,
    previous: usize,
    had_error: bool,
    panic_mode: bool,
}

impl Compiler {
    fn new(tokens: Rc<Vec<Token>>, current: usize) -> Self {
        Self {
            tokens,
            function: Function::default(),
            locals: LocalData::default(),
            current,
            previous: current,
            had_error: false,
            panic_mode: false,
        }
    }
    fn previous_token(&self) -> &Token {
        &self.tokens[self.previous]
    }
    fn current_token(&self) -> &Token {
        &self.tokens[self.current]
    }
    // fn next_token(&self) -> Option<&Token> {
    //     if self.current + 1 >= self.tokens.len() {
    //         return None;
    //     }
    //     Some(&self.tokens[self.current + 1])
    // }
    fn error(&mut self, loc: usize, message: Option<&str>) {
        if self.panic_mode {
            return;
        }
        self.panic_mode = true;
        let token = &self.tokens[loc];
        eprint!("Error on line {} ", token.line);
        if token.ttype == TokenType::EoF {
            eprint!(" at end")
        }
        else {
            eprint!(" at '{}'", token.text)
        }
        let message = message.unwrap_or(token.text.as_str());
        eprintln!(": {}", message);
        self.had_error = true;
    }
    fn match_token(&mut self, ttype: TokenType) -> bool {
        if self.current_token().ttype == ttype {
            self.advance();
            return true;
        }
        false
    }
    fn advance(&mut self) {
        self.previous = self.current;
        loop {
            self.current += 1;
            if self.current_token().ttype != TokenType::Error {
                break;
            }
            self.error(self.current, None);
        }
    }
    fn consume(&mut self, ttype: TokenType, message: &str) {
        if self.current_token().ttype == ttype {
            self.advance();
            return;
        }
        self.error(self.current, Some(message));
    }
    // fn is_declaration(&self) -> bool {
    //     if self.current_token().ttype != TokenType::Ident {
    //         return false;
    //     }
    //     let next_token = self.next_token();
    //     if let Some(next_token) = next_token {
    //         if next_token.ttype != TokenType::Assign {
    //             return false;
    //         }
    //     }
    //     else {
    //         return false;
    //     }
    //     true
    // }
    fn is_eof(&self) -> bool {
        self.current_token().ttype == TokenType::EoF
    }
    fn chunk(&mut self) -> &mut Chunk {
        &mut self.function.chunk
    }
    fn write_opcode(&mut self, opcode: OpCode) {
        let line = self.previous_token().line;
        self.chunk().write_opcode(opcode, line);
    }
    fn write_constant(&mut self, value: Value) -> Result<(), &'static str> {
        let line = self.previous_token().line;
        self.chunk().write_constant(value, line)
    }
    fn write_call(&mut self, arg_count: u8) -> Result<(), &'static str> {
        let line = self.previous_token().line;
        self.chunk().write_call(arg_count, line)
    }

    fn begin_scope(&mut self) {
        self.locals.scope_depth += 1;
    }

    fn end_scope(&mut self) {
        self.locals.scope_depth -= 1;
        let mut n_pops = 0;
        while match self.locals.locals.last() {
            Some(local) => local.depth > self.locals.scope_depth,
            None => false,
        } {
            n_pops += 1;
            self.locals.locals.pop();
        }
        let line = self.previous_token().line;
        if let Err(e) = self.chunk().write_endblock(n_pops, line) {
            self.error(self.previous, Some(e));
        }
    }

    fn parse_precedence(&mut self, precedence: Precedence) {
        if self.is_eof() {
            return;
        }
        self.advance();
        let prefix_rule = match RULES.get(&self.previous_token().ttype).unwrap().prefix {
            Some(rule) => rule,
            None => return self.error(self.previous, Some("Expected token with prefix rule."))
        };
        prefix_rule(self);

        while
            precedence <= RULES.get(&self.current_token().ttype).unwrap().precedence
            && !self.is_eof()
        {
            self.advance();
            let infix_rule = match RULES.get(&self.previous_token().ttype).unwrap().infix {
                Some(rule) => rule,
                None => return self.error(self.previous, Some("Expected token with infix rule."))
            };
            infix_rule(self);
        }
    }

    // then create a constant in the chunk to store the name
    fn create_variable(&mut self, name: String) -> Option<u16> {
        if self.locals.scope_depth == 0 {
            // create a global variable
            return match self.chunk().create_constant(Value::String(name)) {
                Ok(idx) => Some(idx),
                Err(e) => {
                    self.error(self.previous, Some(e));
                    None
                },
            };
        }
        // create a local variable
        let local = Local {
            name,
            depth: self.locals.scope_depth,
        };
        if let Err(e) = self.locals.push(local) {
            self.error(self.previous, Some(e));
        }
        None
    }

    fn var_declaration(&mut self, name: String) {
        let idx = self.create_variable(name);
        self.consume(TokenType::Assign, "Expected ':=' after variable name.");
        self.parse_precedence(Precedence::Assignment);
        match idx {
            Some(idx) => {
                let line = self.previous_token().line;
                if let Err(e) = self.chunk().write_set_global(idx, line) {
                    self.error(self.previous, Some(e));
                }
            },
            None => self.write_opcode(OpCode::SetLocal),
        }
    }

    fn expression(&mut self) {
        self.parse_precedence(Precedence::None.next());
    }

    fn argument_list(&mut self) -> u8 {
        let mut arg_count = 0;
        if !self.match_token(TokenType::RParen) && !self.is_eof() {
            loop {
                self.expression();
                if arg_count == u8::MAX {
                    self.error(self.previous, Some("Too many arguments in function call"));
                }
                println!("{}", self.current_token().text);
                arg_count += 1;
                if !self.match_token(TokenType::Comma) {
                    break;
                }
            }
            self.consume(TokenType::RParen, "Expected ')' after arguments.");
        }
        arg_count
    }

    fn block(&mut self) {
        self.begin_scope();
        while self.current_token().ttype != TokenType::RBrace && !self.is_eof() {
            self.expression();
            self.write_opcode(OpCode::EndExpr);
        }

        self.consume(TokenType::RBrace, "Expected '}' after block.");
        self.end_scope();
    }

    fn if_expr(&mut self) {
        self.expression();
        let line = self.previous_token().line;
        let jump_if_idx = match self.chunk().write_jump(OpCode::JumpIfFalse, line) {
            Ok(idx) => idx,
            Err(e) => return self.error(self.previous, Some(e)),
        };
        self.consume(TokenType::LBrace, "Expected '{' after 'if'.");
        self.block();  // expr evaluated if condition is true
        let line = self.previous_token().line;
        let jump_else_idx = match self.chunk().write_jump(OpCode::Jump, line) {
            Ok(idx) => idx,
            Err(e) => return self.error(self.previous, Some(e)),
        };
        if let Err(e) = self.chunk().patch_jump(jump_if_idx) {
            self.error(self.previous, Some(e));
        };
        if self.match_token(TokenType::Else) {
            self.consume(TokenType::LBrace, "Expected '{' after 'else'.");
            self.block();
        }
        if let Err(e) = self.chunk().patch_jump(jump_else_idx) {
            self.error(self.previous, Some(e));
        }
    }

    fn fn_decl(&mut self) {
        let mut compiler = Compiler::new(self.tokens.clone(), self.current);

        compiler.begin_scope();
        while !compiler.match_token(TokenType::Pipe) && !compiler.is_eof() {
            if compiler.function.arity == u8::MAX {
                compiler.error(compiler.previous, Some("Cannot have more than 255 parameters."));
            }
            compiler.function.arity += 1;
            // read variable name
            compiler.consume(TokenType::Ident, "Expected function parameter name.");
            let name = compiler.previous_token().text.clone();
            if compiler.create_variable(name).is_some() {
                compiler.error(compiler.previous, Some("Compiler params should be in local scope"));
            };
            // check for comma or end of params
            if compiler.current_token().ttype != TokenType::Pipe {
                compiler.consume(TokenType::Comma, "Expected ',' after function parameter.");
            }
        }
        compiler.consume(TokenType::LBrace, "Expected '{' before function body");
        compiler.block();
        compiler.write_opcode(OpCode::Return);
        compiler.end_scope();

        self.current = compiler.current;
        self.previous = compiler.previous;
        if compiler.had_error {
            self.had_error = true;
        }
        if let Err(e) = self.write_constant(Value::Function(Rc::new(compiler.function))) {
            self.error(self.previous, Some(e));
        }
    }

    fn number(&mut self) {
        let token = self.previous_token();
        let value = match token.ttype {
            TokenType::Int => Value::Int(token.text.parse::<i64>().unwrap()),
            TokenType::Float => Value::Float(token.text.parse::<f64>().unwrap()),
            _ => unreachable!(),
        };
        if let Err(e) = self.write_constant(value) {
            self.error(self.previous, Some(e));
        }
    }
    fn string(&mut self) {
        let text = &self.previous_token().text;
        let string = Value::String(text[1..text.len() - 1].to_string());
        if let Err(e) = self.write_constant(string) {
            self.error(self.previous, Some(e));
        }
    }
    fn variable(&mut self) {
        let name = &self.previous_token().text;
        // check whether this is an assignment
        if self.current_token().ttype == TokenType::Assign {
            self.var_declaration(name.clone());
            return;
        }
        // proceed with the assumption that the variable has already been defined
        let line = self.previous_token().line;
        let res = match self.locals.get_idx(&name) {
            Some(idx) => self.chunk().write_get_local(idx, line),
            None => {
                let name = name.clone();
                self.chunk().write_get_global(name, line)
            },
        };
        if let Err(e) = res {
            self.error(self.previous, Some(e));
        }
    }
    fn grouping(&mut self) {
        self.parse_precedence(Precedence::None.next());
        self.consume(TokenType::RParen, "Expected ')' after expression.");
    }
    fn unary(&mut self) {
        let operator = self.previous_token().ttype;
        self.parse_precedence(Precedence::Unary);
        self.write_opcode(
            match operator {
                TokenType::Minus => OpCode::Negate,
                TokenType::Bang => OpCode::Not,
                _ => unreachable!(),
            }
        );
    }
    fn binary(&mut self) {
        let operator = self.previous_token().ttype;
        let rule = RULES.get(&operator).unwrap();
        self.parse_precedence(rule.precedence.next());
        self.write_opcode(
            match operator {
                TokenType::Eq => OpCode::Equal,
                TokenType::NEq => OpCode::NotEqual,
                TokenType::GT => OpCode::Greater,
                TokenType::GEq => OpCode::GreaterEqual,
                TokenType::LT => OpCode::Less,
                TokenType::LEq => OpCode::LessEqual,
                TokenType::Plus => OpCode::Add,
                TokenType::Minus => OpCode::Subtract,
                TokenType::Star => OpCode::Multiply,
                TokenType::Slash => OpCode::Divide,
                TokenType::And => OpCode::And,
                TokenType::Or => OpCode::Or,
                _ => unreachable!(),
            }
        );
    }
    fn call(&mut self) {
        let arg_count = self.argument_list();
        if let Err(e) = self.write_call(arg_count) {
            self.error(self.previous, Some(e));
        }
    }
    fn literal(&mut self) {
        match self.previous_token().ttype {
            TokenType::True => self.write_opcode(OpCode::True),
            TokenType::False => self.write_opcode(OpCode::False),
            _ => unreachable!(),
        }
    }

    fn parse(&mut self) {
        while !self.is_eof() {
            self.expression();
            self.write_opcode(OpCode::EndExpr);
        }
        self.write_opcode(OpCode::Return);
    }
}

pub fn compile(source: String) -> Result<Function, ()> {
    let tokens = Rc::new(scan(source));
    let mut compiler = Compiler::new(tokens, 0);
    compiler.parse();

    if compiler.had_error {
        Err(())
    }
    else {
        Ok(compiler.function)
    }
}
