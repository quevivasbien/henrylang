use std::collections::HashMap;

use lazy_static::lazy_static;

use crate::{OpCode, Chunk};

#[derive(Debug, PartialEq, Copy, Clone)]
enum TokenType {
    LParen,
    RParen,
    LBrace,
    RBrace,
    Pipe,

    Comma,
    Dot,
    Colon,

    Eq,
    NEq,
    GT,
    LT,
    GEq,
    LEq,

    Plus,
    Minus,
    Slash,
    Star,

    Assign,
    Bang,

    Ident,
    Int,
    Float,
    Str,

    And,
    Or,
    Type,
    If,
    Else,
    True,
    False,
    To,

    Error,
    EoF,
}

lazy_static! {
    static ref KEYWORDS: HashMap<&'static str, TokenType> = {
        let mut map = HashMap::new();
        map.insert("and", TokenType::And);
        map.insert("or", TokenType::Or);
        map.insert("type", TokenType::Type);
        map.insert("if", TokenType::If);
        map.insert("else", TokenType::Else);
        map.insert("true", TokenType::True);
        map.insert("false", TokenType::False);
        map.insert("to", TokenType::To);

        map
    };
}

#[derive(Debug)]
struct Token {
    ttype: TokenType,
    line: usize,
    text: String,
}

struct Scanner {
    source: Vec<char>,
    start: usize,
    current: usize,
    line: usize,
}

impl Scanner {
    pub fn new(source: String) -> Self {
        Self {
            source: source.chars().collect(),
            start: 0,
            current: 0,
            line: 1,
        }
    }

    fn is_at_end(&self) -> bool {
        self.peek(0) == '\0'
    }

    fn peek(&self, offset: usize) -> char {
        if self.current + offset >= self.source.len() {
            '\0'
        }
        else {
            self.source[self.current + offset]
        }
    }

    fn advance(&mut self) -> char {
        self.current += 1;
        self.source[self.current - 1]
    }

    fn match_char(&mut self, expected: char) -> bool {
        if self.is_at_end() {
            return false;
        }
        if self.peek(0) != expected {
            return false;
        }
        self.advance();
        true
    }

    fn make_token(&self, ttype: TokenType) -> Token {
        let text: String = self.source[self.start..self.current].iter().collect();
        Token {
            ttype,
            line: self.line,
            text: text,
        }
    }

    fn error_token(&self, message: &'static str) -> Token {
        Token {
            ttype: TokenType::Error,
            line: self.line,
            text: message.to_string(),
        }
    }

    fn handle_comment(&mut self) {
        if !self.match_char('?') {
            return;
        }
        while !self.is_at_end() && self.peek(0) != '\n' {
            self.advance();
        }
    }

    fn handle_comments_and_whitespace(&mut self) {
        while !self.is_at_end() {
            match self.peek(0) {
                '\n' => {
                    self.line += 1;
                },
                '?' => self.handle_comment(),
                x => if !x.is_ascii_whitespace() {
                    break
                },
            };
            if !self.is_at_end() {
                self.advance();
            }
        }
    }

    fn read_string(&mut self) -> Token {
        while !self.is_at_end() && self.peek(0) != '"' {
            if self.peek(0) == '\n' {
                self.line += 1;
            }
            self.advance();
        }
        if self.is_at_end() {
            return self.error_token("Unterminated string");
        }
        self.advance();
        self.make_token(TokenType::Str)
    }

    fn read_number(&mut self) -> Token {
        while self.peek(0).is_ascii_digit() {
            self.advance();
        }
        if self.peek(0) == '.' && self.peek(1).is_ascii_digit() {
            self.advance();
            while self.peek(0).is_ascii_digit() {
                self.advance();
            }
            return self.make_token(TokenType::Float);
        }
        self.make_token(TokenType::Int)
    }

    fn read_ident_or_keyword(&mut self) -> Token {
        while self.peek(0).is_alphanumeric() || self.peek(0) == '_' {
            self.advance();
        }
        let text: String = self.source[self.start..self.current].iter().collect();
        let ttype = match KEYWORDS.get(text.as_str()) {
            Some(x) => *x,
            None => TokenType::Ident,
        };

        Token {
            ttype,
            line: self.line,
            text: text,
        }
    }

    fn scan_token(&mut self) -> Token {
        self.handle_comments_and_whitespace();

        self.start = self.current;

        if self.is_at_end() {
            return self.make_token(TokenType::EoF);
        }

        // match single-character tokens
        let c = self.advance();
        match c {
            '(' => return self.make_token(TokenType::LParen),
            ')' => return self.make_token(TokenType::RParen),
            '{' => return self.make_token(TokenType::LBrace),
            '}' => return self.make_token(TokenType::RBrace),
            '|' => return self.make_token(TokenType::Pipe),
            '.' => return self.make_token(TokenType::Dot),
            ',' => return self.make_token(TokenType::Comma),
            '=' => return self.make_token(TokenType::Eq),
            '+' => return self.make_token(TokenType::Plus),
            '-' => return self.make_token(TokenType::Minus),
            '/' => return self.make_token(TokenType::Slash),
            '*' => return self.make_token(TokenType::Star),
            '!' => {
                let is_neq = self.match_char('=');
                return self.make_token(
                    if is_neq {
                        TokenType::NEq
                    }
                    else {
                        TokenType::Bang
                    }
                );
            },
            '>' => {
                let is_geq = self.match_char('=');
                return self.make_token(
                    if is_geq {
                        TokenType::GEq
                    }
                    else {
                        TokenType::GT
                    }
                );
            },
            '<' => {
                let is_leq = self.match_char('=');
                return self.make_token(
                    if is_leq {
                        TokenType::LEq
                    }
                    else {
                        TokenType::LT
                    }
                );
            },
            ':' => {
                let is_assign = self.match_char('=');
                return self.make_token(
                    if is_assign {
                        TokenType::Assign
                    }
                    else {
                        TokenType::Colon
                    }
                );
            },
            _ => (),
        };

        // handle literals
        if c == '"' {
            return self.read_string();
        }
        if c.is_numeric() {
            return self.read_number();
        }
        if c.is_alphabetic() || c == '_' {
            return self.read_ident_or_keyword();
        }

        self.error_token("Unexpected character")
    }

    pub fn scan(&mut self) -> Result<(), &'static str> {
        let mut line = 0;
        loop {
            let token = self.scan_token();
            if token.line != line {
                line = token.line;
                print!("{:04}: ", line);
            }
            else {
                print!("    | ");
            }
            println!("{:?}", token);
            if token.ttype == TokenType::EoF {
                return Ok(());
            }
        }
    }
}

pub fn compile(source: String, name: String) -> Result<Chunk, &'static str> {
    let mut scanner = Scanner::new(source);
    scanner.scan()?;

    let mut chunk = Chunk::new(name);

    chunk.write_opcode(OpCode::Return);
    Ok(chunk)
}