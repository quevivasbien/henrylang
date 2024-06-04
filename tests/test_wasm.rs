use henrylang::*;

fn run(source: &str) -> String {
    match wasmize(source.to_string(), Env::default()) {
        Ok((bytes, typ)) => {
            match run_wasm(&bytes, typ) {
                Ok(x) => x,
                Err(e) => panic!("Runtime Error: {}", e),
            }
        }
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
    // todo: add more tests here
}

#[test]
fn test_objects() {
    assert_eq!(run("MyType := type { c: Bool, a:Int b: Float } MyType(true, 152, 16.2)"), "MyType { c: true, a: 152, b: 16.2 }");
    assert_eq!(run("(type { a: Str, b: Arr(Int) })(\"henry\", [1,2])"), "<anontype> { a: henry, b: [1, 2] }");
    assert_eq!(run("MyType := type { a: Int b: Str } x := MyType(1, \"henry\") x.a = 1 and x.b = \"henry\""), "true");
}