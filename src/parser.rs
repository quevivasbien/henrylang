use std::collections::HashMap;

use lazy_static::lazy_static;

use crate::ast::{self, Expression};
use crate::compiler::TypeContext;
use crate::token::{TokenType, Token};

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
            ParseRule::new(Some(Parser::grouping), Some(Parser::call), Precedence::Call),
        );
        map.insert(
            TokenType::LBrace,
            ParseRule::new(Some(Parser::block), None, Precedence::None),
        );
        map.insert(
            TokenType::Dot,
            ParseRule::new(None, Some(Parser::get_field), Precedence::Call),
        );

        // control flow
        map.insert(
            TokenType::If,
            ParseRule::new(Some(Parser::if_statement), None, Precedence::None),
        );

        // object defs
        map.insert(
            TokenType::Pipe,
            ParseRule::new(Some(Parser::function_def), None, Precedence::None),
        );
        map.insert(
            TokenType::LSquare,
            ParseRule::new(Some(Parser::array), None, Precedence::None),
        );
        map.insert(
            TokenType::Type,
            ParseRule::new(Some(Parser::type_def), None, Precedence::None),
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

        // identifiers
        map.insert(
            TokenType::Ident,
            ParseRule::new(Some(Parser::variable), None, Precedence::None),
        );

        map.insert(
            TokenType::Some,
            ParseRule::new(Some(Parser::some), None, Precedence::None),
        );
        map.insert(
            TokenType::Reduce,
            ParseRule::new(Some(Parser::reduce), None, Precedence::None),
        );
        map.insert(
            TokenType::Filter,
            ParseRule::new(Some(Parser::filter), None, Precedence::None),
        );
        map.insert(
            TokenType::Len,
            ParseRule::new(Some(Parser::len), None, Precedence::None),
        );
        map.insert(
            TokenType::ZipMap,
            ParseRule::new(Some(Parser::zipmap), None, Precedence::None),
        );
        map.insert(
            TokenType::Unwrap,
            ParseRule::new(Some(Parser::unwrap), None, Precedence::None),
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

    last_name: Option<String>,
}

impl Parser {
    fn new(tokens: Vec<Token>) -> Self {
        Self { tokens, current: 0, previous: 0, had_error: false, panic_mode: false, last_name: None }
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
    fn consume_if_match(&mut self, ttype: TokenType) -> bool {
        if self.current_ttype() == ttype {
            self.advance();
            return true;
        }
        false
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
        Box::new(ast::Literal::new(ast::Type::Int, token.text.clone()))
    }
    fn float(&mut self) -> Box<dyn ast::Expression> {
        let token = self.previous_token();
        Box::new(ast::Literal::new(ast::Type::Float, token.text.clone()))
    }
    fn string(&mut self) -> Box<dyn ast::Expression> {
        let token = self.previous_token();
        Box::new(ast::Literal::new(ast::Type::String, token.text.clone()))
    }
    fn boolean(&mut self) -> Box<dyn ast::Expression> {
        let token = self.previous_token();
        Box::new(ast::Literal::new(ast::Type::Bool, token.text.clone()))
    }

    fn assignment(&mut self, name: String) -> Box<dyn ast::Expression> {
        self.last_name = Some(name.clone());
        let value = match self.parse_with_precedence(Precedence::Assignment) {
            Some(expr) => expr,
            None => {
                self.error(Some(
                    format!("Expected an expression after ':=', but couldn't find anything.")
                ));
                return Box::new(ast::ErrorExpression{});
            }
        };
        self.last_name = None;
        Box::new(ast::Assignment::new(name, value))
    }
    fn variable(&mut self) -> Box<dyn ast::Expression> {
        let name = self.previous_token().text.clone();
        // check if this is an assignment
        if self.consume_if_match(TokenType::Assign) {
            return self.assignment(name);
        }
        // proceed assuming variable is already defined
        Box::new(ast::Variable::new(name))
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

    fn type_annotation(&mut self) -> Result<ast::TypeAnnotation, String> {
        let typename = self.current_token();
        if typename.ttype != TokenType::Ident {
            return Err(format!("In type annotation, expected identifier, but found {} instead.", typename.text));
        }
        let typename = typename.text.clone();
        self.advance();
        let mut children = Vec::new();
        if self.consume_if_match(TokenType::LParen) {
            while !self.consume_if_match(TokenType::RParen) {
                children.push(self.type_annotation()?);
                self.consume_if_match(TokenType::Comma);
            }
        }
        Ok(ast::TypeAnnotation::new(typename, children))

    }

    fn grouping(&mut self) -> Box<dyn ast::Expression> {
        if self.consume_if_match(TokenType::RParen) {
            self.error(Some(
                format!("Found nothing in grouping parentheses. Parentheses must contain at least one expression or be associated with a call.")
            ));
            return Box::new(ast::ErrorExpression{});
        }
        let expr = self.parse_with_precedence(Precedence::None.next());
        self.consume(
            TokenType::RParen,
            format!("Expected ')' after expression within grouping but found {} instead.", self.current_token().text)
        );
        match expr {
            Some(expr) => expr,
            None => {
                self.error(Some(
                    format!("This should be unreachable.")
                ));
                Box::new(ast::ErrorExpression{})
            }
        }
    }

    fn block(&mut self) -> Box<dyn ast::Expression> {
        // we've started a new context, so we can start reporting errors again
        self.panic_mode = false;
        let mut expressions = Vec::new();
        while !self.consume_if_match(TokenType::RBrace) && !self.is_eof() {
            match self.expression() {
                Some(expr) => expressions.push(expr),
                None => {
                    self.error(Some(
                        format!("Expected expression in block but found {} instead.", self.current_token().text)
                    ));
                    return Box::new(ast::ErrorExpression{});
                }
            }
        }
        if expressions.is_empty() {
            self.consume(TokenType::Colon, format!(
                "Type annotation is required after empty block."
            ));
            let type_annotation = match self.type_annotation() {
                Ok(type_annotation) => type_annotation,
                Err(e) => {
                    self.error(Some(e));
                    return Box::new(ast::ErrorExpression{});
                }
            };
            Box::new(ast::Maybe::new_null(type_annotation))
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

    fn function_def(&mut self) -> Box<dyn ast::Expression> {
        // read parameter list
        let mut params = Vec::new();
        while !self.consume_if_match(TokenType::Pipe) && !self.is_eof() {
            let name = self.current_token();
            if name.ttype != TokenType::Ident {
                self.error(Some(
                    format!("In function definition, expected parameter name but found {} instead.", name.text)
                ));
                return Box::new(ast::ErrorExpression{});
            }
            let name = name.text.clone();
            self.advance();
            self.consume(TokenType::Colon, format!(
                "Missing type annotation for parameter {}.", name
            ));
            let typ = match self.type_annotation() {
                Ok(type_annotation) => type_annotation,
                Err(e) => {
                    self.error(Some(e));
                    return Box::new(ast::ErrorExpression{});
                }
            };
            params.push(ast::NameAndType::new(name, typ));
            if params.len() > u8::MAX as usize {
                self.error(Some(
                    format!("Too many parameters in function definition.")
                ));
                return Box::new(ast::ErrorExpression{});
            }
            // comma is optional between parameter names
            self.consume_if_match(TokenType::Comma);
        }
        // optional type annotation for return type
        let return_type = if self.consume_if_match(TokenType::Colon) {
            Some(match self.type_annotation() {
                Ok(type_annotation) => type_annotation,
                Err(e) => {
                    self.error(Some(e));
                    return Box::new(ast::ErrorExpression{});
                }
            })
        }
        else {
            None
        };
        self.consume(TokenType::LBrace, "Expected '{' after function parameters.".to_string());
        let body = self.block();
        let name = match &self.last_name {
            Some(name) => name.clone(),
            None => "<anon>".to_string()
        };
        Box::new(ast::Function::new(name, params, return_type, body))
    }

    fn array(&mut self) -> Box<dyn ast::Expression> {
        let mut entries = Vec::new();
        while !self.consume_if_match(TokenType::RSquare) && !self.is_eof() {
            let expr = match self.expression() {
                Some(expr) => expr,
                None => {
                    self.error(Some(
                        format!("Expected expression in array but found {} instead.", self.current_token().text)
                    ));
                    return Box::new(ast::ErrorExpression{});
                }
            };
            entries.push(expr);
            self.consume_if_match(TokenType::Comma);
        }

        if entries.is_empty() {
            self.consume(TokenType::Colon, "Empty arrays must be annoted with a type".to_string());
            let typ = match self.type_annotation() {
                Ok(type_annotation) => type_annotation,
                Err(e) => {
                    self.error(Some(e));
                    return Box::new(ast::ErrorExpression{});
                }
            };
            return match ast::Array::new_empty(typ) {
                Ok(array) => Box::new(array),
                Err(e) => {
                    self.error(Some(e));
                    Box::new(ast::ErrorExpression{})
                }
            };
        }
        Box::new(ast::Array::new(entries))
    }

    fn type_def(&mut self) -> Box<dyn ast::Expression> {
        self.consume(TokenType::LBrace, "Expected '{' after 'type'.".to_string());
        let mut fields = Vec::new();
        while !self.consume_if_match(TokenType::RBrace) && !self.is_eof() {
            let name = self.current_token();
            if name.ttype != TokenType::Ident {
                self.error(Some(
                    format!("In type definition, expected field name but found {} instead.", name.text)
                ));
                return Box::new(ast::ErrorExpression{});
            }
            let name = name.text.clone();
            self.advance();
            self.consume(TokenType::Colon, format!(
                "Missing type annotation for field {}.", name
            ));
            let typ = match self.type_annotation() {
                Ok(type_annotation) => type_annotation,
                Err(e) => {
                    self.error(Some(e));
                    return Box::new(ast::ErrorExpression{});
                }
            };
            fields.push(ast::NameAndType::new(name, typ));
            if fields.len() > u8::MAX as usize {
                self.error(Some(
                    format!("Too many fields in type definition.")
                ));
                return Box::new(ast::ErrorExpression{});
            }
            self.consume_if_match(TokenType::Comma);
        }
        let name = match &self.last_name {
            Some(name) => name.clone(),
            None => "<anontype>".to_string(),
        };
        Box::new(ast::TypeDef::new(name, fields))
    }


    fn call(&mut self, callee: Box<dyn ast::Expression>) -> Box<dyn ast::Expression> {
        let mut arguments = Vec::new();
        if !self.consume_if_match(TokenType::RParen) {
            loop {
                let expr = match self.expression() {
                    Some(expr) => expr,
                    None => {
                        self.error(Some(
                            format!("Expected expression as argument in call.")
                        ));
                        return Box::new(ast::ErrorExpression{});
                    }
                };
                arguments.push(expr);
                if arguments.len() > u8::MAX as usize {
                    self.error(Some(
                        format!("Too many arguments in call")
                    ));
                    break;
                }
                // comma is optional between arguments
                self.consume_if_match(TokenType::Comma);
                if self.consume_if_match(TokenType::RParen) {
                    break;
                }
            }
        }
        match ast::Call::new(callee, arguments) {
            Ok(call) => Box::new(call),
            Err(e) => {
                self.error(Some(e));
                Box::new(ast::ErrorExpression{})
            }
        }
    }

    fn get_field(&mut self, obj: Box<dyn ast::Expression>) -> Box<dyn ast::Expression> {
        let name = self.current_token();
        if name.ttype != TokenType::Ident {
            self.error(Some(
                format!("Expected field name but found {} instead.", name.text)
            ));
            return Box::new(ast::ErrorExpression{});
        }
        let name = name.text.clone();
        self.advance();
        Box::new(ast::GetField::new(obj, name))
    }

    fn if_statement(&mut self) -> Box<dyn ast::Expression> {
        let condition = match self.expression() {
            Some(expr) => expr,
            None => {
                self.error(Some(
                    format!("Expected expression as condition after 'if'.")
                ));
                return Box::new(ast::ErrorExpression{});
            }
        };
        self.consume(TokenType::LBrace, "Expected '{' after 'if' condition.".to_string());
        let then_branch = self.block();
        let else_branch = if self.consume_if_match(TokenType::Else) {
            self.consume(TokenType::LBrace, "Expected '{' after 'else'.".to_string());
            Some(self.block())
        }
        else {
            None
        };
        Box::new(ast::IfStatement::new(condition, then_branch, else_branch))
    }

    fn some(&mut self) -> Box<dyn ast::Expression> {
        self.consume(TokenType::LParen, "Expected '(' after 'some'.".to_string());
        let expr = match self.expression() {
            Some(expr) => expr,
            None => {
                self.error(Some(
                    format!("Expected expression as argument in 'some' expression.")
                ));
                return Box::new(ast::ErrorExpression{});
            }
        };
        self.consume(TokenType::RParen, "Expected ')' after 'some' argument.".to_string());
        Box::new(ast::Maybe::new_some(expr))
    }

    fn reduce(&mut self) -> Box<dyn ast::Expression> {
        self.consume(TokenType::LParen, "Expected '(' after 'reduce'.".to_string());
        let fn_expr = match self.expression() {
            Some(expr) => expr,
            None => {
                self.error(Some(
                    format!("Expected function as first argument in 'reduce' expression.")
                ));
                return Box::new(ast::ErrorExpression{});
            }
        };
        self.consume_if_match(TokenType::Comma);
        let arr_expr = match self.expression() {
            Some(expr) => expr,
            None => {
                self.error(Some(
                    format!("Expected array as second argument in 'reduce' expression.")
                ));
                return Box::new(ast::ErrorExpression{});
            }
        };
        self.consume_if_match(TokenType::Comma);
        let init_expr = match self.expression() {
            Some(expr) => expr,
            None => {
                self.error(Some(
                    format!("Expected initial value as third argument in 'reduce' expression.")
                ));
                return Box::new(ast::ErrorExpression{});
            }
        };
        self.consume(TokenType::RParen, "Expected ')' after 'reduce' arguments.".to_string());
        Box::new(ast::Reduce::new(fn_expr, arr_expr, init_expr))
    }

    fn filter(&mut self) -> Box<dyn ast::Expression> {
        self.consume(TokenType::LParen, "Expected '(' after 'filter'.".to_string());
        let fn_expr = match self.expression() {
            Some(expr) => expr,
            None => {
                self.error(Some(
                    format!("Expected function as first argument in 'filter' expression.")
                ));
                return Box::new(ast::ErrorExpression{});
            }
        };
        self.consume_if_match(TokenType::Comma);
        let arr_expr = match self.expression() {
            Some(expr) => expr,
            None => {
                self.error(Some(
                    format!("Expected array as second argument in 'filter' expression.")
                ));
                return Box::new(ast::ErrorExpression{});
            }
        };
        self.consume(TokenType::RParen, "Expected ')' after 'filter' arguments.".to_string());
        Box::new(ast::Filter::new(fn_expr, arr_expr))
    }

    fn len(&mut self) -> Box<dyn ast::Expression> {
        self.consume(TokenType::LParen, "Expected '(' after 'len'.".to_string());
        let expr = match self.expression() {
            Some(expr) => expr,
            None => {
                self.error(Some(
                    format!("Expected expression as argument in 'len' expression.")
                ));
                return Box::new(ast::ErrorExpression{});
            }
        };
        self.consume(TokenType::RParen, "Expected ')' after 'len' argument.".to_string());
        Box::new(ast::Len::new(expr))
    }

    fn zipmap(&mut self) -> Box<dyn ast::Expression> {
        self.consume(TokenType::LParen, "Expected '(' after 'zipmap'.".to_string());
        let fn_expr = match self.expression() {
            Some(expr) => expr,
            None => {
                self.error(Some(
                    format!("Expected function as first argument in 'zipmap' expression.")
                ));
                return Box::new(ast::ErrorExpression{});
            }
        };
        self.consume_if_match(TokenType::Comma);
        let mut exprs = Vec::new();
        while !self.consume_if_match(TokenType::RParen) {
            let expr = match self.expression() {
                Some(expr) => expr,
                None => {
                    self.error(Some(
                        format!("Expected expression as argument in 'zipmap' expression.")
                    ));
                    return Box::new(ast::ErrorExpression{});
                }
            };
            exprs.push(expr);
            self.consume_if_match(TokenType::Comma);
        }
        Box::new(ast::ZipMap::new(fn_expr, exprs))
    }

    fn unwrap(&mut self) -> Box<dyn ast::Expression> {
        self.consume(TokenType::LParen, "Expected '(' after 'unwrap'.".to_string());
        let value = match self.expression() {
            Some(expr) => expr,
            None => {
                self.error(Some(
                    format!("Expected expression as argument in 'unwrap' expression.")
                ));
                return Box::new(ast::ErrorExpression{});
            }
        };
        self.consume_if_match(TokenType::Comma);
        let default = match self.expression() {
            Some(expr) => expr,
            None => {
                self.error(Some(
                    format!("Expected default value as second argument in 'unwrap' expression.")
                ));
                return Box::new(ast::ErrorExpression{});
            }
        };
        self.consume(TokenType::RParen, "Expected ')' after 'unwrap' argument.".to_string());
        Box::new(ast::Unwrap::new(value, default))
    }

    fn parse(&mut self, typecontext: TypeContext) -> Box<dyn ast::Expression> {
        let block = self.block();
        let mut top_level = Box::new(ast::ASTTopLevel::new(typecontext, block));
        top_level.set_parent(None);
        println!("top_level: {:?}", top_level);
        top_level
    }
}

pub fn parse(tokens: Vec<Token>, typecontext: TypeContext) -> Result<Box<dyn ast::Expression>, ()> {
    let mut parser = Parser::new(tokens);
    let ast = parser.parse(typecontext);
    if parser.had_error {
        return Err(())
    }
    Ok(ast)
}