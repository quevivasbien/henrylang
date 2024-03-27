from typing import Any, Optional

from . import expressions
from . import typedefs

from .ast_parser import Parser
from .scanner import Scanner
from .state import State

def base_state() -> State:
    state = State()

    # define builtins
    def print_(*args, state: Optional[State] = None) -> Any:
        str_ = ' '.join(str(a.eval(state)) for a in args)
        print(str_)
        return expressions.Literal(str_)
    state.set('print', typedefs.BuiltinFunction(print_))

    def map_(fn, l, state: Optional[State] = None) -> Any:
        fn = fn.eval(state)
        l = (expressions.Literal(x) for x in l.eval(state))
        if not isinstance(fn, typedefs.AbstractFunction):
            raise RuntimeError('Expected a function')
        return [fn(x, state=state) for x in l]
    state.set('map', typedefs.BuiltinFunction(map_))

    def filter_(fn, l, state: Optional[State] = None) -> Any:
        fn = fn.eval(state)
        l = (expressions.Literal(x) for x in l.eval(state))
        if not isinstance(fn, typedefs.AbstractFunction):
            raise RuntimeError('Expected a function')
        return [x for x in l if fn(x, state=state)]
    state.set('filter', typedefs.BuiltinFunction(filter_))

    def reduce_(fn, l, state: Optional[State] = None) -> Any:
        fn = fn.eval(state)
        if not isinstance(fn, typedefs.AbstractFunction):
            raise RuntimeError('Expected a function')
        l = [expressions.Literal(x) for x in l.eval(state)]
        if len(l) < 1:
            raise RuntimeError('Expected at least 1 element')
        acc = l[0]
        for i in range(1, len(l)):
            acc = expressions.Literal(fn(acc, l[i], state=state))
        return acc
    state.set('reduce', typedefs.BuiltinFunction(reduce_))

    return state

def exec(code: str, verbose: bool = False):
    tokens = Scanner(code).scan()
    parser = Parser(tokens)
    statements = parser.parse()
    state = base_state()

    value = typedefs.Null()
    for statement in statements:
        if verbose:
            print(statement)
        value = statement.eval(state)
    print(value)
