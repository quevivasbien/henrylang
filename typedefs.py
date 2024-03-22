from .common import *

TYPE = pp.Keyword("type")

class TypeDef:
    def __init__(self, t):
        self.name = t[1]
        self.members = list(t[2])

    def __repr__(self):
        return f'TypeDef({self.name}, {self.members})'

typedef = (TYPE + ident + LPAREN + pp.Group(pp.delimited_list(ident)) + RPAREN).set_parse_action(
    TypeDef
)