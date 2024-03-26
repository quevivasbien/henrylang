import sys

import henrylang as hl

if __name__ == '__main__':
    args = sys.argv
    if len(args) < 2:
        # print(f'Usage: {args[0]} <file> [--verbose]')
        # exit(1)
        args = [args[0], 'test.hl', '--verbose']
    
    with open(args[1]) as f:
        code = f.read()

    verbose = True if '--verbose' in args else False

    tokens = hl.Scanner(code).scan()
    parser = hl.Parser(tokens)
    statements = parser.parse()
    state = hl.State()

    for statement in statements:
        if verbose:
            print(statement)
        value = statement.eval(state)
    print(value)
