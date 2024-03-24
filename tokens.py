from enum import Enum

class TokenType(Enum):
    LPAREN = 1
    RPAREN = 2
    LBRACE = 3
    RBRACE = 4

    COMMA = 5
    COLON = 6
    SEMICOLON = 7

    EQ = 8
    NEQ = 9
    GT = 10
    LT = 11
    GEQ = 12
    LEQ = 13

    ASSIGN = 14

    IDENT = 15
    INT = 16
    FLOAT = 17
    STR = 18

    AND = 19
    OR = 20
    TYPE = 21
    FUNC = 22
    IF = 23
    ELSE = 24
    WHILE = 25
    TRUE = 26
    FALSE = 27

    COMMENT = 28
    NEWLINE = 29

    EOF = 30

SINGLE_TOKENS = {
    '(': TokenType.LPAREN,
    ')': TokenType.RPAREN,
    '{': TokenType.LBRACE,
    '}': TokenType.RBRACE,
    ',': TokenType.COMMA,
    ':': TokenType.COLON,
    ';': TokenType.SEMICOLON,
    '=': TokenType.EQ,
    '>': TokenType.GT,
    '<': TokenType.LT,
    '?': TokenType.COMMENT,
}

KEYWORDS = {
    'and': TokenType.AND,
    'or': TokenType.OR,
    'type': TokenType.TYPE,
    'func': TokenType.FUNC,
    'if': TokenType.IF,
    'else': TokenType.ELSE,
    'while': TokenType.WHILE,
    'true': TokenType.TRUE,
    'false': TokenType.FALSE,
}


class Token:
    def __init__(self, ttype: TokenType, lexeme: str, literal, line: int):
        self.ttype = ttype
        self.lexeme = lexeme
        self.literal = literal
        self.line = line

    def __repr__(self):
        return f'Token({self.ttype}, {self.lexeme}, {self.literal}, {self.line})'