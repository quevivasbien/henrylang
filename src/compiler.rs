use std::collections::HashMap;

use lazy_static::lazy_static;

use crate::{OpCode, Chunk, TokenType, Token, Value, scan};

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
            TokenType::Int,
            ParseRule::new(Some(Parser::number), None, Precedence::None),
        );
        map.insert(
            TokenType::Float,
            ParseRule::new(Some(Parser::number), None, Precedence::None),
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
    fn expression(&mut self) {
        self.parse_precedence(Precedence::Assignment);
    }

    fn number(&mut self) {
        let token = self.previous_token();
        let value = match token.ttype {
            TokenType::Int => Value::Int(token.text.parse::<i64>().unwrap()),
            TokenType::Float => Value::Float(token.text.parse::<f64>().unwrap()),
            _ => unreachable!(),
        };
        match self.chunk.write_constant(value, token.line) {
            Ok(_) => (),
            Err(e) => self.error(self.previous, Some(e)),
        };
    }
    pub fn grouping(&mut self) {
        self.expression();
        self.consume(TokenType::RParen, "Expected ')' after expression.");
    }
    fn unary(&mut self) {
        let operator = self.previous_token().ttype;
        self.parse_precedence(Precedence::Unary);
        self.chunk.write_opcode(
            match operator {
                TokenType::Minus => OpCode::Negate,
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
                TokenType::Plus => OpCode::Add,
                TokenType::Minus => OpCode::Subtract,
                TokenType::Star => OpCode::Multiply,
                TokenType::Slash => OpCode::Divide,
                _ => unreachable!(),
            },
            self.previous_token().line
        );
    }

    fn parse(&mut self) {
        self.expression();
        self.chunk.write_opcode(OpCode::Return, 1);
    }
}

pub fn compile(source: String, name: String) -> Result<Chunk, &'static str> {
    let tokens = scan(source);
    let mut parser = Parser::new(tokens, name);
    parser.parse();

    if parser.had_error {
        Err("Error during compilation")
    }
    else {
        Ok(parser.chunk)
    }
}
