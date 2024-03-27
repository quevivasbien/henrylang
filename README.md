# henrylang

`henry` is a language designed for lobsters. It is a primarily functional language inspired by Julia, Rust, and Haskell, best-suited for mathematical computation.

## Features

- All variables are immutable.
- Everything is an expression.

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

### Compute a sum
```
sum := |list| reduce(|acc, x| acc + x, list)
sum(0 to 10)
```
