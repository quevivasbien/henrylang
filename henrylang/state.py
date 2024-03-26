from typing import Optional, Self

class RuntimeError(Exception):
    pass

class State:
    def __init__(self, parent: Optional[Self] = None):
        self.values = dict()
        self.parent = parent

    def get(self, name):
        value = self.values.get(name)
        if value is not None:
            return value
        # look in parent scope
        if self.parent is not None:
            return self.parent.get(name)
        raise RuntimeError(f'Undefined variable {name}')

    def set(self, name, value):
        self.values[name] = value