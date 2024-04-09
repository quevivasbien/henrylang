use std::collections::HashMap;

use lazy_static::lazy_static;

use crate::ast;
use crate::token::{TokenType, Token};
use crate::values::Type;

#[derive(PartialEq, PartialOrd, Copy, Clone)]
enum Precedence {
    None,
    Assignment,
    Or,
    And,
    Equality,
    Comparison,
    Range,
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

type PrefixParseFn = fn(&mut Parser) -> Box<dyn ast::Expression>;
type InfixParseFn = fn(&mut Parser, Box<dyn ast::Expression>) -> Box<dyn ast::Expression>;

struct ParseRule {
    prefix: Option<PrefixParseFn>,
    infix: Option<InfixParseFn>,
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
    fn new(prefix: Option<PrefixParseFn>, infix: Option<InfixParseFn>, precedence: Precedence) -> Self {
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
        // groupings
        map.insert(
            TokenType::LParen,
            ParseRule::new(Some(Parser::grouping), Some(Parser::call), Precedence::None),
        );
        map.insert(
            TokenType::LBrace,
            ParseRule::new(Some(Parser::block), None, Precedence::None),
        );

        // literals
        map.insert(
            TokenType::Int,
            ParseRule::new(Some(Parser::int), None, Precedence::None),
        );
        map.insert(
            TokenType::Float,
            ParseRule::new(Some(Parser::float), None, Precedence::None),
        );
        map.insert(
            TokenType::Str,
            ParseRule::new(Some(Parser::string), None, Precedence::None),
        );
        map.insert(
            TokenType::True,
            ParseRule::new(Some(Parser::boolean), None, Precedence::None),
        );
        map.insert(
            TokenType::False,
            ParseRule::new(Some(Parser::boolean), None, Precedence::None),
        );

        // operators
        map.insert(
            TokenType::Bang,
            ParseRule::new(Some(Parser::unary), None, Precedence::None),
        );
        map.insert(
            TokenType::Plus,
            ParseRule::new(None, Some(Parser::binary), Precedence::Term),
        );
        map.insert(
            TokenType::Minus,
            ParseRule::new(Some(Parser::unary), Some(Parser::binary), Precedence::Term),
        );
        map.insert(
            TokenType::Star,
            ParseRule::new(None, Some(Parser::binary), Precedence::Factor),
        );
        map.insert(
            TokenType::Slash,
            ParseRule::new(None, Some(Parser::binary), Precedence::Factor),
        );
        
        map.insert(
            TokenType::And,
            ParseRule::new(None, Some(Parser::binary), Precedence::And),
        );
        map.insert(
            TokenType::Or,
            ParseRule::new(None, Some(Parser::binary), Precedence::Or),
        );
        
        map.insert(
            TokenType::To,
            ParseRule::new(None, Some(Parser::binary), Precedence::Range),
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
            TokenType::LT,
            ParseRule::new(None, Some(Parser::binary), Precedence::Comparison),
        );
        map.insert(
            TokenType::LEq,
            ParseRule::new(None, Some(Parser::binary), Precedence::Comparison),
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
            TokenType::RightArrow,
            ParseRule::new(None, Some(Parser::binary), Precedence::Assignment),
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
    current: usize,
    previous: usize,

    had_error: bool,
    panic_mode: bool,
}

impl Parser {
    fn new(tokens: Vec<Token>) -> Self {
        Self { tokens, current: 0, previous: 0, had_error: false, panic_mode: false }
    }
    
    fn previous_token(&self) -> &Token {
        &self.tokens[self.previous]
    }
    fn current_token(&self) -> &Token {
        &self.tokens[self.current]
    }
    fn current_ttype(&self) -> TokenType {
        self.current_token().ttype
    }
    fn is_eof(&self) -> bool {
        self.current_ttype() == TokenType::EoF
    }

    fn error(&mut self, message: Option<String>) {
        if self.panic_mode {
            return;
        }
        self.panic_mode = true;
        let token = self.previous_token();
        eprint!("Error on line {} ", token.line);
        if token.ttype == TokenType::EoF {
            eprint!("at end")
        }
        else {
            eprint!("at '{}'", token.text)
        }
        let message = message.unwrap_or(token.text.clone());
        eprintln!(": {}", message);
        self.had_error = true;
    }


    fn advance(&mut self) {
        self.previous = self.current;
        loop {
            self.current += 1;
            if self.current_ttype() != TokenType::Error {
                break;
            }
            self.error(None);
        }
    }
    fn consume(&mut self, ttype: TokenType, message: String) {
        if self.current_ttype() == ttype {
            self.advance();
            return;
        }
        self.error(Some(message));
    }

    fn parse_with_precedence(&mut self, precedence: Precedence) -> Option<Box<dyn ast::Expression>> {
        if self.is_eof() {
            return None;
        }
        self.advance();
        let prefix_rule = match RULES.get(&self.previous_token().ttype).unwrap().prefix {
            Some(rule) => rule,
            None => {
                self.error(Some(
                    format!("Expected a token that can begin an expression, but got {}", self.previous_token().text)
                ));
                return None;
            }
        };
        let mut prefix = prefix_rule(self);

        while
            precedence <= RULES.get(&self.current_token().ttype).unwrap().precedence
            && !self.is_eof()
        {
            self.advance();
            let infix_rule = match RULES.get(&self.previous_token().ttype).unwrap().infix {
                Some(rule) => rule,
                None => {
                    self.error(Some(
                    format!("Expected token that can act as an infix operator, but got {}", self.previous_token().text)));
                    return None;
                }
            };
            prefix = infix_rule(self, prefix);
        }

        Some(prefix)
    }

    fn expression(&mut self) -> Option<Box<dyn ast::Expression>> {
        self.parse_with_precedence(Precedence::None.next())
    }

    fn int(&mut self) -> Box<dyn ast::Expression> {
        let token = self.previous_token();
        Box::new(ast::Literal::new(Type::Int, token.text.clone()))
    }
    fn float(&mut self) -> Box<dyn ast::Expression> {
        let token = self.previous_token();
        Box::new(ast::Literal::new(Type::Float, token.text.clone()))
    }
    fn string(&mut self) -> Box<dyn ast::Expression> {
        let token = self.previous_token();
        Box::new(ast::Literal::new(Type::String, token.text.clone()))
    }
    fn boolean(&mut self) -> Box<dyn ast::Expression> {
        let token = self.previous_token();
        Box::new(ast::Literal::new(Type::Bool, token.text.clone()))
    }

    fn unary(&mut self) -> Box<dyn ast::Expression> {
        let token = self.previous_token().clone();
        let right = match self.parse_with_precedence(Precedence::Unary) {
            Some(expr) => expr,
            None => {
                self.error(Some(
                    format!("Expected something that could follow '{}', but couldn't find anything.", token.text)
                ));
                return Box::new(ast::ErrorExpression{});
            }
        };
        match ast::Unary::new(token.ttype, right) {
            Ok(expr) => Box::new(expr),
            Err(e) => {
                self.error(Some(e));
                Box::new(ast::ErrorExpression{})
            }
        }
    }

    fn binary(&mut self, left: Box<dyn ast::Expression>) -> Box<dyn ast::Expression> {
        let token = self.previous_token().clone();
        let rule = RULES.get(&token.ttype).unwrap();
        let right = match self.parse_with_precedence(rule.precedence.next()) {
            Some(expr) => expr,
            None => {
                self.error(Some(
                    format!("Expected something that could follow '{}', but couldn't find anything.", token.text)
                ));
                return Box::new(ast::ErrorExpression{});
            }
        };
        match ast::Binary::new(left, token.ttype, right) {
            Ok(expr) => Box::new(expr),
            Err(e) => {
                self.error(Some(e));
                Box::new(ast::ErrorExpression{})
            }
        }
    }

    fn grouping(&mut self) -> Box<dyn ast::Expression> {
        let expr = self.parse_with_precedence(Precedence::None.next());
        self.consume(
            TokenType::RParen,
            format!("Expected ')' after expression within grouping but found {} instead.", self.current_token().text)
        );
        match expr {
            Some(expr) => expr,
            None => Box::new(ast::Maybe::new(None))
        }
    }

    fn block(&mut self) -> Box<dyn ast::Expression> {
        let mut expressions = Vec::new();
        while self.current_ttype() != TokenType::RBrace && !self.is_eof() {
            if let Some(expr) = self.expression() {
                expressions.push(expr);
            }
        }
        self.consume(TokenType::RBrace, "Expected '}' after block.".to_string());
        if expressions.is_empty() {
            Box::new(ast::Maybe::new(None))
        }
        else {
            match ast::Block::new(expressions) {
                Ok(block) => Box::new(block),
                Err(e) => {
                    self.error(Some(e));
                    Box::new(ast::ErrorExpression{})
                }
            }
        }
    }

    fn call(&mut self, callee: Box<dyn ast::Expression>) -> Box<dyn ast::Expression> {
        todo!()
    }

    fn parse(&mut self) -> ast::Function {
        let mut main = ast::Function::new("<main>".to_string(), Vec::new(), Type::Maybe);
        while !self.is_eof() {
            if let Some(expr) = self.expression() {
                main.expressions.push(expr);
            }
        }
        main
    }
}

pub fn parse(tokens: Vec<Token>) -> Result<ast::Function, String> {
    let mut parser = Parser::new(tokens);
    Ok(parser.parse())
}