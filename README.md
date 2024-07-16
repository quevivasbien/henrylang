# henrylang

`henry` is a language designed for lobsters.

## How to run

Using cargo:

* For an interactive interpreter session:

    ```
    cargo run
    ```

* To run a script:

    ```
    cargo run [script name]
    ```

(Or use `cargo build` or `cargo install`, then run the resulting binary.)

## Features

- All variables are immutable.
- Everything is an expression.
- Functions are first-class.
- Types are resolved at compile time.
- Iterators are lazy.
- Functions can be overloaded for different argument types.

## In-progress

### Compilation to WASM

Enable with the `--wasm` flag. For example, if building & running with cargo, the command
```
cargo run script.hl -- --wasm
```
will run the script `script.hl` by compiling it to WASM, then executing the compiled web assembly using the Wasmer runtime. 

At this point, most of the current language features are implemented for the WASM compiler. The following is an overview of which features from the bytecode-compiled version of `henrylang` have been implemented, and which are still in-progress:

#### Finished

* Arithmetic
* Variables
* Basic functions
* UTF-8 Strings
* Arrays
* User-defined types
* Iterators (ranges, mapping, filtering, reduction)
* Capturing functions (closures)

#### To-do

* Recursion
* Some built-in functions
* Garbage collection


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
        sqrt_n := int(sqrt(float(n))) + 1
        all(|p: Int| { mod(n, p) != 0 } -> 2 to sqrt_n)
    }
}

filter(is_prime, 2 to 100)
```

### Create a custom type
```
Complex := type { re: Float, im: Float }

norm := |x: Complex| {
    sqrt(x.re * x.re + x.im * x.im)
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
