from .common import *
from .expressions import function_def, function_call, literal_value, block

class FunctionDef(Nestable):
    def __init__(self, t):
        self.name = t[0]
        self.args = list(t[1])
        self.block = t[2]
        super().__init__()

    def children(self):
        return [self.block]
    
    def __repr__(self):
        return f'Function({self.name}, {self.args}, {self.block})'
    
    def python(self):
        indent = ' ' * (self.depth + 1) * 4
        return f'def {self.name}({", ".join(self.args)}):\n{self.block.python()}'

function_def <<= (ident + LPAREN + pp.Group(pp.delimited_list(ident)) + RPAREN + block).add_parse_action(FunctionDef)


class FunctionCall:
    def __init__(self, t):
        self.name = t[0]
        self.args = list(t[1])
    
    def __repr__(self):
        return f'FunctionCall({self.name}, {self.args})'

function_call <<= (ident + LPAREN + pp.Group(pp.delimited_list(literal_value)) + RPAREN).set_parse_action(
    FunctionCall
)