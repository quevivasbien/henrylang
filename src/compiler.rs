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
    prefix: Option<fn(&mut Parser)>,
    infix: Option<fn(&mut Parser)>,
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
    fn new(prefix: Option<fn(&mut Parser)>, infix: Option<fn(&mut Parser)>, precedence: Precedence) -> Self {
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
            ParseRule::new(Some(Parser::grouping), None, Precedence::None),
        );
        map.insert(
            TokenType::Eq,
            ParseRule::new(None, Some(Parser::binary), Precedence::Equality),
        );
        map.insert(
            TokenType::NEq,
            ParseRule::new(None, Some(Parser::binary), Precedence::Equality),
        );
        map.insert(
            TokenType::GT,  
            ParseRule::new(None, Some(Parser::binary), Precedence::Comparison),
        );
        map.insert(
            TokenType::GEq,
            ParseRule::new(None, Some(Parser::binary), Precedence::Comparison),
        );
        map.insert(
            TokenType::LT,
            ParseRule::new(None, Some(Parser::binary), Precedence::Comparison),
        );
        map.insert(
            TokenType::LEq,
            ParseRule::new(None, Some(Parser::binary), Precedence::Comparison),
        );
        map.insert(
            TokenType::Minus,
            ParseRule::new(Some(Parser::unary), Some(Parser::binary), Precedence::Term),
        );
        map.insert(
            TokenType::Plus,
            ParseRule::new(None, Some(Parser::binary), Precedence::Term),
        );
        map.insert(
            TokenType::Slash,
            ParseRule::new(None, Some(Parser::binary), Precedence::Factor),
        );
        map.insert(
            TokenType::Star,
            ParseRule::new(None, Some(Parser::binary), Precedence::Factor),
        );
        map.insert(
            TokenType::Bang,
            ParseRule::new(Some(Parser::unary), None, Precedence::None),
        );
        map.insert(
            TokenType::Int,
            ParseRule::new(Some(Parser::number), None, Precedence::None),
        );
        map.insert(
            TokenType::Float,
            ParseRule::new(Some(Parser::number), None, Precedence::None),
        );
        map.insert(
            TokenType::Str,
            ParseRule::new(Some(Parser::string), None, Precedence::None),
        );
        map.insert(
            TokenType::True,
            ParseRule::new(Some(Parser::literal), None, Precedence::None),
        );
        map.insert(
            TokenType::False,
            ParseRule::new(Some(Parser::literal), None, Precedence::None),
        );
        map.insert(
            TokenType::Ident,
            ParseRule::new(Some(Parser::variable), None, Precedence::None),
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

struct Parser {
    tokens: Vec<Token>,
    chunk: Chunk,
    current: usize,
    previous: usize,
    had_error: bool,
    panic_mode: bool,
}

impl Parser {
    fn new(tokens: Vec<Token>, name: String) -> Self {
        Self {
            tokens,
            chunk: Chunk::new(name),
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
    fn is_declaration(&self) -> bool {
        if self.current_token().ttype != TokenType::Ident {
            return false;
        }
        let next_token = self.next_token();
        if let Some(next_token) = next_token {
            if next_token.ttype != TokenType::Assign {
                return false;
            }
        }
        else {
            return false;
        }
        true
    }

    fn parse_precedence(&mut self, precedence: Precedence) {
        self.advance();
        let prefix_rule = match RULES.get(&self.previous_token().ttype).unwrap().prefix {
            Some(rule) => rule,
            None => return self.error(self.previous, Some("Expected token with prefix rule."))
        };
        prefix_rule(self);

        while precedence <= RULES.get(&self.current_token().ttype).unwrap().precedence {
            self.advance();
            let infix_rule = match RULES.get(&self.previous_token().ttype).unwrap().infix {
                Some(rule) => rule,
                None => return self.error(self.previous, Some("Expected token with infix rule."))
            };
            infix_rule(self);
        }
    }

    fn create_variable(&mut self, message: &str) -> Result<u16, &'static str> {
        self.consume(TokenType::Ident, message);
        let name = &self.previous_token().text;
        self.chunk.create_define(name.clone())
    }

    fn define_global(&mut self, idx: u16) {
        if let Err(e) = self.chunk.write_define(idx, self.previous_token().line) {
            self.error(self.previous, Some(e));
        }
    }

    fn var_declaration(&mut self) -> bool {
        match self.create_variable("Expected variable name") {
            Ok(idx) => {
                self.consume(TokenType::Assign, "Expected ':=' after variable name.");
                self.expression();
                self.define_global(idx);
            },
            Err(e) => self.error(self.current + 1, Some(e)),
        };
        true
    }

    fn expression(&mut self) {
        if self.is_declaration() {
            self.var_declaration();
        }
        else {
            self.parse_precedence(Precedence::Assignment);
        }
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
        let name = &self.previous_token().text;
        if let Err(e) = self.chunk.write_get(name.clone(), self.previous_token().line) {
            self.error(self.previous, Some(e));
        }
    }
    fn grouping(&mut self) {
        self.expression();
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
        while self.current_token().ttype != TokenType::EoF {
            self.expression();
        }
        self.chunk.write_opcode(OpCode::Return, 1);
    }
}

pub fn compile(source: String, name: String) -> Result<Chunk, ()> {
    let tokens = scan(source);
    let mut parser = Parser::new(tokens, name);
    parser.parse();

    if parser.had_error {
        Err(())
    }
    else {
        Ok(parser.chunk)
    }
}
