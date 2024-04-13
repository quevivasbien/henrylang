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
    let source = "fib_helper := |n: Int, x: Int, y: Int| {
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
            prod(1 to x)
        }
    }
    
    approx_e := |n: Int| {
        if n = 0 {
            1.0
        }
        else {
            1.0 / float(factorial(n)) + approx_e(n-1)
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
    f := |x| {
        a := x
        g := || {
            add_a := |z| { a + z }
            add_a
        }
        add_a := g()
    
        add_a(2)
    }
    
    sum(f -> 0 to 3)
    ";

    let result = run_expect_value!(source, Int);
    assert_eq!(result, 14);
}

#[test]
fn test_object() {
    let source = "
    myobj := type {
        a
        b
        c
    }
    x := myobj(1, 2, 3)
    y := myobj(-1, -2, -3)
    
    zipped := zip(array(x), array(y))
    added := sum(|x| { sum(x) } -> zipped)
    
    added = 0 and x != y and x.b = -y(1)
    ";

    let result = run_expect_value!(source, Bool);
    assert!(result);
}

#[test]
fn test_maybe() {
    let source = "
    null_if_negative := |x| {
        if x > 0 {
            x
        }
    }
    
    positives := |x| {unwrap(x, 0)} -> filter(is_some, null_if_negative -> -4 to 4)
    len(positives) = 4 and sum(positives) = 10
    ";

    let result = run_expect_value!(source, Bool);
    assert!(result);
}

#[test]
fn test_builtins() {
    let result = run_expect_value!("sum(0 to 4)", Int);
    assert_eq!(result, 10);

    let result = run_expect_value!("prod(1 to 4)", Int);
    assert_eq!(result, 24);

    let result = run_expect_value!("sum(string -> (0 to 4))", String);
    assert_eq!(result, "01234");

    let result = run_expect_value!("max(|x| { -2 * x*x + x + 4 } -> (-4 to 4))", Int);
    assert_eq!(result, 4);

    let result = run_expect_value!("min(|x| { -2 * x*x + x + 4 } -> (-4 to 4))", Int);
    assert_eq!(result, -32);

    let result = run_expect_value!("sum(|x| { if x > 0 {x} else {-x} } -> filter(|x| {x > 4}, -500 to 500))", Int);
    assert_eq!(result, 125240);

    let result = run_expect_value!("sum(0 to 100) = reduce(|acc, x| { acc + x }, 0 to 100, 0)", Bool);
    assert!(result);

    let result = run_expect_value!("0 to 100 = reduce(|acc, x| { acc + [x] }, 0 to 100, [])", Bool);
    assert!(result);
}
