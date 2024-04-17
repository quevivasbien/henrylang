# henrylang

`henry` is a language designed for lobsters. It is a functional language intended mostly for mathematical computation.

## Features

- All variables are immutable.
- Everything is an expression.
- Functions are first-class.
- Types are resolved at compile time.
- Lazy iterators

## Planned / in-progress

- Function polymorphism with static dispatch
- ND Arrays
- Multithreading

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
mysum := |list: Array(Int)| {
    reduce(|acc, x| { acc + x }, list, 0)
}

mysum(0 to 10) = sumi(0 to 10)
```

### Find prime numbers
```
is_prime := |n: Int| {
    if n = 2 { true }
    else {
        sqrt_n := ftoi(powf(itof(n), 0.5)) + 1
        all(|p: Int| { mod(n, p) != 0 } -> 2 to sqrt_n)
    }
}

filter(is_prime, 2 to 100)
```

### Create a custom type
```
Complex := type { re: Float, im: Float }

norm := |x: Complex| {
    powf(x.re * x.re + x.im * x.im, 0.5)
}

x := Complex(1.0, -1.0)
norm(x)
```
