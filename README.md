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
