use crate::{TokenType, Token};

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

        Token {
            ttype: TokenType::keyword_or_ident(text.as_str()),
            line: self.line,
            text,
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
        if let Some(ttype) = TokenType::single_char_keyword(c) {
            return self.make_token(ttype);
        }
        match c {
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
            '-' => {
                let is_right_arrow = self.match_char('>');
                return self.make_token(
                    if is_right_arrow {
                        TokenType::RightArrow
                    }
                    else {
                        TokenType::Minus
                    }
                )
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

    pub fn scan(&mut self) -> Vec<Token> {
        let mut tokens = Vec::new();
        #[cfg(feature = "debug")]
        let mut line = 0;
        loop {
            let token = self.scan_token();
            #[cfg(feature = "debug")]
            {
                if token.line != line {
                    line = token.line;
                    print!("{:04}: ", line);
                }
                else {
                    print!("    | ");
                }
                println!("{:?}", token);
            }
            if token.ttype == TokenType::EoF {
                tokens.push(token);
                return tokens;
            }
            tokens.push(token);
        }
    }
}

pub fn scan(source: String) -> Vec<Token> {
    let mut scanner = Scanner::new(source);
    scanner.scan()
}
