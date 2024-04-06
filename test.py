def fib_helper(n, x, y):
    z = x + y
    if n == 1:
        return z
    else:
        return fib_helper(n - 1, y, z)

def fib(n):
    if n < 3:
        return 1
    else:
        return fib_helper(n - 2, 1, 1)
    
print(fib(80))
