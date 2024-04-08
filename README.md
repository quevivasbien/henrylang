# henrylang

`henry` is a language designed for lobsters. It is a functional language inspired by Julia, Rust, and Haskell. It is intended mostly for mathematical computation.

## Features

- All variables are immutable.
- Everything is an expression.
- Functions are first-class.

## Usage examples

### Define a variable
```
x := "hello"
y := 3.14159
```

### Define a function, then call it
```
f := |x| {
    x * x + 2
}

f(4)
```

### Compute a sum in two different ways
```
mysum := |list| {
    reduce(|acc, x| { acc + x }, list, 0)
}

mysum(0 to 10) = sum(0 to 10)
```

### Find prime numbers
```
is_prime := |n| {
    if n = 2 { true }
    else {
        sqrt_n := int(pow(float(n), 0.5)) + 1
        all(|p| { mod(n, p) != 0 } -> 2 to sqrt_n)
    }
}

filter(is_prime, 2 to 100)
```

### Create a custom type
```
Complex := type { re im }

norm := |x| {
    sqrt(x.re * x.re + x.im * x.im)
}

x := Complex(1.0, -1.0)
norm(x)
```
