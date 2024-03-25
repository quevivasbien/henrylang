from tokens import TokenType, Token

class Expression:
    pass

class Binary(Expression):
    def __init__(self, left: Expression, operator: Token, right: Expression):
        self.left = left
        self.operator = operator
        self.right = right

    def __repr__(self):
        return f'({self.left} {self.operator} {self.right})'

    def eval(self):
        left = self.left.eval()
        right = self.right.eval()

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
    
    def eval(self):
        return self.expression.eval()

class Literal(Expression):
    def __init__(self, value: Token):
        self.value = value

    def __repr__(self):
        return str(self.value)
    
    def eval(self):
        return self.value
    
class Unary(Expression):
    def __init__(self, operator: Token, right: Expression):
        self.operator = operator
        self.right = right

    def __repr__(self):
        return f'({self.operator}{self.right})'
    
    def eval(self):
        right = self.right.eval()

        if self.operator.ttype == TokenType.MINUS:
            return -right
        elif self.operator.ttype == TokenType.BANG:
            return not right
        
        return None