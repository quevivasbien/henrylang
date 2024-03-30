use crate::{OpCode, Chunk, TokenType, Token, scan};

struct Parser<'a, 'b> {
    tokens: &'a Vec<Token>,
    chunk: &'b mut Chunk,
    current: usize,
    previous: usize,
    had_error: bool,
    panic_mode: bool,
}

impl<'a, 'b> Parser<'a, 'b> {
    fn new(tokens: &'a Vec<Token>, chunk: &'b mut Chunk) -> Self {
        Self {
            tokens,
            chunk,
            current: 0,
            previous: 0,
            had_error: false,
            panic_mode: false,
        }
    }
    fn previous_token(&self) -> &'a Token {
        &self.tokens[self.previous]
    }
    fn current_token(&self) -> &'a Token {
        &self.tokens[self.current]
    }
    fn scan_token(&mut self) -> usize {
        todo!()
    }
    fn error(&mut self, loc: usize, message: &str) {
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
        eprintln!(": {}", message);
        self.had_error = true;
    }
    fn consume(&mut self, ttype: TokenType, message: &str) {
        if self.current_token().ttype == ttype {
            self.advance();
            return;
        }
        self.error(self.current, message);
    }
    fn emit_byte(&mut self, byte: u8) {
        self.chunk.write_byte(byte, self.previous_token().line);
    }
    fn expression(&mut self) {
        todo!()
    }
    fn number(&mut self) {
        let value = self.previous_token().text.parse::<f64>().unwrap();
        let idx = match self.chunk.add_constant(value) {
            Ok(idx) => idx,
            Err(e) => return self.error(self.previous, e),
        };
        self.chunk.write_constant(idx);
    }
    fn grouping(&mut self) {
        self.expression();
        self.consume(TokenType::RParen, "Expected ')' after expression.");
    }
    fn unary(&mut self) {
        let operator_type = self.previous_token().ttype;
        self.expression();
        self.chunk.write_opcode(match operator_type {
            TokenType::Minus => OpCode::Negate,
            _ => unreachable!(),
        });
    }
    fn advance(&mut self) {
        self.previous = self.current;
        loop {
            self.current = self.scan_token();
            if self.current_token().ttype == TokenType::Error {
                break;
            }

            self.error(self.current, self.current_token().text.as_str());
        }
    }

    fn parse(&mut self) {
        self.chunk.write_opcode(OpCode::Return);
    }
}

pub fn compile(source: String, name: String) -> Result<Chunk, &'static str> {
    let tokens = scan(source);
    let mut chunk = Chunk::new(name);
    let mut parser = Parser::new(&tokens, &mut chunk);
    parser.parse();
    Ok(chunk)
}
