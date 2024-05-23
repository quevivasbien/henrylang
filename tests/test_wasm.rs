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
}

#[test]
fn test_arrays() {
    assert_eq!(run("[1, 2, 3, 4]"), "[1, 2, 3, 4]");
    assert_eq!(run("a := [1, 2, 3, 4] a"), "[1, 2, 3, 4]");
    assert_eq!(run("a := [1, 2, 3, 4] a = [1, 2, 3, 4]"), "true");
    assert_eq!(run("a := [1, 2, 3, 4] b := a b = a"), "true");
    assert_eq!(run("[1, 2, 3] + [4, 5, 6]"), "[1, 2, 3, 4, 5, 6]");
}

#[test]
fn test_strings() {
    assert_eq!(run("\"Hello, World!\""), "Hello, World!");
    assert_eq!(run("a := \"Hello, World!\" a"), "Hello, World!");
    assert_eq!(run("a := \"Hello, World!\" b := a b = a"), "true");
    assert_eq!(run("\"Hello, \" + \"World!\""), run("\"Hello, World!\""));
}
