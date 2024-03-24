from typing import List, Optional

from tokens import SINGLE_TOKENS, KEYWORDS, TokenType, Token


class Scanner:
    def __init__(self, source: str):
        self.source = source
        self.tokens = []
        self.start = 0
        self.current = 0
        self.line = 1
        self.had_error = False

    def is_at_end(self, offset: int = 0) -> bool:
        return (self.current + offset) >= len(self.source)
    
    def advance(self) -> str:
        char = self.source[self.current]
        self.current += 1
        return char
    
    def match_next(self, expected: str) -> bool:
        if self.is_at_end():
            return False
        if self.source[self.current] != expected:
            return False
        self.current += 1
        return True
    
    def peek(self, offset: int = 0) -> str:
        if self.is_at_end(offset):
            return '\0'
        return self.source[self.current + offset]
    
    def scan_string(self):
        while self.peek() != '"' and not self.is_at_end():
            if self.advance() == '\n':
                self.line += 1
        if self.is_at_end():
            print(f'Unterminated string on line {self.line}')
            self.had_error = True
            return
        self.advance()

        value = self.source[self.start+1:self.current-1]
        self.add_token(TokenType.STR, value)

    def scan_number(self):
        while self.peek().isdigit():
            self.advance()
        if self.peek() == '.' and self.peek(1).isdigit():
            self.advance()
            while self.peek().isdigit():
                self.advance()
        value = self.source[self.start:self.current]
        if '.' in value:
            self.add_token(TokenType.FLOAT, value)
        else:
            self.add_token(TokenType.INT, value)

    def scan_identifier(self):
        while self.peek().isalnum() or self.peek() == '_':
            self.advance()
        value = self.source[self.start:self.current]
        ttype = KEYWORDS.get(value)
        if ttype is None:
            ttype = TokenType.IDENT
        self.add_token(ttype)
    
    def add_token(self, ttype: TokenType, literal=None):
        text = self.source[self.start:self.current]
        self.tokens.append(Token(ttype, text, literal, self.line))

    def scan_token(self):
        char = self.advance()

        # match two-character tokens
        if char == '!' and self.match_next('='):
            self.add_token(TokenType.NEQ)
            return
        if char == '>' and self.match_next('='):
            self.add_token(TokenType.GEQ)
            return
        if char == '<' and self.match_next('='):
            self.add_token(TokenType.LEQ)
            return
        if char == ':' and self.match_next('='):
            self.add_token(TokenType.ASSIGN)
            return
        
        # match one-character tokens
        ttype = SINGLE_TOKENS.get(char)
        if ttype is not None:
            self.add_token(ttype)
            return
        
        # handle whitespace
        if char in ' \r\t':
            return
        if char == '\n':
            self.add_token(TokenType.NEWLINE)
            self.line += 1
            return

        # handle literals
        if char == '"':
            self.scan_string()
            return
        if char.isdigit():
            self.scan_number()
            return
        if char.isalpha() or char == '_':
            self.scan_identifier()
            return
        
        print(f'Unexpected character {char} on line {self.line}')
        self.had_error = True

    def scan(self) -> List[Token]:
        while not self.is_at_end():
            self.start = self.current
            self.scan_token()

        self.tokens.append(Token(TokenType.EOF, '', None, self.line))
        return self.tokens
