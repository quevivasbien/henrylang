class RuntimeError(Exception):
    pass

class State:
    def __init__(self):
        self.values = dict()

    def get(self, name):
        value = self.values.get(name)
        if value is None:
            raise RuntimeError(f'Undefined variable {name}')
        return value

    def set(self, name, value):
        self.values[name] = value