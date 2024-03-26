from typing import List

from .state import State
from .tokens import TokenType, Token

class Expression:
    pass

class Binary(Expression):
    def __init__(self, left: Expression, operator: Token, right: Expression):
        self.left = left
        self.operator = operator
        self.right = right

    def __repr__(self):
        return f'({self.left} {self.operator} {self.right})'

    def eval(self, state: State):
        left = self.left.eval(state)
        right = self.right.eval(state)

        if self.operator.ttype == TokenType.PLUS:
            return left + right
        elif self.operator.ttype == TokenType.MINUS:
            return left - right
        elif self.operator.ttype == TokenType.STAR:
            return left * right
        elif self.operator.ttype == TokenType.SLASH:
            return left / right
        elif self.operator.ttype == TokenType.GT:
            return left > right
        elif self.operator.ttype == TokenType.LT:
            return left < right
        elif self.operator.ttype == TokenType.GEQ:
            return left >= right
        elif self.operator.ttype == TokenType.LEQ:
            return left <= right
        elif self.operator.ttype == TokenType.EQ:
            return left == right
        elif self.operator.ttype == TokenType.NEQ:
            return left != right
        
        return None

class Grouping(Expression):
    def __init__(self, expression: Expression):
        self.expression = expression

    def __repr__(self):
        return f'({self.expression})'
    
    def eval(self, state: State):
        return self.expression.eval(state)

class Literal(Expression):
    def __init__(self, value: Token):
        self.value = value

    def __repr__(self):
        return str(self.value)
    
    def eval(self, state: State):
        return self.value
    
class Unary(Expression):
    def __init__(self, operator: Token, right: Expression):
        self.operator = operator
        self.right = right

    def __repr__(self):
        return f'({self.operator}{self.right})'
    
    def eval(self, state: State):
        right = self.right.eval(state)

        if self.operator.ttype == TokenType.MINUS:
            return -right
        elif self.operator.ttype == TokenType.BANG:
            return not right
        
        return None

class Variable(Expression):
    def __init__(self, name: Token):
        self.name = name

    def __repr__(self):
        return str(self.name)
    
    def eval(self, state: State):
        return state.get(self.name.lexeme)

class Assignment(Expression):
    def __init__(self, name: Token, value: Expression):
        self.name = name
        self.value = value

    def __repr__(self):
        return f'({self.name} := {self.value})'
    
    def eval(self, state: State):
        value = self.value.eval(state)
        state.set(self.name.lexeme, value)
        return value
    
class Block(Expression):
    def __init__(self, statements: List[Expression]):
        self.statements = statements

    def __repr__(self):
        statements = ' '.join(str(statement) for statement in self.statements)
        return f'{{ {statements} }}'
    
    def eval(self, state: State):
        inner_state = State(parent=state)
        for statement in self.statements:
            value = statement.eval(inner_state)
        return value