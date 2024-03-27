from __future__ import annotations
from typing import Any, List, Optional, TYPE_CHECKING

if TYPE_CHECKING:
    from .expressions import Expression
    from .tokens import Token

from .state import RuntimeError, State

class Null:
    def __repr__(self):
        return 'null'

class AbstractFunction:
    def __call__(self, *args, state: Optional[State] = None) -> Any:
        raise NotImplementedError

class BuiltinFunction(AbstractFunction):
    def __init__(self, fn):
        self.fn = fn
    
    def __call__(self, *args, state: Optional[State] = None) -> Any:
        return self.fn(*args, state = state)

class Function(AbstractFunction):
    def __init__(self, parameters: List[Token], body: Expression):
        self.parameters = parameters
        self.body = body
    
    def __repr__(self):
        return f'function({" ".join(str(s) for s in self.parameters)})'

    def __call__(self, *args, state: Optional[State] = None) -> Any:
        if len(args) != len(self.parameters):
            raise RuntimeError(f'Expected {len(self.parameters)} arguments but got {len(args)}')
        argvals = (arg.eval(state) for arg in args)
        inner_state = State(parent=state)
        for i, argval in enumerate(argvals):
            inner_state.set(self.parameters[i].lexeme, argval)
        return self.body.eval(state=inner_state)
