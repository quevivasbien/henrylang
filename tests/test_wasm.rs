use henrylang::*;

fn run(source: &str) -> String {
    match wasmize(source, Env::default()) {
        Ok((bytes, typ)) => match run_wasm(&bytes, typ) {
            Ok(x) => x,
            Err(e) => panic!("Runtime Error: {}", e),
        },
        Err(e) => panic!("Compile Error: {}", e),
    }
}

#[test]
fn test_arithmetic() {
    assert_eq!(run("1 + 2 * 3 + 4"), "11");
    assert_eq!(run("1 - 2 / (3 - 4)"), "3");
    assert_eq!(run("x := 1 + 2 * 3 + 4 x"), "11");
    assert_eq!(run("x := { x := 1 + { 2 } x + 1 } x"), "4");
}

#[test]
fn test_arrays() {
    assert_eq!(run("[1, 2, 3, 4]"), "[1, 2, 3, 4]");
    assert_eq!(run("a := [1, 2, 3, 4] a"), "[1, 2, 3, 4]");
    assert_eq!(run("a := [1, 2, 3, 4] a = [1, 2, 3, 4]"), "true");
    assert_eq!(run("a := [1, 2, 3, 4] b := a b = a"), "true");
    assert_eq!(run("[1, 2, 3] + [4, 5, 6]"), "[1, 2, 3, 4, 5, 6]");
    assert_eq!(run("a := [1, 2, 3] a(1)"), "2");
    assert_eq!(run("a := [\"hello\", \"world\"] a(1)"), "world");
    assert_eq!(run("a := [[1,2], [3]] a(0) + a(1)"), "[1, 2, 3]");
}

#[test]
fn test_strings() {
    assert_eq!(run("\"Hello, World!\""), "Hello, World!");
    assert_eq!(run("a := \"Hello, World!\" a"), "Hello, World!");
    assert_eq!(run("a := \"Hello, World!\" b := a b = a"), "true");
    assert_eq!(run("\"Hello, \" + \"World!\""), run("\"Hello, World!\""));
    assert_eq!(run("x := { \"Hello\" } x"), "Hello");
    assert_eq!(run("x := { \"Hello, \" + \"World!\" }"), "Hello, World!");
}

#[test]
fn test_if_statement() {
    assert_eq!(run("if true { 1 } else { 0 }"), "1");
    assert_eq!(run("if false { 1 } else { 0 }"), "0");
    assert_eq!(run("if 1 = 1 { 1 } else { 0 }"), "1");
    assert_eq!(run("if { x := 1 x > 0 } { 1 } else { 0 }"), "1");
}

#[test]
fn test_functions() {
    assert_eq!(run("f := |x: Int| { x + 1 } f(1)"), "2");
    assert_eq!(run("f := |x: Float| { x + 1.0 } f(1.0)"), "2.0");
    assert_eq!(run("f := |x: Int, y: Int| { x + y } f(1, 2)"), "3");
    assert_eq!(
        run("f := |x: Arr(Int), i: Int| { x(i) } f([1, 2, 3], 1)"),
        "2"
    );
    assert_eq!(
        run("f := |x: Str, y: Str| { x + y } f(\"hello\", \"world\")"),
        "helloworld"
    );
    assert_eq!(
        run("f := ||{[\"henry\",\"lenry\"]} x:=f() [121,122,188,100] x"),
        "[henry, lenry]"
    );
}

#[test]
fn test_closures() {
    assert_eq!(run("x := 2 f := |y: Int|{x+y+x} f(1)"), "5");
    assert_eq!(run("x := 2 f := |y: Int|{g := |z: Int|{x+z} g(y)} f(1)"), "3");
    assert_eq!(run("x := 2 f := |y: Int|{g := |z: Int|{x+y+z} g(y)} f(1)"), "4");
}

#[test]
fn test_objects() {
    assert_eq!(
        run("MyType := type { c: Bool, a:Int b: Float } MyType(true, 152, 16.2)"),
        "MyType { c: true, a: 152, b: 16.2 }"
    );
    assert_eq!(
        run("(type { a: Str, b: Arr(Int) })(\"henry\", [1,2])"),
        "<anontype> { a: henry, b: [1, 2] }"
    );
    assert_eq!(run("MyType := type { a: Int b: Str } x := MyType(1, \"henry\") x.a = 1 and x.b = \"henry\""), "true");
    assert_eq!(
        run("MyType := type { a: Int } get_a := |x: MyType|{x.a} x := MyType(152) get_a(x)"),
        "152"
    );
}

#[test]
fn test_len() {
    assert_eq!(run("len(\"hello\")"), "5");
    assert_eq!(run("len(\"Ο Χένρι είναι ο πιο κουλ\")"), "24");
    assert_eq!(run("len(\"😀\")"), "1");
    assert_eq!(run("len([1, 2, 3])"), "3");
    assert_eq!(run("len([\"hello\", \"there\"])"), "2");
    assert_eq!(run("len(0 to 5)"), "6");
    assert_eq!(run("len([]: Int)"), "0");
    assert_eq!(run("len(|x:Int|{x} -> []:Int)"), "0");
}

#[test]
fn test_maybe() {
    assert_eq!(run("unwrap(some(1), 0)"), "1");
    assert_eq!(run("unwrap({}: Int, 0)"), "0");
    assert_eq!(run("unwrap(some(\"Henry\"), \"Lenry\")"), "Henry");
    assert_eq!(run("unwrap({}: Str, \"Lenry\")"), "Lenry");

    assert_eq!(run("issome(some(1))"), "true");
    assert_eq!(run("!issome({}: Int)"), "true");
    assert_eq!(run("issome(some(\"Henry\"))"), "true");
    assert_eq!(run("!issome({}: Str)"), "true");
}

#[test]
fn test_ranges() {
    assert_eq!(run("@(0 to 3)"), "[0, 1, 2, 3]");
    assert_eq!(run("@(0 to -3)"), "[0, -1, -2, -3]");
}

#[test]
fn test_map() {
    assert_eq!(run("@(|x: Int| {x + 1} -> 0 to 3)"), "[1, 2, 3, 4]");
    assert_eq!(
        run("f := |x: Int| {x + 1} iter := f -> f -> 0 to 3 @iter"),
        "[2, 3, 4, 5]"
    );
    assert_eq!(
        run("f := |x: Int| {x + 1} iter := f -> [0, 1, 2, 3] @iter"),
        "[1, 2, 3, 4]"
    );
    assert_eq!(
        run("f := |x: Int| { 1.0 } iter := f -> 0 to 3 @iter"),
        "[1.0, 1.0, 1.0, 1.0]"
    );
    assert_eq!(
        run("f := |x: Float| { x + 1.0 } iter := f -> [0.0, 1.0] @iter"),
        "[1.0, 2.0]"
    );
    assert_eq!(
        run("f := |x: Str| { x + \"!\" } iter := f -> [\"hello\", \"world\"] @iter"),
        "[hello!, world!]"
    );
    assert_eq!(
        run("@(|x: Int| { @(0 to x) } -> 0 to 3)"),
        "[[0], [0, 1], [0, 1, 2], [0, 1, 2, 3]]"
    );
}

#[test]
fn test_reduce() {
    assert_eq!(
        run("reduce(|acc: Int, x: Int| { acc + x }, 0 to 100, 0)"),
        "5050"
    );
    assert_eq!(
        run("reduce(|acc: Int, x: Int| { acc + x }, 0 to 100, -5050)"),
        "0"
    );
    assert_eq!(
        run("reduce(|acc: Float, x: Float| { acc + x }, [1.0, 2.0, 3.0], 0.0)"),
        "6.0"
    );
    assert_eq!(
        run("reduce(|acc:Str, x:Str|{acc + \" \" + x}, [\"Henry\", \"is\", \"cool\"], \">\")"),
        "> Henry is cool"
    );
    assert_eq!(run("reduce(|acc:Str, x:Arr(Str)|{ acc + reduce(|acc:Str, x:Str|{acc + x}, x, \"\") }, [[\"Hi\", \"There\"], [\"How\", \"Are\", \"You\"]], \"\")"), "HiThereHowAreYou");
}

#[test]
fn test_filter() {
    assert_eq!(run("@filter(|x: Int| { x > 0 }, -3 to 3)"), "[1, 2, 3]");
    assert_eq!(
        run("@filter(|x: Str| { x = \"Henry\" }, [\"Henry\", \"Lenry\", \"Henry\"])"),
        "[Henry, Henry]"
    );
    assert_eq!(run("@filter(|x: Int| { x > 0 }, 0 to -3)"), "[]");
    assert_eq!(
        run("first_is_positive := |x: Arr(Int)| { x(0) > 0 } @filter(first_is_positive, [[-1, 1, 2], [1, 2, 3]])"),
        "[[1, 2, 3]]"
    );
}

#[test]
fn test_zipmap() {
    assert_eq!(run("@zipmap(|x: Int| { x + 1 }, 0 to 3)"), "[1, 2, 3, 4]");
    assert_eq!(
        run("@zipmap(|x: Int, y: Int| { x * y }, 0 to 3, 0 to -3)"),
        "[0, -1, -4, -9]"
    );
    assert_eq!(
        run("@zipmap(|x: Int, y: Int| { x + y }, [1, 2, 3], [4, 5, 6])"),
        "[5, 7, 9]"
    );
    assert_eq!(
        run("@zipmap(|x:Str, y:Str|{x+ \" \" +y}, [\"Henry\", \"Lenry\", \"Glenry\"], [\"is cool\", \"is not cool\"])"),
        "[Henry is cool, Lenry is not cool]"
    );
}

#[test]
fn test_callable_builtins() {
    assert_eq!(run("abs(1) = abs(-1) and abs(-1) = 1"), "true");
    assert_eq!(run("abs(1.0) = abs(-1.0) and abs(-1.0) = 1.0"), "true");
    assert_eq!(run("float(1)"), "1.0");
    assert_eq!(run("int(1.2)"), "1");
    assert_eq!(run("int(-1.2)"), "-1");
    assert_eq!(run("sqrt(4.0)"), "2.0");
    assert_eq!(run("mod(5, 3)"), "2");
    assert_eq!(run("mod(-5, 3)"), "1");
    assert_eq!(run("sum(0 to 100)"), "5050");
    assert_eq!(run("sum(|x:Int|{float(x)} -> 0 to 100)"), "5050.0");
    assert_eq!(run("prod(1 to 3)"), "6");
    assert_eq!(run("prod(|x:Int|{float(x)} -> 1 to 3)"), "6.0");
    assert_eq!(run("all(|x:Int|{mod(x, 2) = 0} -> 0 to 1234)"), "false");
    assert_eq!(run("any(|x:Int|{mod(x, 2) = 0} -> 0 to 1234)"), "true");
    assert_eq!(run("all(|x:Int|{mod(x, 2) = 0} -> filter(|x:Int|{mod(x, 2) = 0}, 0 to 1234))"), "true");
}

#[test]
fn test_primes() {
    let result = run("

    is_prime := |n: Int| {
        if n = 2 { true }
        else {
            sqrt_n := int(sqrt(float(n))) + 1
            all(|p: Int| { mod(n, p) != 0 } -> 2 to sqrt_n)
        }
    }

    sum(filter(is_prime, 2 to 100))
    ");

    assert_eq!(result, "1060");
}

#[test]
fn test_taylor_series() {
    let result = run("
    factorial := |x: Int| {
        if x <= 1 {
            1
        }
        else {
            prod(1 to x)
        }
    }
    
    approx_e := |n: Int|: Float {
        if n = 0 {
            1.0
        }
        else {
            1.0 / float(factorial(n)) + approx_e(n-1)
        }
    }
    
    approx_e(10)
    ");

    assert_eq!(result, "2.718282");
}
