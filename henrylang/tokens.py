from enum import Enum, auto

class TokenType(Enum):
    LPAREN = auto()
    RPAREN = auto()
    LBRACE = auto()
    RBRACE = auto()

    COMMA = auto()
    COLON = auto()
    SEMICOLON = auto()

    EQ = auto()
    NEQ = auto()
    GT = auto()
    LT = auto()
    GEQ = auto()
    LEQ = auto()

    PLUS = auto()
    MINUS = auto()
    SLASH = auto()
    STAR = auto()

    ASSIGN = auto()
    BANG = auto()
    COMMENT = auto()
    NEWLINE = auto()

    IDENT = auto()
    INT = auto()
    FLOAT = auto()
    STR = auto()

    AND = auto()
    OR = auto()
    TYPE = auto()
    FUNC = auto()
    IF = auto()
    ELSE = auto()
    TRUE = auto()
    FALSE = auto()

    EOF = auto()


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
    '+': TokenType.PLUS,
    '-': TokenType.MINUS,
    '/': TokenType.SLASH,
    '*': TokenType.STAR,
    '!': TokenType.BANG,
    '?': TokenType.COMMENT,
}

KEYWORDS = {
    'and': TokenType.AND,
    'or': TokenType.OR,
    'type': TokenType.TYPE,
    'func': TokenType.FUNC,
    'if': TokenType.IF,
    'else': TokenType.ELSE,
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
        # return f'Token({self.ttype}, {self.lexeme}, {self.literal}, {self.line})'
        return self.lexeme