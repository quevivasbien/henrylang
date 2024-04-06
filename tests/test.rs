use approx::assert_relative_eq;
use henrylang::*;

#[test]
fn test_fib() {
    let source = "fib_helper := |n, x, y| {
        z := x + y
        if n = 1 {
            z
        }
        else {
            fib_helper(n - 1, y, z)
        }
    }
    
    fib := |n| {
        if n < 3 {
            1
        }
        else {
            fib_helper(n - 2, 1, 1)
        }
    }
    
    fib(80)
    ";

    let result = VM::new().interpret(source.to_string())
        .unwrap()
        .expect("Should be a value")
    ;
    let value = match result {
        Value::Int(x) => x,
        _ => panic!("Should be an Int"),
    };
    assert_eq!(value, 23416728348467685);
}

#[test]
fn test_euler() {
    let source = "factorial := |x| {
        if x <= 1 {
            1
        }
        else {
            x * factorial(x - 1)
        }
    }
    
    approx_e := |n| {
        if n = 0 {
            1.0
        }
        else {
            1.0 / float(factorial(n)) + approx_e(n-1)
        }
    }
    
    approx_e(20)
    ";

    let result = VM::new().interpret(source.to_string())
        .unwrap()
        .expect("Should be a value")
    ;
    let value = match result {
        Value::Float(x) => x,
        _ => panic!("Should be an Float"),
    };
    assert_relative_eq!(value, 2.718281828459045);
}