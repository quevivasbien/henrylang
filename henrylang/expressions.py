from typing import List, Optional, Tuple

from .state import State
from .tokens import TokenType, Token

class Expression:
    pass

class Null(Expression):
    def __repr__(self):
        return 'null'

    def eval(self, state: State):
        return False

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
        elif self.operator.ttype == TokenType.AND:
            return left and right
        elif self.operator.ttype == TokenType.OR:
            return left or right
        elif self.operator.ttype == TokenType.TO:
            return range(left, right)
        
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
        
        return Null()

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
        if not self.statements:
            return Null()
        inner_state = State(parent=state)
        for statement in self.statements:
            value = statement.eval(inner_state)
        return value
    
class If(Expression):
    def __init__(
            self,
            if_: Tuple[Expression, Expression],
            elseifs: List[Tuple[Expression, Expression]],
            else_: Optional[Expression],
    ):
        self.if_ = if_
        self.elseifs = elseifs
        self.else_ = else_
    
    def __repr__(self):
        elseifs = ''.join(' elseif ' + str(elseif) for elseif in self.elseifs)
        return f'(if {self.if_}{elseifs} else {self.else_})'

    def eval(self, state: State):
        if self.if_[0].eval(state):
            inner_state = State(parent=state)
            return self.if_[1].eval(inner_state)
        for elseif in self.elseifs:
            if elseif[0].eval(state):
                inner_state = State(parent=state)
                return elseif[1].eval(inner_state)
        if self.else_ is not None:
            inner_state = State(parent=state)
            return self.else_.eval(inner_state)
        return Null()
    
class For(Expression):
    def __init__(self, name: Token, value: Expression, inner: Expression):
        self.name = name
        self.value = value
        self.inner = inner

    def __repr__(self):
        return f'(for {self.name} := {self.value} {self.inner})'
    
    def eval(self, state: State):
        out = []
        for i in self.value.eval(state):
            inner_state = State(parent=state)
            inner_state.set(self.name.lexeme, i)
            out.append(self.inner.eval(inner_state))
        return out