use approx::assert_relative_eq;
use henrylang::*;

macro_rules! run_expect_value {
    ($source:expr, $variant:ident) => {
        match VM::new().interpret($source.to_string())
            .unwrap()
        {
            values::TaggedValue::$variant(x) => x,
            _ => panic!("Should be a {}", stringify!($variant)),
        }
    }
}

#[test]
fn test_fib() {
    let source = "fib_helper := |n: Int, x: Int, y: Int|: Int {
        z := x + y
        if n = 1 {
            z
        }
        else {
            fib_helper(n - 1, y, z)
        }
    }
    
    fib := |n: Int| {
        if n < 3 {
            1
        }
        else {
            fib_helper(n - 2, 1, 1)
        }
    }
    
    fib(80)
    ";

    let result = run_expect_value!(source, Int);
    assert_eq!(result, 23416728348467685);
}

#[test]
fn test_euler() {
    let source = "factorial := |x: Int| {
        if x <= 1 {
            1
        }
        else {
            prodi(1 to x)
        }
    }
    
    approx_e := |n: Int|: Float {
        if n = 0 {
            1.0
        }
        else {
            1.0 / itof(factorial(n)) + approx_e(n-1)
        }
    }
    
    approx_e(20)
    ";

    let result = run_expect_value!(source, Float);
    assert_relative_eq!(result, 2.718281828459045);
}

#[test]
fn test_primes() {
    let source = "
    is_prime := |n| {
        if n = 2 { true }
        else {
            sqrt_n := int(pow(float(n), 0.5)) + 1
            all(|p| { mod(n, p) != 0 } -> 2 to sqrt_n)
        }
    }
    
    primes := filter(is_prime, 2 to 100)
    sum(primes)
    ";

    let result = run_expect_value!(source, Int);
    assert_eq!(result, 1060);
}

#[test]
fn test_closure() {
    let source = "
    f := |x: Int| {
        a := x
        g := || {
            add_a := |z: Int| { a + z }
            add_a
        }
        add_a := g()
    
        add_a(2)
    }
    
    sumi(f -> 0 to 3)
    ";

    let result = run_expect_value!(source, Int);
    assert_eq!(result, 14);
}

#[test]
fn test_object() {
    let source = "
    myobj := type {
        a: Int
        b: Int
        c: String
    }
    x := myobj(1, 2, \"henry\")
    y := myobj(-1, -2, \"henry\")
    
    x.a = -y.a and x.b = -y.b and x.c = y.c
    ";

    let result = run_expect_value!(source, Bool);
    assert!(result);
}

#[test]
fn test_maybe() {
    let source = "
    null_if_negative := |x: Int| {
        if x > 0 {
            x
        }
    }
    
    zeros_if_negative := |x:Maybe(Int)|{unwrap(x, 0)} -> (null_if_negative -> -4 to 4)
    sumi(zeros_if_negative)
    ";

    let result = run_expect_value!(source, Int);
    assert_eq!(result, 10);
}

#[test]
fn test_reduce() {
    let source = "
    sum := |arr: Array(Int)| {
        reduce(|acc: Int, x: Int| {acc + x}, arr, 0)
    }
    
    n := powi(2, 8)
    sum(0 to n) = sumi(0 to n)
    ";
    let result = run_expect_value!(source, Bool);
    assert!(result);

    let source = "
    my_all := |arr: Array(Bool)| {
        reduce(|acc: Bool, x: Bool|{acc and x}, arr, true)
    }
    
    all_true := |_x: Int| {true} -> 0 to 100
    some_false := |x: Int| { x < 90 or x > 95 } -> 0 to 100
    
    my_all(all_true) = all(all_true) ? = true
    my_all(some_false) = all(some_false) ? = false
    ";
    let result = run_expect_value!(source, Bool);
    assert!(result);    
}

#[test]
fn test_zipmap() {
    let source = "
    haslen := |arr: Array(Int), l: Int| { len(arr) = l }
    all(zipmap(haslen, [0 to 3, 0 to 4, -4 to 4], [4, 5, 9]))
    ";
    let result = run_expect_value!(source, Bool);
    assert!(result);

    let source = "
    x1 := |x:Int|{mod(x, 2)} -> 0 to 4
    x2 := zipmap(mod, 0 to 4, [2, 2, 2, 2, 2])
    x1 = x2
    ";
    let result = run_expect_value!(source, Bool);
    assert!(result);

    let source = "
    MyType := type { name: String, number: Int }
    mytypes := zipmap(MyType, [\"henry\", \"lenry\"], [1, 2])

    mytypes(0).name = \"henry\" and mytypes(1).number = 2 
    ";
    let result = run_expect_value!(source, Bool);
    assert!(result);
}