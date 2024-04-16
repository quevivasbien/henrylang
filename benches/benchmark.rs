use criterion::{criterion_group, criterion_main, Criterion};

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

fn fibonacci() -> i64 {
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

    run_expect_value!(source, Int)
}

fn sum_primes() -> i64 {
    let source = "
    is_prime := |n: Int| {
        if n = 2 { true }
        else {
            sqrt_n := ftoi(powf(itof(n), 0.5)) + 1
            all(|p: Int| { mod(n, p) != 0 } -> 2 to sqrt_n)
        }
    }
    
    primes := filter(is_prime, 2 to 10000)
    sumi(primes)
    ";
    run_expect_value!(source, Int)
}

fn criterion_fibonacci(c: &mut Criterion) {
    c.bench_function(
        "fibonacci",
        move |b| b.iter(|| fibonacci())
    );
}

fn criterion_primes(c: &mut Criterion) {
    let mut group = c.benchmark_group("primes");
    group.sample_size(80);
    group.bench_function(
        "sum_primes",
        move |b| b.iter(sum_primes)
    );
    group.finish();
}

criterion_group!(benches, criterion_fibonacci, criterion_primes);
criterion_main!(benches);