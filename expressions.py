import pyparsing as pp

from .common import *

expression = pp.Forward()

class LiteralValue:
    def __init__(self, t):
        self.value = t[0]

    def __repr__(self):
        return f'LiteralValue({self.value})'
    
    def python(self):
        return self.value

literal_value = (pp.Word(pp.nums + ".") | ident | pp.QuotedString(quoteChar='"', esc_char='\\')).add_parse_action(LiteralValue)

class Assignment:
    def __init__(self, t):
        self.name = t[0]
        self.value = t[2]
    
    def __repr__(self):
        return f'Assignment({self.name}, {self.value})'
    
    def python(self):
        return f'{self.name} = {self.value.python()}'

assignment = (ident + ASSIGN_EQ + expression).add_parse_action(Assignment)

class Comparison:
    def __init__(self, t):
        self.lhs = t[0]
        self.op = t[1]
        self.rhs = t[2]

    def __repr__(self):
        return f'Comparison({self.lhs}, {self.op}, {self.rhs})'
    
    def python(self):
        return f'{self.lhs.python()} {self.op} {self.rhs.python()}'

comparison = (literal_value + (GE | LE | GEQ | LEQ) + literal_value).set_parse_action(Comparison)

block = pp.Forward()
function_def = pp.Forward()
function_call = pp.Forward()
expression <<= function_def | block | function_call | assignment | comparison | literal_value

class Block(Nestable):
    def __init__(self, t):
        self.expressions = list(t)
        super().__init__()

    def __repr__(self):
        return f'Block({self.expressions})'
    
    def children(self):
        return self.expressions
    
    def python(self):
        indent = ' ' * self.depth * 4
        return '\n'.join(indent + e.python() for e in self.expressions)

block <<= (LBRACE + pp.ZeroOrMore(expression) + RBRACE).add_parse_action(Block)