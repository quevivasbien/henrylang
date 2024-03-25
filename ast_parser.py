from typing import List

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

    def peek(self, offset: int = 0) -> Token:
        return self.tokens[self.current + offset]

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
        message = f'line {token.line}: {message}'
        return ParseException(message)
    
    def consume(self, ttype: TokenType, message: str) ->Token:
        if self.check(ttype):
            return self.advance()
        raise self.error(self.peek(), message)
    
    def primary(self) -> Expression:
        # match true or false
        if self.match(TokenType.FALSE):
            return expressions.Literal(False)
        if self.match(TokenType.TRUE):
            return expressions.Literal(True)
        
        # match literals
        if self.match(TokenType.INT, TokenType.FLOAT, TokenType.STR):
            return expressions.Literal(self.previous().literal)
        
        # match identifiers
        if self.match(TokenType.IDENT):
            return expressions.Variable(self.previous())

        # match grouping
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
            ):
                return
            self.advance()

    def statement(self) -> Expression:
        # match variable assignment
        if self.peek().ttype == TokenType.IDENT and self.peek(1).ttype == TokenType.ASSIGN:
            name = self.consume(TokenType.IDENT, "Expect variable name.")
            self.consume(TokenType.ASSIGN, "Expect '=' after variable name.")
            expr = self.expression()
            self.consume(TokenType.NEWLINE, "Expect newline after statement.")
            return expressions.Assignment(name, expr)
        # match basic expression
        expr = self.expression()
        self.consume(TokenType.NEWLINE, "Expect newline after statement.")
        return expr

    def parse(self) -> List[Expression]:
        statements = []
        while not self.is_at_end():
            try:
                statements.append(self.statement())
            except ParseException as e:
                print(e)
                self.synchronize()
        
        return statements


if __name__ == '__main__':
    from scanner import Scanner
    from state import State
    tokens = Scanner('x := "taco"\ny := x\ny = "taco"\n').scan()
    parser = Parser(tokens)
    statements = parser.parse()
    state = State()
    for statement in statements:
        print(statement)
        print(statement.eval(state))
        