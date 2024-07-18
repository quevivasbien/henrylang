# henrylang

`henrylang` is a programming language.

This repository includes both a bytecode interpreter and a web assembly compiler for henrylang. Both implementations are built from scratch.

## How to run

Compile with cargo:
```bash
cargo build --release
```

Compile & run:
```bash
cargo run --release
```

Or install on your machine:
```bash
cargo install --path .
```

This will create an executable called `henrylang`. You can run that executable for an interactive interpreter session, or provide a file to run, i.e.,
```bash
henrylang script.hl
```

## Compilation to WASM

If you want to compile code to be run in a web environment, you can provide the `--save` flag. For example,
```bash
henrylang script.hl --save
```
will create a directory called `script_wasm` that contains 3 files: an `index.html`, `index.js`, and `module.wasm`. You can open `index.html` in a web browser to see a simple example that loads and runs the web assembly module and displays the result.

There is also the option to run web assembly code using the Wasmer runtime. To allow this, you'll need to compile with the `wasmer` feature enabled: e.g.,
```bash
cargo build --release -F=wasmer
```
Then you can supply the `--wasm` flag to run scripts by compiling them to web assembly, then running it with the Wasmer runtime (instead of using the bytecode interpreter). For example:
```bash
henrylang script.hl --wasm
```


At this point, most of the current language features are implemented for the WASM compiler. The following is an overview of which features from the bytecode-compiled version of `henrylang` have been implemented, and which are still in-progress:

#### Finished

- Arithmetic
- Variables
- Basic functions
- UTF-8 Strings
- Arrays
- User-defined types
- Iterators (ranges, mapping, filtering, reduction)
- Capturing functions (closures)
- Builtin functions
- Recursive functions

#### To-do

- Garbage collection

Note that the bytecode interpreter uses 64 bit data types, while the WASM implementation uses 32 bit types.

## Features

- All variables are immutable.
- Everything is an expression.
- Functions are first-class.
- Types are resolved at compile time.
- Iterators are lazy.
- Functions can be overloaded for different argument types.

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
