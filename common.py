import pyparsing as pp

LPAREN, RPAREN, LBRACE, RBRACE = map(pp.Suppress, "(){}")
ASSIGN_EQ = pp.Keyword(":=")
EQ = pp.Keyword("=")
GE = pp.Keyword(">")
LE = pp.Keyword("<")
GEQ = pp.Keyword(">=")
LEQ = pp.Keyword("<=")

ident = pp.common.identifier

class Nestable:
    def __init__(self):
        self.depth = 0
        self.increment_depth(only_children=True)

    def children(self):
        raise NotImplemented
    
    def increment_depth(self, only_children=False):
        if not only_children:
            self.depth += 1
            # print(f'incrementing my depth to {self.depth}: {self}')
        for e in self.children():
            if isinstance(e, Nestable):
                e.increment_depth()