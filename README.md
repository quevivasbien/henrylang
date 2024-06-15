# henrylang

`henry` is a language designed for lobsters.

## Features

- All variables are immutable.
- Everything is an expression.
- Functions are first-class.
- Types are resolved at compile time.
- Iterators are lazy.
- Functions can be overloaded for different argument types.

## In-progress

- Compilation to WASM (currently supports math, non-capturing functions, custom types, and all basic data types except iterators)

## Usage examples

### Define a variable
```
x := "hello"
y := 3.14159
```

### Define a function, then call it
```
f := |x: Int| {
    x * x + 2
}

f(4)
```

### Compute a sum in two different ways
```
mysum := |iter: Iter(Int)| {
    reduce(|acc, x| { acc + x }, iter, 0)
}

mysum(0 to 10) = sum(0 to 10)
```

### Find prime numbers
```
is_prime := |n: Int| {
    if n = 2 { true }
    else {
        sqrt_n := int(pow(float(n), 0.5)) + 1
        all(|p: Int| { mod(n, p) != 0 } -> 2 to sqrt_n)
    }
}

filter(is_prime, 2 to 100)
```

### Create a custom type
```
Complex := type { re: Float, im: Float }

norm := |x: Complex| {
    pow(x.re * x.re + x.im * x.im, 0.5)
}

x := Complex(1.0, -1.0)
norm(x)
```

### Pass a function to a function
```
func_sum := |f: Func(Int, Int), g: Func(Int, Int), x: Int| {
    f(x) + g(x)
}

func_sum(|x: Int|{ x + 1 }, |x: Int|{ x + 2 }, 1)
```
