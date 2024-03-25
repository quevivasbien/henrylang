from typing import List, Optional

import expressions
from expressions import Expression
from tokens import TokenType, Token

class ParseException(Exception):
    pass

class Parser:
    def __init__(self, tokens: List[Token]):
        self.tokens = tokens
        self.current = 0
        self.had_error = False

    def peek(self) -> Token:
        return self.tokens[self.current]

    def is_at_end(self) -> bool:
        return self.peek().ttype == TokenType.EOF

    def previous(self) -> Token:
        return self.tokens[self.current - 1]
    
    def advance(self) -> Token:
        if not self.is_at_end():
            self.current += 1
        return self.previous()
    
    def check(self, *types: TokenType) -> bool:
        if self.is_at_end():
            return False
        return self.peek().ttype in types
    
    def match(self, *types: TokenType) -> bool:
        if self.check(*types):
            self.advance()
            return True
        return False
    
    def error(self, token: Token, message: str):
        self.had_error = True
        return ParseException(message)
    
    def consume(self, ttype: TokenType, message: str) ->Token:
        if self.check(ttype):
            return self.advance()
        raise self.error(self.peek(), message)
    
    def primary(self) -> Expression:
        if self.match(TokenType.FALSE):
            return expressions.Literal(False)
        if self.match(TokenType.TRUE):
            return expressions.Literal(True)
        
        if self.match(TokenType.INT, TokenType.FLOAT, TokenType.STR):
            return expressions.Literal(self.previous().literal)

        if self.match(TokenType.LPAREN):
            expr = self.expression()
            self.consume(TokenType.RPAREN, "Expect ')' after expression.")
            return expressions.Grouping(expr)
        
        raise self.error(self.peek(), "Expect expression.")
    
    def unary(self) -> Expression:
        while self.match(TokenType.MINUS, TokenType.BANG):
            operator = self.previous()
            right = self.unary()
            return expressions.Unary(operator, right)
        return self.primary()

    def factor(self) -> Expression:
        expr = self.unary()
        while self.match(TokenType.SLASH, TokenType.STAR):
            operator = self.previous()
            right = self.unary()
            expr = expressions.Binary(expr, operator, right)
        return expr

    def term(self) -> Expression:
        expr = self.factor()
        while self.match(self, TokenType.PLUS, TokenType.MINUS):
            operator = self.previous()
            right = self.factor()
            expr = expressions.Binary(expr, operator, right)
        return expr

    def comparison(self) -> Expression:
        expr = self.term()
        while self.match(TokenType.GT, TokenType.LT, TokenType.GEQ, TokenType.LEQ):
            operator = self.previous()
            right = self.term()
            expr = expressions.Binary(expr, operator, right)
        return expr

    def equality(self) -> Expression:
        expr = self.comparison()
        while self.match(TokenType.EQ, TokenType.NEQ):
            operator = self.previous()
            right = self.comparison()
            expr = expressions.Binary(expr, operator, right)

        return expr

    def expression(self) -> Expression:
        return self.equality()
    
    def synchronize(self) -> None:
        self.advance()
        while not self.is_at_end():
            if self.previous().ttype == TokenType.NEWLINE:
                return
            next_ttype = self.peek().ttype
            if next_ttype in (
                TokenType.TYPE,
                TokenType.FUNC,
                TokenType.IF,
                TokenType.WHILE,
            ):
                return
            self.advance()

    def parse(self) -> Optional[Expression]:
        try:
            return self.expression()
        except ParseException:
            self.synchronize()
            return None


if __name__ == '__main__':
    from scanner import Scanner
    tokens = Scanner("1 + 2 * 3 if { hello }").scan()
    parser = Parser(tokens)
    expr = parser.parse()
    print(expr)
    print(expr.eval())
        