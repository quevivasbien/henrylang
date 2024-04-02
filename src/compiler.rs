use std::{collections::HashMap, rc::Rc};

use lazy_static::lazy_static;

use crate::{scan, ObjectString, Chunk, OpCode, Token, TokenType, Value};

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
    Primary,
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
            ParseRule::new(Some(Compiler::grouping), None, Precedence::None),
        );
        map.insert(
            TokenType::LBrace,
            ParseRule::new(Some(Compiler::block), None, Precedence::None),
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
            locals: Vec::new(),
            scope_depth: 0,
        }
    }
}

impl LocalData {
    fn push(&mut self, local: Local) -> Result<(), &'static str> {
        self.locals.push(local);
        if self.locals.len() == u16::MAX as usize {
            return Err("Too many locals declared in current program");
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
    tokens: Vec<Token>,
    chunk: Chunk,
    locals: LocalData,
    current: usize,
    previous: usize,
    had_error: bool,
    panic_mode: bool,
}

impl Compiler {
    fn new(tokens: Vec<Token>, name: String) -> Self {
        Self {
            tokens,
            chunk: Chunk::new(name),
            locals: LocalData::default(),
            current: 0,
            previous: 0,
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
    fn next_token(&self) -> Option<&Token> {
        if self.current + 1 >= self.tokens.len() {
            return None;
        }
        Some(&self.tokens[self.current + 1])
    }
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

    fn begin_scope(&mut self) {
        self.locals.scope_depth += 1;
    }

    fn end_scope(&mut self) {
        self.locals.scope_depth -= 1;
        while match self.locals.locals.last() {
            Some(local) => local.depth > self.locals.scope_depth,
            None => false,
        } {
            self.chunk.write_opcode(OpCode::Pop, self.previous_token().line);
            self.locals.locals.pop();
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

    fn create_local_variable(&mut self, name: String) {
        // if self.locals.scope_depth == 0 {
        //     return;
        // }
        let local = Local {
            name,
            depth: self.locals.scope_depth,
        };
        if let Err(e) = self.locals.push(local) {
            self.error(self.previous, Some(e));
        }
    }

    // write a define opcode that refers to a previously created name constant
    fn define_global_variable(&mut self, idx: u16) {
        if self.locals.scope_depth > 0 {
            return;
        }
        if let Err(e) = self.chunk.write_define(idx, self.previous_token().line) {
            self.error(self.previous, Some(e));
        }
    }

    // then create a constant in the chunk to store the name
    fn create_variable(&mut self, name: String) -> Result<u16, &'static str> {
        if self.locals.scope_depth > 0 {
            self.create_local_variable(name);
            return Ok(0);
        }
        self.chunk.create_define(name)
    }

    fn var_declaration(&mut self, name: String) {
        match self.create_variable(name) {
            Ok(idx) => {
                self.consume(TokenType::Assign, "Expected ':=' after variable name.");
                self.parse_precedence(Precedence::Assignment);
                self.define_global_variable(idx);
            },
            Err(e) => self.error(self.current + 1, Some(e)),
        };
    }

    fn expression(&mut self) {
        self.parse_precedence(Precedence::None.next());
        self.chunk.write_opcode(OpCode::EndExpr, self.previous_token().line);
    }

    fn block(&mut self) {
        self.begin_scope();
        while self.current_token().ttype != TokenType::RBrace && !self.is_eof() {
            self.expression();
        }

        self.consume(TokenType::RBrace, "Expected '}' after block.");
        self.end_scope();
    }

    fn number(&mut self) {
        let token = self.previous_token();
        let value = match token.ttype {
            TokenType::Int => Value::Int(token.text.parse::<i64>().unwrap()),
            TokenType::Float => Value::Float(token.text.parse::<f64>().unwrap()),
            _ => unreachable!(),
        };
        if let Err(e) = self.chunk.write_constant(value, token.line) {
            self.error(self.previous, Some(e));
        }
    }
    fn string(&mut self) {
        let text = &self.previous_token().text;
        let string = Value::Object(Rc::new(
            ObjectString::new(text[1..text.len() - 1].to_string())
        ));
        if let Err(e) = self.chunk.write_constant(string, self.previous_token().line) {
            self.error(self.previous, Some(e));
        }
    }
    fn variable(&mut self) {
        let name = self.previous_token().text.clone();
        // check whether this is an assignment
        if self.current_token().ttype == TokenType::Assign {
            self.var_declaration(name.clone());
            return;
        }
        // proceed with the assumption that the variable has already been defined
        let res = match self.locals.get_idx(&name) {
            Some(idx) => self.chunk.write_get_local(idx, self.previous_token().line),
            None => self.chunk.write_get_global(name.clone(), self.previous_token().line),
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
        self.chunk.write_opcode(
            match operator {
                TokenType::Minus => OpCode::Negate,
                TokenType::Bang => OpCode::Not,
                _ => unreachable!(),
            },
            self.previous_token().line
        );
    }
    fn binary(&mut self) {
        let operator = self.previous_token().ttype;
        let rule = RULES.get(&operator).unwrap();
        self.parse_precedence(rule.precedence.next());
        self.chunk.write_opcode(
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
            },
            self.previous_token().line
        );
    }
    fn literal(&mut self) {
        match self.previous_token().ttype {
            TokenType::True => self.chunk.write_opcode(OpCode::True, self.previous_token().line),
            TokenType::False => self.chunk.write_opcode(OpCode::False, self.previous_token().line),
            _ => unreachable!(),
        }
    }

    fn parse(&mut self) {
        while !self.is_eof() {
            self.expression();
        }
        self.chunk.write_opcode(OpCode::Return, 1);
    }
}

pub fn compile(source: String, name: String) -> Result<Chunk, ()> {
    let tokens = scan(source);
    let mut compiler = Compiler::new(tokens, name);
    compiler.parse();

    if compiler.had_error {
        Err(())
    }
    else {
        Ok(compiler.chunk)
    }
}
